use llcontrol_ipc::client::IpcClient;
use llcontrol_ipc::framing::read_bounded_line;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn newline_less_input_is_rejected_before_overflow_is_accumulated() {
    let (mut writer, reader) = tokio::io::duplex(32);
    let writer_task = tokio::spawn(async move {
        writer.write_all(&[b'x'; 65]).await.expect("write input");
    });
    let mut reader = BufReader::with_capacity(7, reader);
    let mut line = String::new();
    let error = read_bounded_line(&mut reader, &mut line, 64)
        .await
        .expect_err("oversized input must fail");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(line.is_empty());
    writer_task.abort();
}

#[tokio::test]
async fn utf8_split_across_fill_buf_chunks_is_accepted() {
    let (mut writer, reader) = tokio::io::duplex(8);
    let writer_task = tokio::spawn(async move {
        writer
            .write_all("{\"name\":\"Ж\"}\n".as_bytes())
            .await
            .expect("write input");
    });
    let mut reader = BufReader::with_capacity(1, reader);
    let mut line = String::new();
    read_bounded_line(&mut reader, &mut line, 64)
        .await
        .expect("valid UTF-8 line");
    assert_eq!(line, "{\"name\":\"Ж\"}\n");
    writer_task.await.expect("writer task");
}

#[tokio::test]
async fn client_rejects_an_oversized_hello_line() {
    let (client_end, mut server_end) = tokio::io::duplex(1024);
    let writer = tokio::spawn(async move {
        let chunk = vec![b'x'; 64 * 1024];
        for _ in 0..=128 {
            if server_end.write_all(&chunk).await.is_err() {
                return;
            }
        }
    });
    let error = match IpcClient::from_stream(client_end).await {
        Ok(_) => panic!("oversized hello must fail"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("exceeds 8388608 bytes"));
    writer.abort();
}

#[tokio::test]
async fn oversized_response_closes_multiplexer_and_later_call_returns_promptly() {
    let (client_end, mut server_end) = tokio::io::duplex(64 * 1024);
    let server = tokio::spawn(async move {
        server_end
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"hello\",\"params\":{}}\n")
            .await
            .expect("write hello");
        let mut request = String::new();
        BufReader::new(&mut server_end)
            .read_line(&mut request)
            .await
            .expect("read request");
        let chunk = vec![b'x'; 64 * 1024];
        for _ in 0..=128 {
            if server_end.write_all(&chunk).await.is_err() {
                return;
            }
        }
    });
    let client = IpcClient::from_stream(client_end)
        .await
        .expect("connect client");

    let first = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        client.call("first", json!({})),
    )
    .await
    .expect("first call must not hang")
    .expect_err("oversized response closes connection");
    assert!(first.to_string().contains("response dropped"));

    let second = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        client.call("second", json!({})),
    )
    .await
    .expect("later call must not hang")
    .expect_err("closed multiplexer rejects later call");
    assert!(second.to_string().contains("client multiplexer closed"));
    server.abort();
}
