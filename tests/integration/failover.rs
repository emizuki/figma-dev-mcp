//! Leader failover: backend swapping and automatic re-election.

use std::time::Duration;

use figma_dev_mcp_broker::{
    Broker, BrokerClient, BrokerConfig, FrontendClient, Limits, OpenCall, Supervisor,
};

#[tokio::test]
async fn local_broker_resolves_through_the_swappable_cell() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let client = BrokerClient::local(broker.clone());

    assert!(
        client.local_broker().is_some(),
        "a local client must expose its Broker so cancellation can reach the plugin"
    );

    broker.shutdown().await;
}

#[tokio::test]
async fn a_remote_client_has_no_local_broker() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let server = tokio::spawn(broker.clone().serve_frontends(listener));

    let frontend = figma_dev_mcp_broker::FrontendClient::connect(address)
        .await
        .unwrap();
    let client = BrokerClient::remote(frontend);
    assert!(
        client.local_broker().is_none(),
        "a follower has no local Broker to cancel through"
    );

    broker.shutdown().await;
    let _ = tokio::time::timeout(Duration::from_secs(1), server).await;
}

#[tokio::test]
async fn frontend_client_closed_resolves_when_the_leader_goes_away() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let server = tokio::spawn(broker.clone().serve_frontends(listener));

    let frontend = figma_dev_mcp_broker::FrontendClient::connect(address)
        .await
        .unwrap();

    // Still alive: closed() must not resolve.
    assert!(
        tokio::time::timeout(Duration::from_millis(50), frontend.closed())
            .await
            .is_err(),
        "closed() must not resolve while the leader is serving"
    );

    // Kill the leader.
    broker.shutdown().await;
    let _ = tokio::time::timeout(Duration::from_secs(1), server).await;

    tokio::time::timeout(Duration::from_secs(2), frontend.closed())
        .await
        .expect("closed() must resolve once the leader's RPC connection ends");
}

/// Reserve two ports, then release them, so the supervisor can bind them.
async fn free_addresses() -> (std::net::SocketAddr, std::net::SocketAddr) {
    let plugin = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let frontend = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addresses = (plugin.local_addr().unwrap(), frontend.local_addr().unwrap());
    drop(plugin);
    drop(frontend);
    addresses
}

fn test_config(
    plugin_address: std::net::SocketAddr,
    frontend_address: std::net::SocketAddr,
) -> BrokerConfig {
    BrokerConfig {
        plugin_address,
        frontend_address,
        limits: Limits::reduced_for_test(),
    }
}

#[tokio::test]
async fn the_first_supervisor_leads_and_the_second_follows() {
    let (plugin_address, frontend_address) = free_addresses().await;
    let config = test_config(plugin_address, frontend_address);

    let leader = Supervisor::start(config.clone()).await;
    assert!(leader.is_leader(), "the first process must lead");
    assert!(
        leader.client().local_broker().is_some(),
        "a leader's client must be backed by a local Broker"
    );

    let follower = Supervisor::start(config).await;
    assert!(!follower.is_leader(), "the second process must follow");
    assert!(
        follower.client().local_broker().is_none(),
        "a follower's client must be backed by an RPC connection"
    );

    follower.shutdown(Duration::from_millis(0)).await.unwrap();
    leader.shutdown(Duration::from_millis(0)).await.unwrap();
}

#[tokio::test]
async fn a_leading_supervisor_binds_both_ports() {
    let (plugin_address, frontend_address) = free_addresses().await;
    let leader = Supervisor::start(test_config(plugin_address, frontend_address)).await;

    assert!(
        tokio::net::TcpStream::connect(plugin_address).await.is_ok(),
        "the leader must be listening on the plugin port"
    );
    assert!(
        tokio::net::TcpStream::connect(frontend_address)
            .await
            .is_ok(),
        "the leader must be listening on the frontend port"
    );

    leader.shutdown(Duration::from_millis(0)).await.unwrap();
}

