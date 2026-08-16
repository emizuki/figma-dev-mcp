use figma_dev_mcp_broker::{Broker, BrokerConfig, Limits};
use std::time::Duration;
use tokio::net::TcpListener;

use super::multi_client::connect_plugin;

#[tokio::test(start_paused = true)]
async fn idle_shutdown_waits_until_all_frontend_leases_are_gone_for_the_full_grace() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let frontend = broker.frontend_lease().unwrap();
    let waiting = tokio::spawn({
        let broker = broker.clone();
        async move { broker.wait_until_idle(Duration::from_secs(30)).await }
    });

    tokio::time::advance(Duration::from_secs(60)).await;
    assert!(!waiting.is_finished());

    drop(frontend);
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(29)).await;
    assert!(!waiting.is_finished());
    tokio::time::advance(Duration::from_secs(1)).await;
    waiting.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn activity_during_idle_grace_restarts_the_full_timer() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let waiting = tokio::spawn({
        let broker = broker.clone();
        async move { broker.wait_until_idle(Duration::from_secs(30)).await }
    });

    tokio::time::advance(Duration::from_secs(10)).await;
    let frontend = broker.frontend_lease().unwrap();
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(1)).await;
    drop(frontend);
    tokio::task::yield_now().await;

    tokio::time::advance(Duration::from_secs(29)).await;
    assert!(!waiting.is_finished());
    tokio::time::advance(Duration::from_secs(1)).await;
    waiting.await.unwrap();
}

#[tokio::test]
async fn a_live_plugin_blocks_idle_shutdown_and_disconnect_starts_a_new_grace() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(broker.clone().serve(listener));
    let plugin = connect_plugin(
        address,
        "123e4567-e89b-42d3-a456-426614174020",
        "Idle lifetime",
    )
    .await;
    while broker.live_file_count().await != 1 {
        tokio::task::yield_now().await;
    }

    let waiting = tokio::spawn({
        let broker = broker.clone();
        async move { broker.wait_until_idle(Duration::from_millis(20)).await }
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!waiting.is_finished());

    drop(plugin);
    while broker.live_file_count().await != 0 {
        tokio::task::yield_now().await;
    }
    tokio::time::timeout(Duration::from_millis(100), waiting)
        .await
        .expect("plugin disconnect must start a fresh idle grace")
        .unwrap();

    broker.shutdown().await;
    server.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn idle_shutdown_atomically_rejects_new_activity_after_grace() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let waiting = tokio::spawn({
        let broker = broker.clone();
        async move { broker.wait_until_idle(Duration::from_secs(30)).await }
    });

    tokio::time::advance(Duration::from_secs(30)).await;
    waiting.await.unwrap();
    assert!(
        broker.frontend_lease().is_none(),
        "committed idle shutdown must reject a racing frontend"
    );
}
