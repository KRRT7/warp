use std::{
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use async_channel::Sender;
use futures::future::FutureExt as _;
use futures::io::AsyncReadExt as _;
use futures::pin_mut;
use futures::select;
use serde::Deserialize;
use warpui::r#async::executor::Background;

use crate::terminal::event::Event;

const MAX_IPC_MESSAGE_BYTES: u64 = 1024 * 1024;
const MAX_IPC_READ_TIMEOUT: Duration = Duration::from_secs(5);

pub struct CLIAgentIpcListener {
    endpoint: String,
    token: String,
    shutdown: Arc<AtomicBool>,
}

impl CLIAgentIpcListener {
    pub(crate) fn start(
        event_sender: Sender<Event>,
        background_executor: Arc<Background>,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let endpoint = listener.local_addr()?.to_string();
        let token = uuid::Uuid::new_v4().simple().to_string();

        let listener = async_io::Async::new(listener)?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = shutdown.clone();
        let thread_token = token.clone();
        let thread_executor = background_executor.clone();
        background_executor
            .spawn(async move {
                run_listener(
                    listener,
                    event_sender,
                    thread_token,
                    thread_shutdown,
                    thread_executor,
                )
                .await;
            })
            .detach();

        Ok(Self {
            endpoint,
            token,
            shutdown,
        })
    }

    pub(crate) fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub(crate) fn token(&self) -> &str {
        &self.token
    }
}

impl Drop for CLIAgentIpcListener {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(&self.endpoint);
    }
}

async fn run_listener(
    listener: async_io::Async<TcpListener>,
    event_sender: Sender<Event>,
    expected_token: String,
    shutdown: Arc<AtomicBool>,
    background_executor: Arc<Background>,
) {
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept().await {
            Ok((stream, _)) => {
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
                let connection_sender = event_sender.clone();
                let connection_token = expected_token.clone();
                let connection_shutdown = shutdown.clone();
                background_executor
                    .spawn(async move {
                        handle_connection(
                            stream,
                            connection_sender,
                            connection_token,
                            connection_shutdown,
                        )
                        .await;
                    })
                    .detach();
            }
            Err(err) => {
                log::warn!("CLI agent IPC listener failed to accept connection: {err}");
                break;
            }
        }
    }
}

async fn handle_connection(
    mut stream: async_io::Async<TcpStream>,
    event_sender: Sender<Event>,
    expected_token: String,
    shutdown: Arc<AtomicBool>,
) {
    if shutdown.load(Ordering::Relaxed) {
        return;
    }

    let mut bytes = Vec::new();
    let mut read_stream = (&mut stream).take(MAX_IPC_MESSAGE_BYTES);
    let read_future = read_stream.read_to_end(&mut bytes).fuse();
    let timeout_future = async_io::Timer::after(MAX_IPC_READ_TIMEOUT).fuse();
    pin_mut!(read_future, timeout_future);

    let read_result = select! {
        result = read_future => result,
        _ = timeout_future => {
            log::warn!(
                "Timed out reading CLI agent IPC event after {:?}",
                MAX_IPC_READ_TIMEOUT
            );
            return;
        }
    };

    match read_result {
        Ok(_) => {
            if let Some((title, body)) = parse_ipc_message(&bytes, &expected_token) {
                if let Err(err) = event_sender.try_send(Event::PluggableNotification { title, body })
                {
                    log::warn!("Failed to forward CLI agent IPC event: {err}");
                }
            }
        }
        Err(err) => {
            log::warn!("Failed to read CLI agent IPC event: {err}");
        }
    }
}

fn parse_ipc_message(bytes: &[u8], expected_token: &str) -> Option<(Option<String>, String)> {
    let envelope: IpcEnvelope = serde_json::from_slice(bytes).ok()?;
    if envelope.v != super::CLI_AGENT_IPC_PROTOCOL_VERSION {
        log::warn!(
            "Ignoring CLI agent IPC event with unsupported protocol version {}",
            envelope.v
        );
        return None;
    }
    if envelope.token != expected_token {
        log::warn!("Ignoring CLI agent IPC event with invalid token");
        return None;
    }

    let title = non_empty_title(envelope.title.trim()).map(|title| title.to_owned())?;
    let body = envelope.body;
    if body.trim().is_empty() {
        return None;
    }
    Some((Some(title), body))
}

fn non_empty_title(title: &str) -> Option<&str> {
    (!title.is_empty()).then_some(title)
}

#[derive(Deserialize)]
struct IpcEnvelope {
    v: u32,
    token: String,
    title: String,
    body: String,
}

#[cfg(test)]
#[path = "ipc_tests.rs"]
mod tests;
