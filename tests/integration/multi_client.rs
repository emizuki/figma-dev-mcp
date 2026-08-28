use figma_dev_mcp_broker::{
    Broker, BrokerClient, BrokerConfig, BrokerError, FrontendClient, Limits,
    PLUGIN_PROTOCOL_VERSION,
};
use figma_dev_mcp_protocol::{
    domain::{ConnectionId, GetMetadataInput, RequestId},
    error::ErrorCode,
    rpc::{
        FRONTEND_PROTOCOL_VERSION, FrontendHandshake, FrontendToLeader, RpcRequestId, encode_frame,
    },
    wire::{BrokerCall, BrokerResult, BrokerToPlugin, Invocation, ReadOperation},
};
use figma_dev_mcp_tools::McpService;
use futures_util::{SinkExt, StreamExt};
use rmcp::ServiceExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use tokio_util::sync::CancellationToken;

pub(super) type PluginSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub(super) async fn connect_plugin(
    address: std::net::SocketAddr,
    connection_id: &str,
    file_name: &str,
) -> PluginSocket {
    let mut request = format!("ws://{address}/").into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Origin", "null".parse().unwrap());
    let (mut plugin, _) = connect_async(request).await.unwrap();
    plugin
        .send(Message::Text(
            serde_json::json!({
                "type": "hello", "protocolVersion": PLUGIN_PROTOCOL_VERSION, "connectionId": connection_id,
                "displayName": file_name, "fileName": file_name,
                "currentPage": {"id": "0:1", "name": "Page 1"},
                "editorType": "dev", "pluginVersion": "0.1.0", "capabilities": {}
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
    plugin
}

async fn next_plugin_request(plugin: &mut PluginSocket) -> figma_dev_mcp_protocol::wire::Request {
    loop {
        let message = plugin.next().await.unwrap().unwrap();
        let Message::Text(text) = message else {
            continue;
        };
        if let BrokerToPlugin::Request(request) = serde_json::from_str(&text).unwrap() {
            return request;
        }
    }
}

async fn send_metadata_response(plugin: &mut PluginSocket, request_id: &RequestId, name: &str) {
    plugin
        .send(Message::Text(
            serde_json::json!({
                "type": "response", "requestId": request_id.as_str(),
                "result": {
                    "operation": "get_metadata",
                    "result": {
                        "file": {"name": name, "editorType": "dev"},
                        "pages": [{"id": "0:1", "name": "Page 1"}],
                        "currentPageId": "0:1", "pluginVersion": "0.1.0",
                        "capabilities": {}, "truncated": false,
                        "observation": {"startedAt": "1", "completedAt": "2"}
                    }
                }
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
}

async fn raw_frontend(address: std::net::SocketAddr) -> TcpStream {
    let mut stream = TcpStream::connect(address).await.unwrap();
    let hello = serde_json::json!({
        // The frontend RPC link carries its own version, which is not the
        // plugin socket's and does not move with it.
        "protocolVersion": FRONTEND_PROTOCOL_VERSION,
        "frontendId": "123e4567-e89b-42d3-a456-426614174010"
    });
    stream
        .write_all(&encode_frame(&hello).unwrap())
        .await
        .unwrap();
    let body = read_raw_frame(&mut stream).await;
    assert_eq!(
        serde_json::from_slice::<FrontendHandshake>(&body).unwrap(),
        FrontendHandshake::Ready
    );
    stream
}

async fn read_raw_frame(stream: &mut TcpStream) -> Vec<u8> {
    let mut prefix = [0_u8; 4];
    stream.read_exact(&mut prefix).await.unwrap();
    let mut body = vec![0_u8; u32::from_be_bytes(prefix) as usize];
    stream.read_exact(&mut body).await.unwrap();
    body
}

pub(super) fn metadata_call(connection_id: Option<&str>) -> BrokerCall {
    BrokerCall::Invoke {
        connection_id: connection_id.map(|value| ConnectionId::try_from(value).unwrap()),
        invocation: Box::new(Invocation {
            operation: ReadOperation::GetMetadata(GetMetadataInput {
                connection_id: None,
            }),
        }),
    }
}

fn metadata_name(result: BrokerResult) -> String {
    let BrokerResult::Invocation {
        result: figma_dev_mcp_protocol::wire::ReadResult::GetMetadata(metadata),
    } = result
    else {
        panic!("call returned the wrong result variant");
    };
    metadata.file.name.as_str().to_owned()
}

#[tokio::test]
async fn two_frontends_share_one_leader_registry() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(broker.clone().serve_frontends(listener));

    let first = FrontendClient::connect(address).await.unwrap();
    let second = FrontendClient::connect(address).await.unwrap();
    assert_eq!(first.list_files().await.unwrap(), broker.list_files().await);
    assert_eq!(
        second.list_files().await.unwrap(),
        broker.list_files().await
    );

    drop(first);
    drop(second);
    broker.shutdown().await;
    server.await.unwrap().unwrap();
}

#[tokio::test]
async fn remote_broker_client_can_back_an_mcp_frontend() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let rpc_server = tokio::spawn(broker.clone().serve_frontends(listener));

    let remote = BrokerClient::remote(FrontendClient::connect(address).await.unwrap());
    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let mcp_server = tokio::spawn(async move { McpService::new(remote).serve(server_io).await });
    let client = ().serve(client_io).await.unwrap();
    let mcp_server = mcp_server.await.unwrap().unwrap();

    let files = client
        .call_tool(rmcp::model::CallToolRequestParams::new("list_files"))
        .await
        .unwrap();
    assert_eq!(
        files.structured_content.unwrap()["files"],
        serde_json::json!([])
    );

    client.cancel().await.unwrap();
    mcp_server.waiting().await.unwrap();
    broker.shutdown().await;
    rpc_server.await.unwrap().unwrap();
}

#[tokio::test]
async fn frontend_handshake_rejects_protocol_mismatch_before_registering_lease() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let rpc_server = tokio::spawn(broker.clone().serve_frontends(listener));

    let mut stream = TcpStream::connect(address).await.unwrap();
    let hello = serde_json::json!({
        "protocolVersion": "999",
        "frontendId": "123e4567-e89b-42d3-a456-426614174000"
    });
    stream
        .write_all(&encode_frame(&hello).unwrap())
        .await
        .unwrap();

    let mut prefix = [0_u8; 4];
    stream.read_exact(&mut prefix).await.unwrap();
    let mut body = vec![0_u8; u32::from_be_bytes(prefix) as usize];
    stream.read_exact(&mut body).await.unwrap();
    let handshake: figma_dev_mcp_protocol::rpc::FrontendHandshake =
        serde_json::from_slice(&body).unwrap();
    assert!(
        matches!(handshake, figma_dev_mcp_protocol::rpc::FrontendHandshake::Rejected { error } if error.code() == ErrorCode::ProtocolMismatch)
    );

    assert_eq!(broker.live_file_count().await, 0);
    broker.shutdown().await;
    rpc_server.await.unwrap().unwrap();
}

#[tokio::test]
async fn cancelling_a_frontend_call_cancels_the_matching_plugin_request() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let plugin_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plugin_address = plugin_listener.local_addr().unwrap();
    let plugin_server = tokio::spawn(broker.clone().serve(plugin_listener));
    let mut request = format!("ws://{plugin_address}/")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("Origin", "null".parse().unwrap());
    let (mut plugin, _) = connect_async(request).await.unwrap();
    plugin
        .send(Message::Text(
            serde_json::json!({
                "type": "hello", "protocolVersion": PLUGIN_PROTOCOL_VERSION,
                "connectionId": "123e4567-e89b-42d3-a456-426614174000",
                "displayName": "Checkout", "fileName": "Checkout",
                "currentPage": {"id": "0:1", "name": "Page 1"},
                "editorType": "dev", "pluginVersion": "0.1.0", "capabilities": {}
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
    while broker.live_file_count().await == 0 {
        tokio::task::yield_now().await;
    }

    let frontend_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let frontend_address = frontend_listener.local_addr().unwrap();
    let rpc_server = tokio::spawn(broker.clone().serve_frontends(frontend_listener));
    let frontend = FrontendClient::connect(frontend_address).await.unwrap();
    let cancellation = CancellationToken::new();
    let call_task = tokio::spawn({
        let frontend = frontend.clone();
        let cancellation = cancellation.clone();
        async move {
            frontend
                .call(
                    BrokerCall::Invoke {
                        connection_id: Some(
                            ConnectionId::try_from("123e4567-e89b-42d3-a456-426614174000").unwrap(),
                        ),
                        invocation: Box::new(Invocation {
                            operation: ReadOperation::GetMetadata(GetMetadataInput {
                                connection_id: None,
                            }),
                        }),
                    },
                    &cancellation,
                )
                .await
        }
    });

    let plugin_request_id = loop {
        let message = plugin.next().await.unwrap().unwrap();
        let Message::Text(text) = message else {
            continue;
        };
        if let BrokerToPlugin::Request(request) = serde_json::from_str(&text).unwrap() {
            break request.request_id;
        }
    };
    cancellation.cancel();
    let cancelled_id = loop {
        let message = plugin.next().await.unwrap().unwrap();
        let Message::Text(text) = message else {
            continue;
        };
        if let BrokerToPlugin::Cancel(cancel) = serde_json::from_str(&text).unwrap() {
            break cancel.request_id;
        }
    };
    assert_eq!(cancelled_id, plugin_request_id);
    assert_eq!(
        call_task.await.unwrap().unwrap_err().code(),
        ErrorCode::Cancelled
    );

    drop(frontend);
    drop(plugin);
    broker.shutdown().await;
    rpc_server.await.unwrap().unwrap();
    plugin_server.await.unwrap().unwrap();
}

#[tokio::test]
async fn leader_routes_explicit_calls_and_rejects_ambiguous_omissions() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let plugin_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plugin_address = plugin_listener.local_addr().unwrap();
    let plugin_server = tokio::spawn(broker.clone().serve(plugin_listener));
    let mut first_plugin = connect_plugin(
        plugin_address,
        "123e4567-e89b-42d3-a456-426614174001",
        "First",
    )
    .await;
    let mut second_plugin = connect_plugin(
        plugin_address,
        "123e4567-e89b-42d3-a456-426614174002",
        "Second",
    )
    .await;
    while broker.live_file_count().await != 2 {
        tokio::task::yield_now().await;
    }

    let frontend_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let frontend_address = frontend_listener.local_addr().unwrap();
    let rpc_server = tokio::spawn(broker.clone().serve_frontends(frontend_listener));
    let frontend = FrontendClient::connect(frontend_address).await.unwrap();

    let explicit = tokio::spawn({
        let frontend = frontend.clone();
        async move {
            frontend
                .call(
                    metadata_call(Some("123e4567-e89b-42d3-a456-426614174001")),
                    &CancellationToken::new(),
                )
                .await
        }
    });
    let request = next_plugin_request(&mut first_plugin).await;
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(10),
            next_plugin_request(&mut second_plugin)
        )
        .await
        .is_err()
    );
    send_metadata_response(&mut first_plugin, &request.request_id, "First").await;
    assert_eq!(metadata_name(explicit.await.unwrap().unwrap()), "First");

    let omitted = frontend
        .call(metadata_call(None), &CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(omitted.code(), ErrorCode::AmbiguousConnection);

    drop(frontend);
    drop(first_plugin);
    drop(second_plugin);
    broker.shutdown().await;
    rpc_server.await.unwrap().unwrap();
    plugin_server.await.unwrap().unwrap();
}

#[tokio::test]
async fn concurrent_frontend_calls_are_correlated_when_responses_arrive_out_of_order() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let plugin_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plugin_address = plugin_listener.local_addr().unwrap();
    let plugin_server = tokio::spawn(broker.clone().serve(plugin_listener));
    let connection_id = "123e4567-e89b-42d3-a456-426614174003";
    let mut plugin = connect_plugin(plugin_address, connection_id, "Concurrent").await;
    while broker.live_file_count().await != 1 {
        tokio::task::yield_now().await;
    }

    let frontend_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let frontend_address = frontend_listener.local_addr().unwrap();
    let rpc_server = tokio::spawn(broker.clone().serve_frontends(frontend_listener));
    let frontend = FrontendClient::connect(frontend_address).await.unwrap();

    let first_call = tokio::spawn({
        let frontend = frontend.clone();
        async move {
            frontend
                .call(
                    metadata_call(Some(connection_id)),
                    &CancellationToken::new(),
                )
                .await
        }
    });
    let first_request = next_plugin_request(&mut plugin).await;
    let second_call = tokio::spawn({
        let frontend = frontend.clone();
        async move {
            frontend
                .call(
                    metadata_call(Some(connection_id)),
                    &CancellationToken::new(),
                )
                .await
        }
    });
    let second_request = next_plugin_request(&mut plugin).await;

    send_metadata_response(&mut plugin, &second_request.request_id, "Second response").await;
    send_metadata_response(&mut plugin, &first_request.request_id, "First response").await;
    assert_eq!(
        metadata_name(first_call.await.unwrap().unwrap()),
        "First response"
    );
    assert_eq!(
        metadata_name(second_call.await.unwrap().unwrap()),
        "Second response"
    );

    drop(frontend);
    drop(plugin);
    broker.shutdown().await;
    rpc_server.await.unwrap().unwrap();
    plugin_server.await.unwrap().unwrap();
}

#[tokio::test]
async fn duplicate_active_rpc_id_closes_frontend_and_cancels_owned_call() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let plugin_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plugin_address = plugin_listener.local_addr().unwrap();
    let plugin_server = tokio::spawn(broker.clone().serve(plugin_listener));
    let connection_id = "123e4567-e89b-42d3-a456-426614174004";
    let mut plugin = connect_plugin(plugin_address, connection_id, "Duplicate").await;
    while broker.live_file_count().await != 1 {
        tokio::task::yield_now().await;
    }

    let frontend_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let frontend_address = frontend_listener.local_addr().unwrap();
    let rpc_server = tokio::spawn(broker.clone().serve_frontends(frontend_listener));
    let mut frontend = raw_frontend(frontend_address).await;
    let rpc_request_id = RpcRequestId::try_from("123e4567-e89b-42d3-a456-426614174011").unwrap();
    let message = FrontendToLeader::Request {
        rpc_request_id,
        call: Box::new(metadata_call(Some(connection_id))),
    };
    let encoded = encode_frame(&message).unwrap();
    frontend.write_all(&encoded).await.unwrap();
    let request = next_plugin_request(&mut plugin).await;
    frontend.write_all(&encoded).await.unwrap();

    let cancelled_id = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let message = plugin.next().await.unwrap().unwrap();
            let Message::Text(text) = message else {
                continue;
            };
            if let BrokerToPlugin::Cancel(cancel) = serde_json::from_str(&text).unwrap() {
                break cancel.request_id;
            }
        }
    })
    .await
    .expect("duplicate request must cancel the owned plugin call");
    assert_eq!(cancelled_id, request.request_id);

    let mut remaining = Vec::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        frontend.read_to_end(&mut remaining),
    )
    .await
    .expect("duplicate request must close the frontend connection")
    .unwrap();
    drop(frontend);
    drop(plugin);
    broker.shutdown().await;
    rpc_server.await.unwrap().unwrap();
    plugin_server.await.unwrap().unwrap();
}

