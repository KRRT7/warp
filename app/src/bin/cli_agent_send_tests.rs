use super::build_envelope;

#[test]
fn envelope_uses_current_protocol_version() {
    let envelope = build_envelope("warp://cli-agent", "{\"event\":\"session_start\"}", "token");

    let json = serde_json::to_value(&envelope).unwrap();
    assert_eq!(json["v"], warp::terminal::cli_agent_sessions::CLI_AGENT_IPC_PROTOCOL_VERSION);
    assert_eq!(json["token"], "token");
    assert_eq!(json["title"], "warp://cli-agent");
    assert_eq!(json["body"], "{\"event\":\"session_start\"}");
}
