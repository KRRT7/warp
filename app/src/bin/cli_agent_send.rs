use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    env,
    io::{self, Read, Write},
    net::TcpStream,
};

use warp::terminal::cli_agent_sessions::CLI_AGENT_IPC_PROTOCOL_VERSION;

#[derive(Serialize)]
struct IpcEnvelope<'a> {
    v: u32,
    token: &'a str,
    title: &'a str,
    body: &'a str,
}

fn build_envelope<'a>(title: &'a str, body: &'a str, token: &'a str) -> IpcEnvelope<'a> {
    IpcEnvelope {
        v: CLI_AGENT_IPC_PROTOCOL_VERSION,
        token,
        title,
        body,
    }
}

fn main() -> Result<()> {
    let endpoint = env::var("WARP_CLI_AGENT_IPC")
        .context("WARP_CLI_AGENT_IPC must point to Warp's local endpoint")?;
    let token = env::var("WARP_CLI_AGENT_TOKEN")
        .context("WARP_CLI_AGENT_TOKEN must be set for Warp CLI agent IPC")?;
    let title = env::args()
        .nth(1)
        .context("missing notification title argument")?;

    let mut body = String::new();
    io::stdin()
        .read_to_string(&mut body)
        .context("failed to read CLI agent body from stdin")?;
    if body.trim().is_empty() {
        return Ok(());
    }

    let envelope = build_envelope(&title, &body, &token);

    let mut stream = TcpStream::connect(&endpoint)
        .with_context(|| format!("failed to connect to Warp CLI agent endpoint {endpoint}"))?;
    serde_json::to_writer(&mut stream, &envelope).context("failed to write IPC envelope")?;
    stream.flush().context("failed to flush CLI agent IPC payload")?;
    Ok(())
}

#[cfg(test)]
#[path = "cli_agent_send_tests.rs"]
mod tests;
