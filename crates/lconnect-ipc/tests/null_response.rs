//! Optional getters legitimately return JSON null. Absence remains malformed.
use lconnect_ipc::{client::IpcClient, jsonrpc::Response, transport_memory::mem_pair, IpcError};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[test]
fn response_roundtrip_preserves_present_null_but_not_missing_member() {
    let wire = serde_json::to_value(Response::ok(json!(1), Value::Null)).unwrap();
    assert!(wire.as_object().unwrap().contains_key("result"));
    let response: Response = serde_json::from_value(wire).unwrap();
    assert_eq!(response.result, Some(Value::Null));
    let missing: Response = serde_json::from_value(json!({"jsonrpc":"2.0","id":1})).unwrap();
    assert_eq!(missing.result, None);
    assert!(serde_json::from_value::<Response>(
        json!({"jsonrpc":"2.0","id":1,"result":null,"error":null})
    )
    .is_err());
}

#[tokio::test]
async fn client_accepts_null_and_rejects_absent_or_conflicting_results() {
    let (client_side, server_side) = mem_pair();
    let server = tokio::spawn(async move {
        let (read, mut write) = tokio::io::split(server_side);
        write
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"hello\",\"params\":{}}\n")
            .await
            .unwrap();
        let mut reader = BufReader::new(read);
        for mut response in [
            json!({"result":null}),
            json!({}),
            json!({"result":null,"error":{"code":-32002,"message":"invalid"}}),
            json!({"error":{"code":-32002,"message":"invalid"}}),
            json!({"result":null,"error":null}),
            json!({"error":"malformed"}),
            json!({"result":null}),
        ] {
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            response["jsonrpc"] = json!("2.0");
            response["id"] = request["id"].clone();
            let mut bytes = serde_json::to_vec(&response).unwrap();
            bytes.push(b'\n');
            write.write_all(&bytes).await.unwrap();
        }
    });
    let client = IpcClient::from_stream(client_side).await.unwrap();
    let run = async {
        assert_eq!(
            client.call("get_lcd_theme", json!({})).await.unwrap(),
            Value::Null
        );
        assert!(
            matches!(client.call("missing",json!({})).await,Err(IpcError::Transport(s)) if s.contains("neither"))
        );
        assert!(
            matches!(client.call("conflict",json!({})).await,Err(IpcError::Transport(s)) if s.contains("both"))
        );
        assert!(matches!(
            client.call("error", json!({})).await,
            Err(IpcError::InvalidArgument(_))
        ));
        for method in ["null_error", "malformed_error"] {
            assert!(matches!(
                client.call(method, json!({})).await,
                Err(IpcError::Transport(_))
            ));
        }
        assert_eq!(
            client.call("get_lcd_theme", json!({})).await.unwrap(),
            Value::Null
        );
        server.await.unwrap();
    };
    tokio::time::timeout(std::time::Duration::from_secs(3), run)
        .await
        .unwrap();
}
