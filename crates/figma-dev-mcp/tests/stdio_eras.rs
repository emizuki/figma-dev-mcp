//! Real-binary stdio protocol coverage for both MCP eras.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

use crate::common::{
    STDIO_ERA_LOCK, assert_production_ports_free, frontend_port, plugin_port, wait_until,
};

const TOOL_NAMES: [&str; 14] = [
    "get_components",
    "get_design_context",
    "get_dev_mode_data",
    "get_fonts",
    "get_metadata",
    "get_motion",
    "get_nodes",
    "get_reactions",
    "get_screenshot",
    "get_selection",
    "get_styles",
    "get_variables",
    "list_files",
    "search_nodes",
];

const PROMPT_NAMES: [&str; 3] = [
    "prototype_flow_strategy",
    "read_design_strategy",
    "style_audit_strategy",
];

struct StdioServer {
    child: Child,
    stdin: Option<std::process::ChildStdin>,
    lines: Receiver<String>,
    collected: Vec<String>,
}

impl StdioServer {
    fn spawn() -> Self {
        let server = Self::spawn_without_ports();
        assert!(
            wait_until(Duration::from_secs(5), || {
                std::net::TcpStream::connect(("127.0.0.1", plugin_port())).is_ok()
                    && std::net::TcpStream::connect(("127.0.0.1", frontend_port())).is_ok()
            }),
            "production binary must bind the plugin and frontend listeners"
        );
        server
    }

    /// Spawn without waiting for the listeners. Used by tests where election
    /// cannot succeed, so the ports never come up.
    fn spawn_without_ports() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_figma-dev-mcp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("FIGMA_DEV_MCP_LOG", "off")
            .spawn()
            .expect("production binary must spawn");
        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = child.stdout.take().expect("stdout is piped");
        let stderr = child.stderr.take().expect("stderr is piped");
        thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                line.clear();
            }
        });
        let (tx, lines) = mpsc::channel();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else {
                    break;
                };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        // From here the Drop guard is live, so a failed bind assertion in
        // `spawn` reaps the child instead of leaking it onto the production
        // ports.
        Self {
            child,
            stdin: Some(stdin),
            lines,
            collected: Vec::new(),
        }
    }

    fn request(&mut self, payload: Value) -> Value {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        writeln!(stdin, "{payload}").expect("stdin write");
        stdin.flush().expect("stdin flush");
        let line = self
            .lines
            .recv_timeout(Duration::from_secs(5))
            .expect("server must emit a JSON-RPC frame on stdout");
        self.collected.push(line.clone());
        let value: Value = serde_json::from_str(&line).unwrap_or_else(|error| {
            panic!("stdout must contain protocol frames only, got {line:?}: {error}")
        });
        assert_eq!(value["jsonrpc"], "2.0", "stdout frame is not JSON-RPC");
        value
    }

    fn notify(&mut self, payload: Value) {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        writeln!(stdin, "{payload}").expect("stdin write");
        stdin.flush().expect("stdin flush");
    }

    fn kill(mut self) {
        self.stdin.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    fn terminate_sigterm(mut self) {
        self.stdin.take();
        let pid = self.child.id();
        let status = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .expect("kill -TERM");
        assert!(status.success(), "kill -TERM must succeed");
        let finished = wait_until(Duration::from_secs(5), || {
            matches!(self.child.try_wait(), Ok(Some(_)))
        });
        if !finished {
            let _ = self.child.kill();
            let _ = self.child.wait();
            panic!("SIGTERM must terminate the production binary without hanging on idle grace");
        }
        while let Ok(line) = self.lines.recv_timeout(Duration::from_millis(50)) {
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(&line).unwrap_or_else(|error| {
                panic!("stdout after SIGTERM still must be protocol frames, got {line:?}: {error}")
            });
            assert_eq!(value["jsonrpc"], "2.0");
            self.collected.push(line);
        }
        assert!(
            self.collected
                .iter()
                .all(|line| serde_json::from_str::<Value>(line)
                    .ok()
                    .is_some_and(|value| value["jsonrpc"] == "2.0")),
            "stdout must contain protocol frames only"
        );
        assert!(
            wait_until(Duration::from_secs(2), || {
                std::net::TcpListener::bind(("127.0.0.1", plugin_port())).is_ok()
                    && std::net::TcpListener::bind(("127.0.0.1", frontend_port())).is_ok()
            }),
            "SIGTERM/EOF cleanup must release production listeners"
        );
        assert_production_ports_free();
    }
}

impl Drop for StdioServer {
    /// These tests bind the real production ports. A panic anywhere between
    /// `Self` coming into existence in `spawn` and an explicit
    /// `kill`/`terminate_sigterm` would otherwise leave a live server holding
    /// 3056 and 3057 — breaking every later test in this file and the
    /// developer's own Figma session with it. `spawn` constructs `Self`
    /// immediately after taking stdin/stdout/stderr and starting the reader
    /// threads, before the port-bind assertion, precisely so this guard is
    /// live for that assertion. The one gap this does not cover: a panic
    /// during the `Command::spawn` call itself or the three `.take().expect`
    /// calls right after it, before `Self` exists. That window cannot
    /// realistically fire — the pipes were configured on the very same
    /// `Command` that just spawned successfully — but it is not covered by
    /// this guard.
    fn drop(&mut self) {
        self.stdin.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn modern_meta() -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": {
            "name": "stdio-era-test",
            "version": "0.1.0"
        },
        "io.modelcontextprotocol/clientCapabilities": {}
    })
}

