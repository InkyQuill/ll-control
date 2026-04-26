//! Integration tests for the JSON-RPC 2.0 envelope types.

use lconnect_ipc::jsonrpc::{ErrorObj, Request, Response};
use serde_json::json;

#[test]
fn request_serializes_with_jsonrpc_2_0_envelope() {
    let r = Request::new(42, "list_devices", json!({}));
    let s = serde_json::to_string(&r).unwrap();
    assert!(s.contains(r#""jsonrpc":"2.0""#));
    assert!(s.contains(r#""method":"list_devices""#));
    assert!(s.contains(r#""id":42"#));
}

#[test]
fn response_with_result() {
    let r = Response::ok(json!(7), json!({"ok": true}));
    let s = serde_json::to_string(&r).unwrap();
    assert!(s.contains(r#""result":{"ok":true}"#));
    assert!(s.contains(r#""id":7"#));
    assert!(s.contains(r#""jsonrpc":"2.0""#));
}

#[test]
fn response_with_error() {
    let r = Response::err(
        Some(json!(7)),
        ErrorObj {
            code: -32000,
            message: "device not found".into(),
            data: None,
        },
    );
    let s = serde_json::to_string(&r).unwrap();
    assert!(s.contains(r#""code":-32000"#));
    assert!(s.contains(r#""message":"device not found""#));
}

/// Per JSON-RPC 2.0, a successful response must NOT carry an `error` member,
/// and an error response must NOT carry a `result` member. The
/// `skip_serializing_if` attribute is what enforces that on the wire.
#[test]
fn response_omits_unused_payload_field() {
    // ok() omits the error field
    let ok = Response::ok(json!(1), json!("hello"));
    let ok_s = serde_json::to_string(&ok).unwrap();
    assert!(ok_s.contains(r#""result":"hello""#));
    assert!(
        !ok_s.contains("\"error\""),
        "Response::ok must not serialize an `error` field, got: {ok_s}"
    );

    // err() omits the result field
    let err = Response::err(
        Some(json!(2)),
        ErrorObj {
            code: -32601,
            message: "method not found".into(),
            data: None,
        },
    );
    let err_s = serde_json::to_string(&err).unwrap();
    assert!(err_s.contains(r#""code":-32601"#));
    assert!(
        !err_s.contains("\"result\""),
        "Response::err must not serialize a `result` field, got: {err_s}"
    );
}
