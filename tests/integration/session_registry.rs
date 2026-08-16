use std::time::{Duration, SystemTime};

use figma_dev_mcp_broker::registry::{Selection, Session, SessionRegistry};
use figma_dev_mcp_broker::{PendingMap, PendingResult};
use figma_dev_mcp_protocol::domain::ConnectionId;
use figma_dev_mcp_protocol::wire::{BrokerToPlugin, Ping};
use tokio::sync::mpsc;
use uuid::Uuid;

mod fake_plugin;

fn session(connection_id: &str, file_name: &str, connected_at: SystemTime) -> Session {
    let (outbound, _receiver) = mpsc::channel(4);
    Session::from_hello(
        fake_plugin::hello(connection_id, file_name),
        Uuid::new_v4(),
        connected_at,
        outbound,
    )
}

#[test]
fn selection_requires_exactly_one_live_session_and_never_falls_back() {
    let now = tokio::time::Instant::now();
    let mut registry = SessionRegistry::new(now);
    let first = session(
        "123e4567-e89b-42d3-a456-426614174000",
        "Same file",
        SystemTime::UNIX_EPOCH,
    );
    let first_id = first.connection_id.clone();
    registry.insert(first).unwrap();

    assert!(matches!(registry.select(None), Selection::One(_)));
    assert!(matches!(
        registry.select(Some(&first_id)),
        Selection::One(_)
    ));
    assert!(matches!(
        registry.select(Some(
            &ConnectionId::try_from("123e4567-e89b-42d3-a456-426614174001").unwrap()
        )),
        Selection::Missing
    ));

    registry
        .insert(session(
            "123e4567-e89b-42d3-a456-426614174001",
            "Same file",
            SystemTime::UNIX_EPOCH + Duration::from_secs(1),
        ))
        .unwrap();
    assert!(matches!(registry.select(None), Selection::Ambiguous));
}

#[test]
fn duplicate_connection_id_is_rejected_but_duplicate_file_name_is_allowed() {
    let now = tokio::time::Instant::now();
    let mut registry = SessionRegistry::new(now);
    registry
        .insert(session(
            "123e4567-e89b-42d3-a456-426614174000",
            "Same file",
            SystemTime::UNIX_EPOCH,
        ))
        .unwrap();
    assert!(
        registry
            .insert(session(
                "123e4567-e89b-42d3-a456-426614174000",
                "Other display",
                SystemTime::UNIX_EPOCH,
            ))
            .is_err()
    );
    registry
        .insert(session(
            "123e4567-e89b-42d3-a456-426614174001",
            "Same file",
            SystemTime::UNIX_EPOCH + Duration::from_secs(1),
        ))
        .unwrap();
    let files = registry.list_files();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].file_name.as_str(), "Same file");
}

#[test]
fn outbound_queue_reports_full_without_waiting() {
    let now = tokio::time::Instant::now();
    let mut registry = SessionRegistry::new(now);
    let (outbound, _receiver) = mpsc::channel(1);
    let session = Session::from_hello(
        fake_plugin::hello("123e4567-e89b-42d3-a456-426614174000", "Queue"),
        Uuid::new_v4(),
        SystemTime::UNIX_EPOCH,
        outbound,
    );
    let connection_id = session.connection_id.clone();
    registry.insert(session).unwrap();
    let socket_id = match registry.select(Some(&connection_id)) {
        Selection::One(session) => session.socket_id,
        _ => panic!("registered session must be selectable"),
    };
    assert!(
        registry
            .try_send_to(
                &connection_id,
                Uuid::new_v4(),
                BrokerToPlugin::Ping(Ping { nonce: 0 }),
            )
            .is_err()
    );
    registry
        .try_send_to(
            &connection_id,
            socket_id,
            BrokerToPlugin::Ping(Ping { nonce: 1 }),
        )
        .unwrap();
    assert!(
        registry
            .try_send_to(
                &connection_id,
                socket_id,
                BrokerToPlugin::Ping(Ping { nonce: 2 }),
            )
            .is_err()
    );
}