#[tokio::test]
async fn leader_loss_fails_in_flight_call_without_replay() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let plugin_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plugin_address = plugin_listener.local_addr().unwrap();
    let plugin_server = tokio::spawn(broker.clone().serve(plugin_listener));
    let connection_id = "123e4567-e89b-42d3-a456-426614174005";
    let mut plugin = connect_plugin(plugin_address, connection_id, "Leader loss").await;
    while broker.live_file_count().await != 1 {
        tokio::task::yield_now().await;
    }

    let frontend_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let frontend_address = frontend_listener.local_addr().unwrap();
    let rpc_server = tokio::spawn(broker.clone().serve_frontends(frontend_listener));
    let frontend = FrontendClient::connect(frontend_address).await.unwrap();
    let call = tokio::spawn({
        let frontend = frontend.clone();
        async move {
            frontend
                .call(
                    metadata_call(Some(connection_id)),
                    &CancellationToken::new(),
                )
                .await
        }
    });
    let _request = next_plugin_request(&mut plugin).await;

    rpc_server.abort();
    let _ = rpc_server.await;
    let error = tokio::time::timeout(std::time::Duration::from_secs(1), call)
        .await
        .expect("leader loss must resolve the frontend call")
        .unwrap()
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::ConnectionLost);
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(20),
            next_plugin_request(&mut plugin)
        )
        .await
        .is_err(),
        "frontend must not replay an in-flight call"
    );

    drop(frontend);
    drop(plugin);
    broker.shutdown().await;
    plugin_server.await.unwrap().unwrap();
}

