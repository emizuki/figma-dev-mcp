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
    stdin: std::process::ChildStdin,
    lines: Receiver<String>,
    collected: Vec<String>,
}

impl StdioServer {
    fn spawn() -> Self {
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
        assert!(
            wait_until(Duration::from_secs(5), || {
                std::net::TcpStream::connect(("127.0.0.1", plugin_port())).is_ok()
                    && std::net::TcpStream::connect(("127.0.0.1", frontend_port())).is_ok()
            }),
            "production binary must bind the plugin and frontend listeners"
        );
        Self {
            child,
            stdin,
            lines,
            collected: Vec::new(),
        }
    }

    fn request(&mut self, payload: Value) -> Value {
        writeln!(self.stdin, "{payload}").expect("stdin write");
        self.stdin.flush().expect("stdin flush");
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
        writeln!(self.stdin, "{payload}").expect("stdin write");
        self.stdin.flush().expect("stdin flush");
    }

    fn terminate_sigterm(mut self) {
        drop(self.stdin);
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
    let tools = result["tools"].as_array().expect("tools list");
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name"))
        .collect();
    assert_eq!(names, TOOL_NAMES);
    assert_no_top_level_combinators(tools);
}

/// The Anthropic API rejects a tool whose input schema puts `oneOf`, `anyOf`,
/// or `allOf` at the root, and Claude Code drops such a tool from the catalog
/// it offers the model. A schema that only reaches the wire is not enough; it
/// has to reach the model.
fn assert_no_top_level_combinators(tools: &[Value]) {
    for tool in tools {
        let name = tool["name"].as_str().expect("tool name");
        let schema = tool["inputSchema"].as_object().expect("input schema");
        for keyword in ["oneOf", "anyOf", "allOf"] {
            assert!(
                !schema.contains_key(keyword),
                "{name} puts {keyword} at the root of its input schema, \
                 so clients drop the tool before the model ever sees it"
            );
        }
    }
}

fn assert_resource_catalog(result: &Value) {
    let uris: Vec<&str> = result["resources"]
        .as_array()
        .expect("resources list")
        .iter()
        .map(|resource| resource["uri"].as_str().expect("resource uri"))
        .collect();
    let expected: Vec<String> = PROMPT_NAMES
        .iter()
        .map(|name| format!("figma://strategy/{name}"))
        .collect();
    assert_eq!(uris, expected);
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
    assert!(result["capabilities"]["resources"].is_object());

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

    let resources = server.request(json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "resources/list",
        "params": { "_meta": modern_meta() }
    }));
    assert_resource_catalog(&resources["result"]);
    assert_eq!(resources["result"]["ttlMs"], 86_400_000);
    assert_eq!(resources["result"]["cacheScope"], "public");

    let read = server.request(json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "resources/read",
        "params": {
            "uri": "figma://strategy/read_design_strategy",
            "_meta": modern_meta()
        }
    }));
    let contents = read["result"]["contents"]
        .as_array()
        .expect("resource contents");
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0]["uri"], "figma://strategy/read_design_strategy");
    assert_eq!(contents[0]["mimeType"], "text/markdown");
    assert!(
        contents[0]["text"]
            .as_str()
            .expect("resource text")
            .contains("get_design_context"),
        "the strategy body must arrive over real stdio"
    );

    server.terminate_sigterm();
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