#[tokio::test]
async fn a_follower_promotes_itself_when_the_leader_dies() {
    let (plugin_address, frontend_address) = free_addresses().await;
    let config = test_config(plugin_address, frontend_address);

    let leader = Supervisor::start(config.clone()).await;
    let mut follower = Supervisor::start(config).await;
    assert!(!follower.is_leader());

    // Take the client handle before the supervisor moves into its task. This is
    // the handle the MCP service holds for the life of the process, so it is
    // what proves `BrokerClient::install` actually swapped the backend
    // underneath a live session rather than merely rebinding the ports.
    let client = follower.client();
    assert!(
        client.local_broker().is_none(),
        "a follower's client must start out backed by an RPC connection"
    );

    let supervising = tokio::spawn(async move {
        follower.supervise().await;
    });

    // The leader dies without warning, as it does under SIGTERM. Dropping the
    // Supervisor is the faithful simulation: its JoinSet aborts the listener
    // tasks, which drops the TcpListeners and closes both ports.
    //
    // Do NOT use Supervisor::shutdown here. That is the graceful path, and it
    // waits in wait_until_idle while any frontend lease is outstanding — the
    // attached follower below holds one, so it would block forever. That the
    // graceful path holds the ports open is exactly why this bug only shows up
    // on abrupt death.
    drop(leader);

    // The survivor must reopen the plugin port on its own.
    //
    // Note: we deliberately do NOT assert an observable "port closed" window
    // before this. On this single-threaded test runtime, a fast (correct)
    // failover can close the dead listener and rebind the same port within
    // the same scheduling burst that our own polling loop yields into, so an
    // external connect-probe can legitimately never observe a gap — see the
    // conclusive check below for what actually proves re-election happened.
    let reopened = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if tokio::net::TcpStream::connect(plugin_address).await.is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await;
    assert!(
        reopened.is_ok(),
        "an orphaned follower must re-elect and reopen the plugin port"
    );

    // Reopening the port is not enough: `elect()` binds before `enter_role`
    // spawns the listeners and before the new backend is installed, so the
    // probe above can succeed a moment early. The swap itself is the mechanism
    // that keeps the MCP session alive, so wait for it and assert it happened.
    let swapped = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if client.local_broker().is_some() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await;
    assert!(
        swapped.is_ok(),
        "the promoted follower must install its new local Broker into the client \
         the MCP service already holds"
    );

    // Conclusive check: a fresh participant must find a leader to follow. If
    // the port merely reopened without a real election (impossible, but this
    // is the check that would catch it), a fresh Supervisor would win a second
    // election and become leader itself.
    let checker = Supervisor::start(test_config(plugin_address, frontend_address)).await;
    assert!(
        !checker.is_leader(),
        "a survivor must have actually promoted itself to leader"
    );
    checker.shutdown(Duration::from_millis(0)).await.unwrap();

    supervising.abort();
}

#[tokio::test]
async fn exactly_one_of_two_orphans_takes_the_ports() {
    let (plugin_address, frontend_address) = free_addresses().await;
    let config = test_config(plugin_address, frontend_address);

    let leader = Supervisor::start(config.clone()).await;
    let mut first = Supervisor::start(config.clone()).await;
    let mut second = Supervisor::start(config).await;

    let first_task = tokio::spawn(async move {
        first.supervise().await;
    });
    let second_task = tokio::spawn(async move {
        second.supervise().await;
    });

    // Abrupt death again — see the note in the previous test for why this is a
    // drop and not a shutdown.
    drop(leader);

    // Whoever wins, the port must come back exactly once and stay bound.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if tokio::net::TcpStream::connect(plugin_address).await.is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("one orphan must take the plugin port");

    // A third participant must now find a leader to follow, which is only true
    // if exactly one orphan bound the frontend port.
    let late = Supervisor::start(test_config(plugin_address, frontend_address)).await;
    assert!(
        !late.is_leader(),
        "a late starter must follow the promoted leader, not win a second election"
    );

    late.shutdown(Duration::from_millis(0)).await.unwrap();
    first_task.abort();
    second_task.abort();
}