#[tokio::test]
async fn cancellation_is_scoped_to_one_concurrent_frontend_call() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let plugin_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plugin_address = plugin_listener.local_addr().unwrap();
    let plugin_server = tokio::spawn(broker.clone().serve(plugin_listener));
    let connection_id = "123e4567-e89b-42d3-a456-426614174006";
    let mut plugin = connect_plugin(plugin_address, connection_id, "Scoped cancel").await;
    while broker.live_file_count().await != 1 {
        tokio::task::yield_now().await;
    }

    let frontend_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let frontend_address = frontend_listener.local_addr().unwrap();
    let rpc_server = tokio::spawn(broker.clone().serve_frontends(frontend_listener));
    let frontend = FrontendClient::connect(frontend_address).await.unwrap();
    let first_cancellation = CancellationToken::new();
    let first = tokio::spawn({
        let frontend = frontend.clone();
        let cancellation = first_cancellation.clone();
        async move {
            frontend
                .call(metadata_call(Some(connection_id)), &cancellation)
                .await
        }
    });
    let first_request = next_plugin_request(&mut plugin).await;
    let second = tokio::spawn({
        let frontend = frontend.clone();
        async move {
            frontend
                .call(
                    metadata_call(Some(connection_id)),
                    &CancellationToken::new(),
                )
                .await
        }
    });
    let second_request = next_plugin_request(&mut plugin).await;

    first_cancellation.cancel();
    let cancelled_id = loop {
        let message = plugin.next().await.unwrap().unwrap();
        let Message::Text(text) = message else {
            continue;
        };
        if let BrokerToPlugin::Cancel(cancel) = serde_json::from_str(&text).unwrap() {
            break cancel.request_id;
        }
    };
    assert_eq!(cancelled_id, first_request.request_id);
    send_metadata_response(&mut plugin, &second_request.request_id, "Still running").await;
    assert_eq!(
        first.await.unwrap().unwrap_err().code(),
        ErrorCode::Cancelled
    );
    assert_eq!(
        metadata_name(second.await.unwrap().unwrap()),
        "Still running"
    );

    drop(frontend);
    drop(plugin);
    broker.shutdown().await;
    rpc_server.await.unwrap().unwrap();
    plugin_server.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn frontend_handshake_times_out_when_the_port_is_not_a_compatible_leader() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let stalled = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.unwrap();
        std::future::pending::<()>().await;
    });
    let connecting = tokio::spawn(FrontendClient::connect(address));
    tokio::task::yield_now().await;
    assert!(!connecting.is_finished());

    tokio::time::advance(std::time::Duration::from_secs(2)).await;
    let error = connecting.await.unwrap().unwrap_err();
    assert!(matches!(error, BrokerError::FrontendHandshakeTimedOut));
    stalled.abort();
}