fn assert_tool_catalog(result: &Value) {
    let names: Vec<&str> = result["tools"]
        .as_array()
        .expect("tools list")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name"))
        .collect();
    assert_eq!(names, TOOL_NAMES);
}

fn assert_prompt_catalog(result: &Value) {
    let names: Vec<&str> = result["prompts"]
        .as_array()
        .expect("prompts list")
        .iter()
        .map(|prompt| prompt["name"].as_str().expect("prompt name"))
        .collect();
    assert_eq!(names, PROMPT_NAMES);
}

#[test]
fn modern_2026_07_28_discover_and_stateless_lists_over_real_stdio() {
    let _guard = STDIO_ERA_LOCK.lock().expect("stdio era lock");
    let mut server = StdioServer::spawn();
    let discover = server.request(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "server/discover",
        "params": { "_meta": modern_meta() }
    }));
    let result = &discover["result"];
    let versions: Vec<&str> = result["supportedVersions"]
        .as_array()
        .expect("supported versions")
        .iter()
        .map(|version| version.as_str().expect("version"))
        .collect();
    assert!(versions.contains(&"2026-07-28"));
    assert!(versions.contains(&"2025-11-25"));
    assert!(result["capabilities"]["tools"].is_object());
    assert!(result["capabilities"]["prompts"].is_object());

    let tools = server.request(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": { "_meta": modern_meta() }
    }));
    assert_tool_catalog(&tools["result"]);
    assert_eq!(tools["result"]["ttlMs"], 86_400_000);
    assert_eq!(tools["result"]["cacheScope"], "public");

    let prompts = server.request(json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "prompts/list",
        "params": { "_meta": modern_meta() }
    }));
    assert_prompt_catalog(&prompts["result"]);
    assert_eq!(prompts["result"]["ttlMs"], 86_400_000);
    assert_eq!(prompts["result"]["cacheScope"], "public");

    server.terminate_sigterm();
}

/// The bug this fixes: killing the leader used to close 3056 forever, leaving
/// the plugin reconnecting against nothing and every follower orphaned.
#[test]
fn killing_the_leader_lets_a_follower_reopen_the_plugin_port() {
    let _guard = STDIO_ERA_LOCK.lock().expect("stdio era lock");

    let leader = StdioServer::spawn();
    assert!(
        wait_until(Duration::from_secs(10), || port_is_listening(plugin_port())),
        "the first server must bind the plugin port"
    );

    let mut follower = StdioServer::spawn();
    assert!(
        wait_until(Duration::from_secs(10), || port_is_listening(
            frontend_port()
        )),
        "the frontend port must stay bound while both servers run"
    );

    // Gate: `initialize` completing only proves the process is alive and
    // answering RPCs — the supervisor now races its own election against
    // this call, so it does not prove election finished. What actually
    // proves this process has a backend is the pre-kill `list_files` call
    // below: an unelected process has an unattached client, which returns
    // CONNECTION_LOST, so a plain success there is the real gate.
    let initialize = follower.request(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "failover-follower", "version": "0.1.0" }
        }
    }));
    assert_eq!(initialize["result"]["serverInfo"]["name"], "figma-dev-mcp");
    follower.notify(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }));

    // The follower is serving through the leader right now, over the RPC hop to
    // the other process. `list_files` needs no Figma plugin: against a live
    // broker it returns an empty list *successfully*, and against a dead RPC
    // connection it returns CONNECTION_LOST — so a plain success here is
    // unambiguous evidence the hop works before the leader dies.
    let served = follower.request(list_files_call(2));
    assert_ne!(
        served["result"]["isError"],
        json!(true),
        "the follower must serve tool calls through the leader before it dies, got {served}"
    );

    leader.kill();

    assert!(
        wait_until(Duration::from_secs(20), || port_is_listening(plugin_port())),
        "the surviving server must re-elect and reopen the plugin port"
    );
    assert!(
        port_is_listening(frontend_port()),
        "the promoted leader must own the frontend port too"
    );

    // Reopening the ports is only half the fix. The MCP session on this process
    // never restarted, so it is still holding the same BrokerClient it was
    // handed at startup; unless the supervisor installed the new backend into
    // it, every call still goes to the RPC connection of the process we just
    // killed and comes back CONNECTION_LOST forever. That is the reported bug —
    // a server that answers but returns CONNECTION_LOST for everything.
    //
    // Retried because `elect()` binds the ports before the new backend is
    // installed, so the probe above can win by a hair. Without the install the
    // loop never escapes CONNECTION_LOST and the deadline fires.
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let mut identifier = 3;
    let recovered = loop {
        let response = follower.request(list_files_call(identifier));
        identifier += 1;
        if response["result"]["structuredContent"]["code"] != json!("CONNECTION_LOST") {
            break response;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the promoted follower must stop returning CONNECTION_LOST, still got {response}"
        );
        thread::sleep(Duration::from_millis(200));
    };
    assert!(
        recovered["result"].is_object(),
        "the recovered call must return a result, not a protocol-level error frame"
    );
    assert_ne!(
        recovered["result"]["isError"],
        json!(true),
        "the promoted follower must serve tool calls through its own broker, got {recovered}"
    );

    follower.kill();
    assert!(
        wait_until(Duration::from_secs(10), || {
            std::net::TcpListener::bind(("127.0.0.1", plugin_port())).is_ok()
        }),
        "both listeners must be released once every server is gone"
    );
    assert_production_ports_free();
}