#[tokio::test]
async fn election_retries_until_a_squatted_port_is_released() {
    let (plugin_address, frontend_address) = free_addresses().await;
    let config = test_config(plugin_address, frontend_address);

    let leader = Supervisor::start(config.clone()).await;
    let mut follower = Supervisor::start(config).await;

    // Something outside this project takes the plugin port the instant the
    // leader lets go of it, so every election attempt fails at the plugin bind.
    drop(leader);
    let squatter = loop {
        if let Ok(listener) = tokio::net::TcpListener::bind(plugin_address).await {
            break listener;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    };

    let supervising = tokio::spawn(async move {
        follower.supervise().await;
    });

    // The supervisor must keep retrying rather than giving up.
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(
        !supervising.is_finished(),
        "the supervisor must keep retrying while the port is unavailable"
    );

    // Once the squatter leaves, the next attempt must succeed.
    drop(squatter);
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if tokio::net::TcpStream::connect(frontend_address)
                .await
                .is_ok()
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("election must succeed once the port is free again");

    supervising.abort();
}

#[tokio::test]
async fn a_leader_whose_broker_dies_re_elects_and_rebinds() {
    let (plugin_address, frontend_address) = free_addresses().await;
    let mut leader = Supervisor::start(test_config(plugin_address, frontend_address)).await;
    assert!(leader.is_leader());

    // Broker::shutdown cancels the shared token, which is exactly what a failing
    // accept() does to the three listener tasks.
    leader.client().local_broker().unwrap().shutdown().await;

    let supervising = tokio::spawn(async move {
        leader.supervise().await;
    });

    // Note: we deliberately do NOT assert an observable "port closed" window
    // before this. On this single-threaded test runtime, a fast (correct)
    // failover can close the dead listener and rebind the same port within
    // the same scheduling burst that our own polling loop yields into, so an
    // external connect-probe can legitimately never observe a gap — see the
    // conclusive check below for what actually proves re-election happened.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if tokio::net::TcpStream::connect(plugin_address).await.is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("a leader whose broker died must re-elect and rebind the plugin port");

    // Conclusive check: a fresh participant must find a leader to follow.
    let checker = Supervisor::start(test_config(plugin_address, frontend_address)).await;
    assert!(
        !checker.is_leader(),
        "a survivor must have actually promoted itself to leader"
    );
    checker.shutdown(Duration::from_millis(0)).await.unwrap();

    supervising.abort();
}

// What this proves: `OpenCall.owner` is populated with the `Broker` that
// opened the call, and a different `Broker` cannot cancel it through
// `Broker::cancel`'s own routing (which was never broken).
//
// What this does NOT prove: it never swaps a backend, never calls
// `abort_open`, and never calls `BrokerClient::call`. It does not exercise the
// stale-cell defect those fixed. The consumer-side guarantee — that
// cancellation reaches the broker that opened the call even after a backend
// swap — is enforced by `abort_open`'s signature, which no longer accepts a
// `BrokerClient` and so cannot reach the swappable cell at all. That is a
// stronger guarantee than a test could give here; a test that actually swaps
// the backend would need to live inside `crates/broker` (`install` is
// `pub(crate)`) with a fake plugin duplicated into that crate, which is a
// follow-up rather than part of this test.
#[tokio::test]
async fn an_open_call_records_the_broker_that_opened_it() {
    use super::multi_client::{connect_plugin, metadata_call};
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::Message;

    let connection_id = "123e4567-e89b-42d3-a456-426614174090";
    let opener = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let plugin_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plugin_address = plugin_listener.local_addr().unwrap();
    let plugin_server = tokio::spawn(opener.clone().serve(plugin_listener));
    let mut plugin = connect_plugin(plugin_address, connection_id, "Cancel routing").await;
    while opener.live_file_count().await != 1 {
        tokio::task::yield_now().await;
    }

    let client = BrokerClient::local(opener.clone());
    let open = client
        .open(metadata_call(Some(connection_id)))
        .await
        .unwrap();
    let request_id = open
        .request_id
        .clone()
        .expect("an invoke carries a request id");
    let call_connection = open
        .connection_id
        .clone()
        .expect("an invoke carries a connection id");

    // Drain the request the plugin just received, so the next frame we read is
    // whatever the cancellation produces.
    loop {
        let Message::Text(text) = plugin.next().await.unwrap().unwrap() else {
            continue;
        };
        if matches!(
            serde_json::from_str::<figma_dev_mcp_protocol::wire::BrokerToPlugin>(&text).unwrap(),
            figma_dev_mcp_protocol::wire::BrokerToPlugin::Request(_)
        ) {
            break;
        }
    }

    // A different Broker must not be able to cancel this call. This is the
    // defect: `abort_open` used to re-read the backend cell, so after a swap it
    // cancelled through whichever Broker was current rather than this one, and
    // the plugin kept executing an abandoned request to its own deadline.
    let stranger = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    assert!(
        !stranger.cancel(&call_connection, &request_id).await,
        "a Broker that never opened the call must not be able to cancel it"
    );

    // The owner recorded on the OpenCall must be able to.
    let owner = open.owner.clone().expect("a local call records its Broker");
    assert!(
        owner.cancel(&call_connection, &request_id).await,
        "the Broker that opened the call must cancel it"
    );

    let cancelled = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let Message::Text(text) = plugin.next().await.unwrap().unwrap() else {
                continue;
            };
            if let figma_dev_mcp_protocol::wire::BrokerToPlugin::Cancel(frame) =
                serde_json::from_str(&text).unwrap()
            {
                return frame;
            }
        }
    })
    .await
    .expect("the plugin must receive a Cancel frame");
    assert_eq!(cancelled.request_id, request_id);

    drop(plugin);
    opener.shutdown().await;
    let _ = tokio::time::timeout(Duration::from_secs(1), plugin_server).await;
}

