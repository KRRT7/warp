use super::parse_ipc_message;

#[test]
fn parses_authenticated_envelope() {
    let bytes = br#"{"v":1,"token":"abc123","title":"warp://cli-agent","body":"{\"v\":1,\"agent\":\"claude\"}"}"#;
    let (title, body) = parse_ipc_message(bytes, "abc123").unwrap();

    assert_eq!(title.as_deref(), Some("warp://cli-agent"));
    assert_eq!(body, "{\"v\":1,\"agent\":\"claude\"}");
}

#[test]
fn rejects_invalid_token() {
    let bytes = br#"{"v":1,"token":"abc123","title":"warp://cli-agent","body":"{\"v\":1,\"agent\":\"claude\"}"}"#;
    assert!(parse_ipc_message(bytes, "wrong").is_none());
}
