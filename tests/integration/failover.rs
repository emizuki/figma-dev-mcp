//! Leader failover: backend swapping and automatic re-election.

use std::time::Duration;

use figma_dev_mcp_broker::{Broker, BrokerClient, BrokerConfig, Limits, Supervisor};

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