#[tokio::test]
async fn an_unattached_client_fails_calls_retryably() {
    let client = BrokerClient::unattached();

    assert!(
        client.local_broker().is_none(),
        "an unattached client has no local Broker"
    );

    let open = client
        .open(figma_dev_mcp_protocol::wire::BrokerCall::ListFiles {})
        .await;
    let error = open.expect_err("an unattached client cannot open a call");
    assert_eq!(
        error.code(),
        figma_dev_mcp_protocol::error::ErrorCode::ConnectionLost
    );
    assert!(
        error.retryable(),
        "the window before the first election is transient, so the error must be retryable"
    );

    let called = client
        .call(
            figma_dev_mcp_protocol::wire::BrokerCall::ListFiles {},
            &tokio_util::sync::CancellationToken::new(),
        )
        .await;
    let error = called.expect_err("an unattached client cannot serve a call");
    assert_eq!(
        error.code(),
        figma_dev_mcp_protocol::error::ErrorCode::ConnectionLost
    );
    assert!(error.retryable());
}

#[tokio::test]
async fn a_supervisor_built_unattached_elects_inside_supervise() {
    let (plugin_address, frontend_address) = free_addresses().await;
    let mut supervisor = Supervisor::new(test_config(plugin_address, frontend_address));

    let client = supervisor.client();
    assert!(
        client.local_broker().is_none(),
        "new() must not elect — the client starts unattached"
    );
    assert!(
        tokio::net::TcpStream::connect(plugin_address)
            .await
            .is_err(),
        "new() must not bind the plugin port"
    );

    let supervising = tokio::spawn(async move {
        supervisor.supervise().await;
    });

    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if client.local_broker().is_some() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("supervise() must run the first election when it has no role");

    assert!(
        tokio::net::TcpStream::connect(plugin_address).await.is_ok(),
        "the elected leader must bind the plugin port"
    );

    supervising.abort();
}