#[test]
fn stale_sessions_expire_and_socket_removal_is_idempotent() {
    let start = tokio::time::Instant::now();
    let mut registry = SessionRegistry::new(start);
    let session = session(
        "123e4567-e89b-42d3-a456-426614174000",
        "Stale",
        SystemTime::UNIX_EPOCH,
    );
    let socket_id = session.socket_id;
    registry.insert(session).unwrap();
    registry.expire_stale(start + Duration::from_secs(20));
    assert!(matches!(registry.select(None), Selection::None));
    assert!(!registry.remove_socket(socket_id));
}

#[test]
fn production_config_is_fixed_loopback_and_test_limits_cannot_raise_ceilings() {
    let config = figma_dev_mcp_broker::BrokerConfig::production();
    assert_eq!(config.plugin_address.to_string(), "127.0.0.1:3056");
    assert_eq!(config.frontend_address.to_string(), "127.0.0.1:3057");
    assert!(
        figma_dev_mcp_broker::Limits::checked(
            figma_dev_mcp_protocol::limits::MAX_ENVELOPE_BYTES + 1,
            1,
            1,
            Duration::from_millis(1),
            Duration::from_millis(1),
        )
        .is_err()
    );
}

#[tokio::test]
async fn pending_completion_is_bound_to_socket_identity() {
    let mut pending = PendingMap::default();
    let request_id = figma_dev_mcp_protocol::domain::RequestId::try_from("request-1").unwrap();
    let socket_a = Uuid::new_v4();
    let socket_b = Uuid::new_v4();
    let mut receiver = pending
        .insert(
            socket_a,
            request_id.clone(),
            SystemTime::now(),
            tokio::time::Instant::now() + Duration::from_secs(1),
        )
        .unwrap();

    assert!(!pending.complete(
        socket_b,
        &request_id,
        Err(figma_dev_mcp_protocol::error::ToolError::new(
            figma_dev_mcp_protocol::error::ErrorCode::InternalError,
            false,
        ))
    ));
    assert!(matches!(
        receiver.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
    assert!(pending.cancel(socket_a, &request_id));
    let result: PendingResult = receiver.await.unwrap();
    assert!(result.is_err());
}

#[tokio::test]
async fn pending_timeout_socket_close_and_shutdown_remove_each_entry_once() {
    let mut pending = PendingMap::default();
    let socket = Uuid::new_v4();
    let request_timeout = figma_dev_mcp_protocol::domain::RequestId::try_from("timeout").unwrap();
    let request_close = figma_dev_mcp_protocol::domain::RequestId::try_from("close").unwrap();
    let request_shutdown = figma_dev_mcp_protocol::domain::RequestId::try_from("shutdown").unwrap();
    let timeout_receiver = pending
        .insert(
            socket,
            request_timeout,
            SystemTime::now(),
            tokio::time::Instant::now(),
        )
        .unwrap();
    let close_receiver = pending
        .insert(
            socket,
            request_close,
            SystemTime::now(),
            tokio::time::Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
    let shutdown_receiver = pending
        .insert(
            Uuid::new_v4(),
            request_shutdown,
            SystemTime::now(),
            tokio::time::Instant::now() + Duration::from_secs(1),
        )
        .unwrap();

    assert_eq!(pending.expire(tokio::time::Instant::now()), 1);
    assert_eq!(pending.expire(tokio::time::Instant::now()), 0);
    assert_eq!(pending.remove_socket(socket), 1);
    assert_eq!(pending.remove_socket(socket), 0);
    assert_eq!(pending.shutdown(), 1);
    assert_eq!(pending.shutdown(), 0);
    assert!(timeout_receiver.await.unwrap().is_err());
    assert!(close_receiver.await.unwrap().is_err());
    assert!(shutdown_receiver.await.unwrap().is_err());
}