#[tokio::test]
async fn broker_shutdown_interrupts_a_frontend_stalled_before_hello() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(broker.clone().serve_frontends(listener));
    let _silent = TcpStream::connect(address).await.unwrap();

    broker.shutdown().await;
    tokio::time::timeout(std::time::Duration::from_millis(100), server)
        .await
        .expect("broker shutdown must interrupt a silent frontend handshake")
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn malformed_frame_cancels_an_active_frontend_call_before_closing() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let plugin_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plugin_address = plugin_listener.local_addr().unwrap();
    let plugin_server = tokio::spawn(broker.clone().serve(plugin_listener));
    let connection_id = "123e4567-e89b-42d3-a456-426614174007";
    let mut plugin = connect_plugin(plugin_address, connection_id, "Malformed frame").await;
    while broker.live_file_count().await != 1 {
        tokio::task::yield_now().await;
    }

    let frontend_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let frontend_address = frontend_listener.local_addr().unwrap();
    let rpc_server = tokio::spawn(broker.clone().serve_frontends(frontend_listener));
    let mut frontend = raw_frontend(frontend_address).await;
    let request = FrontendToLeader::Request {
        rpc_request_id: RpcRequestId::try_from("123e4567-e89b-42d3-a456-426614174012").unwrap(),
        call: Box::new(metadata_call(Some(connection_id))),
    };
    frontend
        .write_all(&encode_frame(&request).unwrap())
        .await
        .unwrap();
    let plugin_request = next_plugin_request(&mut plugin).await;
    frontend
        .write_all(&encode_frame(&serde_json::json!({"type": "invalid"})).unwrap())
        .await
        .unwrap();

    let cancelled_id = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let message = plugin.next().await.unwrap().unwrap();
            let Message::Text(text) = message else {
                continue;
            };
            if let BrokerToPlugin::Cancel(cancel) = serde_json::from_str(&text).unwrap() {
                break cancel.request_id;
            }
        }
    })
    .await
    .expect("malformed frontend input must cancel active plugin calls");
    assert_eq!(cancelled_id, plugin_request.request_id);

    drop(frontend);
    drop(plugin);
    broker.shutdown().await;
    rpc_server.await.unwrap().unwrap();
    plugin_server.await.unwrap().unwrap();
}