#[tokio::test]
async fn a_detached_client_stops_answering_through_its_dead_broker() {
    use super::multi_client::connect_plugin;

    let connection_id = "123e4567-e89b-42d3-a456-426614174091";
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let plugin_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plugin_address = plugin_listener.local_addr().unwrap();
    let plugin_server = tokio::spawn(broker.clone().serve(plugin_listener));
    let plugin = connect_plugin(plugin_address, connection_id, "Detach").await;
    while broker.live_file_count().await != 1 {
        tokio::task::yield_now().await;
    }

    let client = BrokerClient::local(broker.clone());

    // Shutting the broker down does NOT clear its SessionRegistry, so a client
    // still holding it keeps answering with stale data. This half is the proof
    // that the defect was real — without it the assertion below could pass for
    // the wrong reason.
    //
    // This assertion is deterministic only because this test runs on the
    // default current-thread `#[tokio::test]` runtime: the plugin task spawned
    // above and the `client.call` below never actually run concurrently, so
    // the plugin's own `cleanup_socket` (which removes this session from the
    // registry once its connection ends) cannot run ahead of the call. Under
    // `flavor = "multi_thread"` those two could race on separate OS threads,
    // and the file count below could come back 0 instead of the stale 1. Do
    // not add a multi-thread flavor to this test.
    broker.shutdown().await;
    let served = client
        .call(
            figma_dev_mcp_protocol::wire::BrokerCall::ListFiles {},
            &tokio_util::sync::CancellationToken::new(),
        )
        .await
        .expect("a shut-down broker that is still installed keeps answering");
    let figma_dev_mcp_protocol::wire::BrokerResult::Files { result } = served else {
        panic!("list_files must return a file list");
    };
    assert_eq!(
        result.files.as_slice().len(),
        1,
        "the dead broker still serves its stale registry, which is the defect"
    );

    // Detaching is what turns that confident wrong answer into an honest one.
    client.detach();
    assert!(
        client.local_broker().is_none(),
        "a detached client has no local Broker"
    );
    let error = client
        .call(
            figma_dev_mcp_protocol::wire::BrokerCall::ListFiles {},
            &tokio_util::sync::CancellationToken::new(),
        )
        .await
        .expect_err("a detached client must not answer");
    assert_eq!(
        error.code(),
        figma_dev_mcp_protocol::error::ErrorCode::ConnectionLost
    );
    assert!(
        error.retryable(),
        "the window between a death and the next election is transient, so the error must be retryable"
    );

    drop(plugin);
    let _ = tokio::time::timeout(Duration::from_secs(1), plugin_server).await;
}

/// A bare `OpenCall`, built directly rather than through a broker or an RPC
/// connection, so `Drop`'s two branches can be exercised without either.
/// Every field on `OpenCall` is `pub` for exactly this reason.
fn bare_open_call(
    abort: tokio_util::sync::CancellationToken,
    watcher: tokio::task::JoinHandle<()>,
) -> OpenCall {
    let (_sender, result) = tokio::sync::oneshot::channel();
    let (_progress_tx, progress) = tokio::sync::mpsc::channel(1);
    OpenCall {
        result,
        progress,
        total_deadline: tokio::time::Instant::now(),
        inactivity_timeout: Duration::from_secs(0),
        connection_id: None,
        request_id: None,
        abort,
        owner: None,
        watcher: Some(watcher),
    }
}

