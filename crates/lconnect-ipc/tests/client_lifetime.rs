use lconnect_ipc::{client::IpcClient, jsonrpc::Notification};
use serde_json::json;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn dropping_client_closes_wire_even_with_full_notification_queue() {
    let (client_wire, mut peer) = tokio::io::duplex(1024);
    peer.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"hello\",\"params\":{}}\n")
        .await
        .unwrap();
    let client = IpcClient::from_stream(client_wire).await.unwrap();
    let mut notifications = client.subscribe("media_export").await;
    // Ensure subscription processed: first notification must arrive.
    let event = format!(
        "{}\n",
        serde_json::to_string(&Notification::new("media_export", json!({"event":"test"}))).unwrap()
    );
    // A call acts as a writer barrier after subscribe.
    {
        let call = client.call("barrier", json!({}));
        tokio::pin!(call);
        tokio::select! {
            result=&mut call => panic!("unexpected reply {result:?}"),
            _=async {
                let mut byte=[0];
                loop { peer.read_exact(&mut byte).await.unwrap(); if byte[0]==b'\n' {break;} }
                peer.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":null}\n").await.unwrap();
            } => {}
        }
        call.await.unwrap();
    }
    peer.write_all(event.as_bytes()).await.unwrap();
    notifications.recv().await.unwrap();
    for _ in 0..33 {
        peer.write_all(event.as_bytes()).await.unwrap();
    }
    drop(client);
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut byte = [0];
        assert_eq!(peer.read(&mut byte).await.unwrap(), 0);
        while notifications.recv().await.is_some() {}
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn full_subscription_closes_without_blocking_responses_or_other_subscriptions() {
    let (client_wire, mut peer) = tokio::io::duplex(16 * 1024);
    peer.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"hello\",\"params\":{}}\n")
        .await
        .unwrap();
    let client = IpcClient::from_stream(client_wire).await.unwrap();
    let mut slow = client.subscribe("slow").await;
    let mut other = client.subscribe("other").await;
    let call = client.call("test", json!({}));
    tokio::pin!(call);
    tokio::select! {
        result = &mut call => panic!("unexpected reply {result:?}"),
        _ = async {
            let mut byte = [0];
            loop { peer.read_exact(&mut byte).await.unwrap(); if byte[0] == b'\n' { break; } }
        } => {}
    }
    for n in 0..33 {
        let event = format!(
            "{}\n",
            serde_json::to_string(&Notification::new("slow", json!({"n": n}))).unwrap()
        );
        peer.write_all(event.as_bytes()).await.unwrap();
    }
    peer.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"other\",\"params\":{}}\n{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":true}\n").await.unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), call)
            .await
            .unwrap()
            .unwrap(),
        json!(true)
    );
    assert!(tokio::time::timeout(Duration::from_secs(2), other.recv())
        .await
        .unwrap()
        .is_some());
    for n in 0..32 {
        assert_eq!(slow.recv().await.unwrap().params["n"], n);
    }
    assert!(tokio::time::timeout(Duration::from_secs(2), slow.recv())
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn dedicated_subscription_preserves_more_than_queue_capacity() {
    let (client_wire, mut peer) = tokio::io::duplex(16384);
    peer.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"hello\",\"params\":{}}\n")
        .await
        .unwrap();
    let client = IpcClient::from_stream(client_wire).await.unwrap();
    let mut events = client.subscribe_reliable("media_export").await;
    let barrier = client.call("barrier", json!({}));
    tokio::pin!(barrier);
    tokio::select! {
        result = &mut barrier => panic!("unexpected reply {result:?}"),
        _ = async {
            let mut byte = [0];
            loop { peer.read_exact(&mut byte).await.unwrap(); if byte[0] == b'\n' { break; } }
            peer.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":true}\n").await.unwrap();
        } => {}
    }
    barrier.await.unwrap();
    for n in 0..64 {
        let line = format!(
            "{}\n",
            serde_json::to_string(&Notification::new("media_export", json!({"n":n}))).unwrap()
        );
        peer.write_all(line.as_bytes()).await.unwrap();
    }
    // Let the reader fill the queue before the slow consumer begins.
    tokio::time::sleep(Duration::from_millis(50)).await;
    for n in 0..64 {
        let event = tokio::time::timeout(Duration::from_secs(2), events.recv())
            .await
            .unwrap()
            .expect("export closed before all frames");
        assert_eq!(event.params["n"], n);
    }
}