/// A `tools/call` for `list_files`, the one tool that needs no Figma plugin.
fn list_files_call(identifier: u32) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": identifier,
        "method": "tools/call",
        "params": { "name": "list_files", "arguments": {} }
    })
}

fn port_is_listening(port: u16) -> bool {
    std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
}

#[test]
fn legacy_2025_11_25_initialize_and_lists_over_real_stdio() {
    let _guard = STDIO_ERA_LOCK.lock().expect("stdio era lock");
    let mut server = StdioServer::spawn();
    let initialize = server.request(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "stdio-era-legacy", "version": "0.1.0" }
        }
    }));
    assert_eq!(initialize["result"]["protocolVersion"], "2025-11-25");
    assert!(initialize["result"]["capabilities"]["tools"].is_object());
    assert!(initialize["result"]["capabilities"]["prompts"].is_object());
    assert_eq!(initialize["result"]["serverInfo"]["name"], "figma-dev-mcp");

    server.notify(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }));

    let tools = server.request(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    }));
    assert_tool_catalog(&tools["result"]);

    let prompts = server.request(json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "prompts/list"
    }));
    assert_prompt_catalog(&prompts["result"]);

    server.terminate_sigterm();
}

/// A follower must notice its leader dying even if its own MCP client has
/// never sent `initialize`. Supervision used to be driven only after the
/// stdio service finished its handshake, so an uninitialized follower sat
/// there while port 3056 stayed shut.
#[test]
fn an_uninitialized_follower_still_reopens_the_plugin_port() {
    let _guard = STDIO_ERA_LOCK.lock().expect("stdio era lock");

    let leader = StdioServer::spawn();
    let follower = StdioServer::spawn();
    // Deliberately no `initialize` on either process. That is the point.

    leader.kill();

    assert!(
        wait_until(Duration::from_secs(20), || port_is_listening(plugin_port())),
        "an uninitialized follower must re-elect and reopen the plugin port"
    );
    assert!(
        port_is_listening(frontend_port()),
        "the promoted leader must own the frontend port too"
    );

    follower.kill();
    assert!(
        wait_until(Duration::from_secs(10), || {
            std::net::TcpListener::bind(("127.0.0.1", plugin_port())).is_ok()
        }),
        "both listeners must be released once every server is gone"
    );
    assert_production_ports_free();
}

/// An election that can never succeed must not hang the process. A foreign
/// listener on the frontend port accepts the connection and never handshakes,
/// so every election attempt fails — but the MCP service must still answer.
#[test]
fn an_election_that_never_succeeds_still_answers_the_client() {
    let _guard = STDIO_ERA_LOCK.lock().expect("stdio era lock");

    let squatter = std::net::TcpListener::bind(("127.0.0.1", frontend_port()))
        .expect("the frontend port must be free before this test");
    // Non-blocking plus a stop flag, so the thread can be joined and the port
    // actually released. A thread parked in a blocking `accept()` would hold
    // 3057 for the rest of the process and break every test that follows.
    squatter
        .set_nonblocking(true)
        .expect("the squatter must not block its thread forever");
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let accepting = std::thread::spawn({
        let stop = std::sync::Arc::clone(&stop);
        move || {
            // Accept and hold. Never speak the frontend protocol, so every
            // handshake times out and the election can never be entered.
            let mut held = Vec::new();
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                match squatter.accept() {
                    Ok((stream, _)) => held.push(stream),
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            }
        }
    });

    let mut server = StdioServer::spawn_without_ports();
    let initialize = server.request(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "stdio-era-squatted", "version": "0.1.0" }
        }
    }));
    assert_eq!(
        initialize["result"]["serverInfo"]["name"], "figma-dev-mcp",
        "the process must answer its client even while election keeps failing"
    );

    server.notify(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    let listed = server.request(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": { "name": "list_files", "arguments": {} }
    }));
    assert_eq!(
        listed["result"]["structuredContent"]["code"], "CONNECTION_LOST",
        "with no backend the call must fail retryably rather than hang"
    );

    server.kill();
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    accepting.join().expect("the squatter thread must exit");
    assert!(
        wait_until(Duration::from_secs(10), || {
            std::net::TcpListener::bind(("127.0.0.1", plugin_port())).is_ok()
        }),
        "the plugin port must be free once the server is gone"
    );
    assert_production_ports_free();
}