// `OpenCall::drop` aborts its watcher only when the abort token was never
// cancelled. This test drives both branches directly against a hand-built
// `OpenCall`, with nothing else holding the abort token or the join handle.
// It does NOT exercise `FrontendClient::open` itself — that is covered
// end-to-end by
// `frontend_client_open_drop_governs_the_remote_watcher_on_both_branches` in
// this file, which is the only test in the repo that constructs an `OpenCall`
// through a real remote call rather than by hand. Both halves matter here
// too: the first is what actually prevents the leak (an abandoned watcher
// sleeping until a dead call's own deadline), and the second is what stops a
// leak fix from swallowing a real cancellation before it can send its
// `Cancel` frame.
#[tokio::test]
async fn dropping_an_open_call_aborts_its_watcher_unless_already_cancelled() {
    use tokio_util::sync::CancellationToken;

    // Not cancelled: the leak-preventing branch. Nothing but this `OpenCall`
    // holds the abort token or the join handle, so if `Drop` did not abort the
    // watcher here, nothing ever would — it would sleep out its full 30
    // seconds for no reason.
    let handle = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(30)).await });
    let abort_handle = handle.abort_handle();
    let open = bare_open_call(CancellationToken::new(), handle);
    drop(open);
    for _ in 0..100 {
        if abort_handle.is_finished() {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(
        abort_handle.is_finished(),
        "dropping an OpenCall whose abort token was never cancelled must abort its watcher"
    );

    // Cancelled: on the remote path the watcher is the only thing that sends
    // the plugin its `Cancel` frame, so `Drop` must leave it running. This
    // assertion is not vacuous: if the `!self.abort.is_cancelled()` guard were
    // ever dropped from `Drop`, this watcher would be aborted the same way the
    // one above was, and `is_finished()` would flip to `true` here too.
    let handle = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(30)).await });
    let abort_handle = handle.abort_handle();
    let abort = CancellationToken::new();
    abort.cancel();
    let open = bare_open_call(abort, handle);
    drop(open);
    tokio::task::yield_now().await;
    assert!(
        !abort_handle.is_finished(),
        "dropping an OpenCall whose abort token was already cancelled must not abort its watcher"
    );
    abort_handle.abort();
}

/// The first end-to-end coverage of `FrontendClient::open` in this repo: a
/// real leader `Broker`, a real fake plugin, and a real RPC connection,
/// rather than a hand-built `OpenCall`. Every other remote-client test only
/// issues `list_files`, which routes through `BrokerClient::call` →
/// `FrontendClient::call` — a different method that builds its own command
/// and handles cancellation inline in its own `select!` arm, never
/// constructing an `OpenCall` or spawning a watcher. `open` is the path every
/// non-`list_files` tool call on a follower takes, and the only path that
/// creates a watcher, so this is also the first test that can catch a
/// regression in either branch of `OpenCall::drop` on the code that actually
/// ships it.
#[tokio::test]
async fn frontend_client_open_drop_governs_the_remote_watcher_on_both_branches() {
    use super::multi_client::{
        connect_plugin, metadata_call, next_plugin_request, send_metadata_response,
    };

    let connection_id = "123e4567-e89b-42d3-a456-426614174093";
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let plugin_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plugin_address = plugin_listener.local_addr().unwrap();
    let plugin_server = tokio::spawn(broker.clone().serve(plugin_listener));
    let mut plugin = connect_plugin(plugin_address, connection_id, "OpenWatcher").await;
    while broker.live_file_count().await == 0 {
        tokio::task::yield_now().await;
    }

    let frontend_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let frontend_address = frontend_listener.local_addr().unwrap();
    let rpc_server = tokio::spawn(broker.clone().serve_frontends(frontend_listener));
    let frontend = FrontendClient::connect(frontend_address).await.unwrap();
    let client = BrokerClient::remote(frontend);

    // Branch A (the leak fix): let a real remote `open()` finish normally,
    // then drop it. Nothing else holds `abort` or the watcher's `JoinHandle`
    // here — if `Drop` did not abort the watcher, it would sleep out its full
    // deadline for no reason on every single non-`list_files` call a follower
    // ever makes.
    let mut open = client
        .open(metadata_call(Some(connection_id)))
        .await
        .expect("open must succeed against a connected plugin");
    let watcher_abort_handle = open
        .watcher
        .as_ref()
        .expect("FrontendClient::open must always spawn a watcher")
        .abort_handle();
    let request = next_plugin_request(&mut plugin).await;
    send_metadata_response(&mut plugin, &request.request_id, "OpenWatcher").await;
    (&mut open.result)
        .await
        .expect("the response sender must not be dropped mid-call")
        .expect("a real metadata response must resolve the call successfully");
    drop(open);
    let mut watcher_finished = false;
    for _ in 0..100 {
        if watcher_abort_handle.is_finished() {
            watcher_finished = true;
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(
        watcher_finished,
        "dropping a finished OpenCall must abort its now-useless watcher"
    );

    // Branch B (must not swallow a real cancel): cancel a second call before
    // dropping it, and prove the plugin actually receives the `Cancel` frame
    // naming that request — it is the watcher, not `Drop`, that must send it.
    let open = client
        .open(metadata_call(Some(connection_id)))
        .await
        .expect("open must succeed against a connected plugin");
    let request = next_plugin_request(&mut plugin).await;
    open.abort.cancel();
    drop(open);
    use futures_util::StreamExt;
    let cancelled_request_id = loop {
        let message = plugin.next().await.unwrap().unwrap();
        let tokio_tungstenite::tungstenite::Message::Text(text) = message else {
            continue;
        };
        if let figma_dev_mcp_protocol::wire::BrokerToPlugin::Cancel(cancel) =
            serde_json::from_str(&text).unwrap()
        {
            break cancel.request_id;
        }
    };
    assert_eq!(
        cancelled_request_id, request.request_id,
        "the watcher must send the plugin a Cancel naming the request it opened"
    );

    drop(plugin);
    broker.shutdown().await;
    let _ = tokio::time::timeout(Duration::from_secs(1), plugin_server).await;
    let _ = tokio::time::timeout(Duration::from_secs(1), rpc_server).await;
}

/// A fake leader that completes the frontend handshake and then hangs up.
///
/// Enough for a real `FrontendClient::from_stream` to succeed
/// (`crates/broker/src/rpc.rs:239-266`), so the supervisor elects, installs a
/// follower role, and then immediately sees it die. That is a genuine spin
/// driven entirely through production code paths — no fault injection, no
/// test-only constructor, no widened API.
///
/// Counts accepted connections, which is the supervisor's cycle count.
async fn handshake_then_hangup(
    listener: tokio::net::TcpListener,
    accepts: std::sync::Arc<std::sync::atomic::AtomicUsize>,
) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    loop {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        accepts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        // Read the client's `FrontendHello`: a 4-byte big-endian length followed
        // by that many bytes of JSON. `LengthDelimitedCodec` is built with
        // `length_field_length(4).big_endian()` (crates/broker/src/rpc.rs:27-33).
        let mut length = [0u8; 4];
        if socket.read_exact(&mut length).await.is_err() {
            continue;
        }
        let mut body = vec![0u8; u32::from_be_bytes(length) as usize];
        if socket.read_exact(&mut body).await.is_err() {
            continue;
        }

        // `FrontendHandshake::Ready`. The enum carries `#[serde(tag = "type",
        // rename_all = "camelCase")]` (crates/protocol/src/rpc.rs:19-29), so the
        // unit variant serialises as exactly this object.
        let reply = br#"{"type":"ready"}"#;
        if socket
            .write_all(&(reply.len() as u32).to_be_bytes())
            .await
            .is_err()
        {
            continue;
        }
        if socket.write_all(reply).await.is_err() {
            continue;
        }
        // Flush before hanging up, so the handshake reply is not lost with the
        // socket. The follower's `client_loop` then sees the connection close,
        // `FrontendClient::closed()` resolves, and the role dies on arrival.
        let _ = socket.flush().await;
        drop(socket);
    }
}

/// The window the cycle count is measured over.
const SPIN_WINDOW: Duration = Duration::from_secs(5);
/// The most accepts a correctly escalating supervisor may produce in
/// `SPIN_WINDOW`. The curve gives 0, 100ms, 200ms, 400ms, 800ms, 1.6s, 3.2s,
/// which reaches roughly seven accepts in five seconds. Without escalation the
/// 100ms floor alone permits about fifty. This bound sits between them: close
/// enough to catch a lost escalation, far enough above the escalated rate that a
/// loaded machine cannot reach it.
///
/// Note the direction. Escalation can only make the observed count SMALLER, so
/// machine load can never fail this assertion spuriously — the failure it
/// detects is "cycled too fast", which load cannot cause.
const MAX_SPIN_CYCLES: usize = 10;

#[tokio::test]
async fn a_role_that_dies_on_arrival_recycles_slower_and_slower() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let (plugin_address, frontend_address) = free_addresses().await;

    // Squat the frontend port before the supervisor starts, so every election
    // takes the follower branch.
    let fake_leader = tokio::net::TcpListener::bind(frontend_address)
        .await
        .unwrap();
    let accepts = Arc::new(AtomicUsize::new(0));
    let fake = tokio::spawn(handshake_then_hangup(fake_leader, Arc::clone(&accepts)));

    let mut supervisor = Supervisor::new(test_config(plugin_address, frontend_address));
    let supervising = tokio::spawn(async move {
        supervisor.supervise().await;
    });

    tokio::time::sleep(SPIN_WINDOW).await;
    let cycles = accepts.load(Ordering::SeqCst);

    supervising.abort();
    fake.abort();

    // Lower bound first: without it this test would pass if the supervisor never
    // reached the fake leader at all, which is the way a rate assertion usually
    // rots into one that cannot fail.
    assert!(
        cycles >= 2,
        "the supervisor must actually be recycling for this bound to mean anything; \
         saw {cycles} accepts in {SPIN_WINDOW:?}"
    );
    assert!(
        cycles <= MAX_SPIN_CYCLES,
        "a role dying on arrival must recycle more slowly each time; saw {cycles} \
         accepts in {SPIN_WINDOW:?}, which is the un-escalated 100ms floor rate"
    );
}

#[tokio::test]
async fn a_swapped_backend_does_not_steal_a_calls_cancellation() {
    use super::multi_client::{connect_plugin, metadata_call};
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::Message;
    use tokio_util::sync::CancellationToken;

    let connection_id = "123e4567-e89b-42d3-a456-426614174092";
    let opener = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let plugin_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plugin_address = plugin_listener.local_addr().unwrap();
    let plugin_server = tokio::spawn(opener.clone().serve(plugin_listener));
    let mut plugin = connect_plugin(plugin_address, connection_id, "Swap routing").await;
    while opener.live_file_count().await != 1 {
        tokio::task::yield_now().await;
    }

    let client = BrokerClient::local(opener.clone());
    let token = CancellationToken::new();
    let calling = tokio::spawn({
        let client = client.clone();
        let token = token.clone();
        async move {
            client
                .call(metadata_call(Some(connection_id)), &token)
                .await
        }
    });

    // Wait until the plugin actually holds the request, so the call has an owner
    // recorded and a request id the Cancel can name.
    let request_id = loop {
        let Message::Text(text) = plugin.next().await.unwrap().unwrap() else {
            continue;
        };
        if let figma_dev_mcp_protocol::wire::BrokerToPlugin::Request(frame) =
            serde_json::from_str(&text).unwrap()
        {
            break frame.request_id;
        }
    };

    // The step no existing test performs: swap the client's backend while the
    // call is in flight, exactly as a re-election does. `BrokerClient::call`'s
    // cancellation arm must still cancel through `open.owner` — the Broker
    // recorded at open time — and not through whatever the cell now holds.
    let stranger = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    client.install_local(stranger.clone());

    token.cancel();

    let cancelled = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let Message::Text(text) = plugin.next().await.unwrap().unwrap() else {
                continue;
            };
            if let figma_dev_mcp_protocol::wire::BrokerToPlugin::Cancel(frame) =
                serde_json::from_str(&text).unwrap()
            {
                return frame;
            }
        }
    })
    .await
    .expect(
        "the Cancel must reach the plugin of the Broker that opened the call, not the \
         Broker the client was swapped to",
    );
    assert_eq!(cancelled.request_id, request_id);

    let error = calling
        .await
        .unwrap()
        .expect_err("a cancelled call must not return a result");
    assert_eq!(
        error.code(),
        figma_dev_mcp_protocol::error::ErrorCode::Cancelled
    );

    drop(plugin);
    opener.shutdown().await;
    stranger.shutdown().await;
    let _ = tokio::time::timeout(Duration::from_secs(1), plugin_server).await;
}
