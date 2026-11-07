use std::{
    collections::HashMap,
    path::PathBuf,
    process::Stdio,
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicU64, Ordering},
    },
};

use anyhow::{Result, anyhow, bail};
use futures::{
    AsyncReadExt as _, AsyncWriteExt as _, FutureExt as _,
    channel::{mpsc, oneshot},
    future::BoxFuture,
    StreamExt as _,
};
use serde_json::{Value, json};

use crate::jsonrpc::{IncomingMessage, MessageId, OutgoingMessage, ResponseOutcome};
use crate::schema::*;
use gpui_util::new_std_command;

/// Filesystem delegation methods from the Agent Client Protocol, answered
/// as unsupported because agents run inside the workspace with their own
/// filesystem access.
const METHOD_FS_READ_TEXT_FILE: &str = "fs/read_text_file";
const METHOD_FS_WRITE_TEXT_FILE: &str = "fs/write_text_file";
const METHOD_SESSION_SET_MODE: &str = "session/set_mode";

/// Events a connected agent emits on the stream polled by the UI. These are
/// session-scoped since one process can host multiple sessions.
#[derive(Debug)]
pub enum AcpEvent {
    SessionUpdate {
        session_id: String,
        update: SessionUpdate,
    },
    /// The agent wants approval before performing a sensitive action. The UI
    /// must answer with [`AcpAgent::acknowledge_permission`] or the agent
    /// stalls.
    PermissionRequest {
        request_id: MessageId,
        request: PermissionRequest,
    },
    /// The agent process closed its side of the connection or was terminated.
    Terminated,
}

/// A live conversation inside an agent process.
#[derive(Clone)]
pub struct AcpSession {
    pub session_id: String,
    pub modes: Vec<SessionMode>,
    pub current_mode_id: Option<String>,
    pub agent: Arc<AcpAgent>,
}

/// A connected agent process.
pub struct AcpAgent {
    child: Option<util::process::Child>,
    pid: Option<u32>,
    next_request_id: AtomicU64,
    outgoing: mpsc::UnboundedSender<OutgoingMessage>,
    pending: Arc<StdMutex<HashMap<MessageId, oneshot::Sender<ResponseOutcome>>>>,
    initialized: Arc<StdMutex<Option<InitializeResult>>>,
}

pub struct AgentSpawnConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub working_directory: Option<PathBuf>,
}

pub struct SpawnedAgent {
    pub agent: Arc<AcpAgent>,
    pub events: mpsc::UnboundedReceiver<AcpEvent>,
    /// The task pumping I/O both ways for this connection. Always spawn it on
    /// a background executor: without it, no traffic ever reaches the agent.
    pub driver: BoxFuture<'static, ()>,
}

impl AcpAgent {
    /// Spawns the agent process and returns the connection with the event
    /// stream. The returned driver future must be spawned by the caller.
    pub async fn spawn(config: AgentSpawnConfig) -> Result<SpawnedAgent> {
        let mut command = new_std_command(&config.command);
        command.args(&config.args).envs(config.env.clone());

        if let Some(working_directory) = &config.working_directory {
            command.current_dir(working_directory);
        }

        let command_display = config.command.clone();
        let child = util::process::Child::spawn(
            command,
            Stdio::piped(),
            Stdio::piped(),
            Stdio::null(),
        )
        .map_err(|error| {
            anyhow!("failed to spawn agent process `{command_display}`: {error}")
        })?;

        let pid = child.id();
        log::info!("spawning agent `{command_display}` (pid {pid})");
        Ok(Self::connect(Some(child)))
    }

    /// Builds a connection from an already-spawned child, taking over its
    /// pipes. The driver future must be spawned by the caller.
    pub fn connect(mut child: Option<util::process::Child>) -> SpawnedAgent {
        let child_stdin = child.as_mut().and_then(|child| child.stdin.take());
        let child_stdout = child
            .as_mut()
            .and_then(|child| child.stdout.take())
            .map(smol::io::BufReader::new);
        Self::connect_from_io(child, child_stdin, child_stdout)
    }

    /// Builds a connection from arbitrary I/O, so tests can drive a fake
    /// agent over an in-memory pipe.
    fn connect_from_io<R, W>(
        child: Option<util::process::Child>,
        child_stdin: Option<W>,
        child_stdout: Option<R>,
    ) -> SpawnedAgent
    where
        R: futures::io::AsyncRead + std::marker::Unpin + std::marker::Send + 'static,
        W: futures::io::AsyncWrite + std::marker::Unpin + std::marker::Send + 'static,
    {
        let pid = child.as_ref().map(|child| child.id());
        let (outgoing_tx, outgoing_rx) = mpsc::unbounded::<OutgoingMessage>();
        let (events_tx, events_rx) = mpsc::unbounded::<AcpEvent>();

        let pending: Arc<StdMutex<HashMap<MessageId, oneshot::Sender<ResponseOutcome>>>> =
            Arc::new(StdMutex::new(HashMap::new()));
        let initialized: Arc<StdMutex<Option<InitializeResult>>> = Arc::new(StdMutex::new(None));

        let agent = Arc::new(AcpAgent {
            child,
            pid,
            next_request_id: AtomicU64::new(1),
            outgoing: outgoing_tx,
            pending: pending.clone(),
            initialized,
        });

        let driver = {
            let agent = agent.clone();
            let mut child_stdout = child_stdout;
            let mut child_stdin = child_stdin;
            let mut outgoing_rx = outgoing_rx;
            let pending = Arc::clone(&pending);

            async move {
                let read_loop = async {
                    loop {
                        let Some(input) = &mut child_stdout else {
                            log::warn!("agent stdout is unavailable");
                            break;
                        };

                        let Some(line) = read_to_newline(input).await else {
                            break;
                        };

                        agent.dispatch_line(&line, &events_tx).await;
                    }
                };

                let write_loop = async {
                    while let Some(message) = outgoing_rx.next().await {
                        let Some(sink) = &mut child_stdin else {
                            log::warn!("agent stdin is unavailable; dropping `{message:?}`");
                            continue;
                        };

                        let mut encoded = Vec::new();
                        message.write_to(&mut encoded);
                        if let Err(error) = sink.write_all(&encoded).await {
                            log::warn!("failed writing a message to the agent: {error}");
                            break;
                        }

                        if let Err(error) = sink.flush().await {
                            log::warn!("failed flushing a message to the agent: {error}");
                            break;
                        }
                    }
                };

                // The read loop decides the connection's lifetime: once the agent
                // stops talking, the write loop would otherwise park forever on
                // the outgoing channel.
                futures::pin_mut!(read_loop);
                futures::pin_mut!(write_loop);
                futures::select_biased! {
                    _ = read_loop.fuse() => (),
                    _ = write_loop.fuse() => (),
                }

                let shutdown_reason = "agent connection ended";
                log::info!("{shutdown_reason}");
                fail_pending(&pending, shutdown_reason);
                if events_tx.unbounded_send(AcpEvent::Terminated).is_err() {
                    log::debug!("session listener already gone at agent shutdown");
                }
            }
        };

        SpawnedAgent {
            agent,
            events: events_rx,
            driver: Box::pin(driver),
        }
    }
}

/// Reads a single newline-delimited payload, returning `None` at EOF.
async fn read_to_newline<R: futures::io::AsyncRead + std::marker::Unpin>(
    reader: &mut R,
) -> Option<String> {
    let mut line = Vec::new();
    let mut byte = [0_u8];

    loop {
        match reader.read(&mut byte).await {
            Ok(0) | Err(_) => {
                if line.is_empty() {
                    return None;
                }

                break;
            }
            Ok(_) => {
                if byte[0] == b'\n' {
                    break;
                }

                line.push(byte[0]);
            }
        }
    }

    match String::from_utf8(line) {
        Ok(line) => Some(line),
        Err(error) => {
            log::warn!("discarding non-utf8 bytes from the agent: {error}");
            None
        }
    }
}

fn fail_pending(
    pending: &Arc<StdMutex<HashMap<MessageId, oneshot::Sender<ResponseOutcome>>>>,
    description: &str,
) {
    let mut pending = pending.lock().unwrap_or_else(|error| error.into_inner());

    for (id, sender) in pending.drain() {
        if sender
            .send(ResponseOutcome::Error {
                code: -32000,
                message: description.to_string(),
            })
            .is_err()
        {
            log::debug!("pending request {id:?} surfaced no answerer");
        }
    }
}

impl Drop for AcpAgent {
    fn drop(&mut self) {
        // Well-behaved agents exit on stdin EOF; make sure of the rest.
        if let Some(child) = &mut self.child {
            drop(child.stdin.take());
        }

        self.kill_process_group();
    }
}

impl AcpAgent {
    pub fn capabilities(&self) -> AgentCapabilities {
        self.initialized
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .map(|handshake| handshake.agent_capabilities.clone())
            .unwrap_or_default()
    }

    /// Declares ourselves to the agent (the first request on the wire) and
    /// stores the capabilities it reveals.
    pub async fn initialize(&self, capabilities: ClientCapabilities) -> Result<InitializeResult> {
        let params = json!(InitializeParams {
            protocol_version: PROTOCOL_VERSION,
            client_capabilities: capabilities,
        });

        let response = self.request(METHOD_INITIALIZE, params).await?;
        let handshake: InitializeResult = serde_json::from_value(response)
            .map_err(|error| anyhow!("agent returned an invalid initialize response: {error}"))?;

        if handshake.protocol_version > PROTOCOL_VERSION {
            bail!(
                "agent speaks protocol version {} while this build supports {PROTOCOL_VERSION}",
                handshake.protocol_version
            );
        }

        *self
            .initialized
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(handshake.clone());

        Ok(handshake)
    }

    /// Starts a fresh working session in the given directory.
    pub async fn new_session(self: &Arc<Self>, working_directory: &str) -> Result<AcpSession> {
        let response = self
            .request(
                METHOD_SESSION_NEW,
                json!(NewSessionParams {
                    cwd: working_directory.to_string(),
                    mcp_servers: Vec::new(),
                }),
            )
            .await?;

        let session: SessionNewResponse = serde_json::from_value(response)
            .map_err(|error| anyhow!("agent returned an invalid session/new response: {error}"))?;
        if session.session_id.is_empty() {
            bail!("agent did not report a session id");
        }

        let (modes, current_mode_id) = match session.modes {
            Some(modes_info) => (
                modes_info.available,
                Some(modes_info.current).filter(|mode_id| !mode_id.is_empty()),
            ),
            None => (Vec::new(), None),
        };

        Ok(AcpSession {
            session_id: session.session_id,
            modes,
            current_mode_id,
            agent: self.clone(),
        })
    }

    /// Terminates the agent: hard-kills the whole process group so
    /// agent-spawned children (MCP servers, shells) do not outlive us. The
    /// agent's stdin also drops when the connection object does, giving
    /// well-behaved agents a graceful exit signal.
    pub fn terminate(&self) {
        self.kill_process_group();
    }

    pub fn request(&self, method: &str, params: Value) -> BoxFuture<'_, Result<Value>> {
        let method = method.to_string();
        Box::pin(async move {
            let request_id = self.next_request_id.fetch_add(1, Ordering::SeqCst);
            let (sender, receiver) = oneshot::channel();

            self.pending
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(MessageId::Number(request_id), sender);

            if self
                .outgoing
                .unbounded_send(OutgoingMessage::Request {
                    id: request_id,
                    method: method.clone(),
                    params,
                })
                .is_err()
            {
                self.pending
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .remove(&MessageId::Number(request_id));
                bail!("agent connection closed while sending `{method}`");
            }

            match receiver.await {
                Ok(ResponseOutcome::Success(result)) => Ok(result),
                Ok(ResponseOutcome::Error { code, message }) => Err(anyhow!(
                    "agent rejected `{method}` (error code {code}): {message}"
                )),
                Err(_) => Err(anyhow!(
                    "agent connection closed before answering `{method}`"
                )),
            }
        })
    }

    pub fn notification(&self, method: &str, params: Value) {
        if let Err(error) = self
            .outgoing
            .unbounded_send(OutgoingMessage::Notification {
                method: method.to_string(),
                params,
            })
        {
            log::warn!("failed sending `{method}` to the agent: {error}");
        }
    }

    /// Answers a request the agent made to us.
    pub fn respond(&self, request_id: MessageId, outcome: ResponseOutcome) {
        if let Err(error) = self
            .outgoing
            .unbounded_send(OutgoingMessage::Response {
                id: request_id,
                outcome,
            })
        {
            log::warn!("failed sending a response to the agent: {error}");
        }
    }

    pub fn acknowledge_permission(&self, request_id: MessageId, outcome: PermissionOutcome) {
        self.respond(
            request_id,
            ResponseOutcome::Success(json!({ "outcome": json!(outcome) })),
        )
    }

    #[cfg(unix)]
    fn kill_process_group(&self) {
        let Some(pid) = self.pid else {
            return;
        };

        let group_id = -(pid as i32);
        let killed = unsafe { libc::kill(group_id, libc::SIGKILL) };
        if killed != 0 {
            let error = std::io::Error::last_os_error();
            if let Some(libc::ESRCH) = error.raw_os_error() {
                log::debug!("agent process group {pid} was already gone");
            } else {
                log::warn!("failed killing agent process group {pid}: {error}");
            }
        }
    }

    #[cfg(not(unix))]
    fn kill_process_group(&self) {
        // On Windows, the job object attached to `util::process::Child`
        // tears the whole process tree down when the child is dropped.
    }

    fn respond_rejecting(&self, request_id: MessageId, message: &str) {
        self.respond(
            request_id,
            ResponseOutcome::Error {
                code: -32601,
                message: message.to_string(),
            },
        );
    }

    fn respond_invalid<E: std::fmt::Display>(&self, request_id: MessageId, error: &E) {
        self.respond(
            request_id,
            ResponseOutcome::Error {
                code: -32602,
                message: error.to_string(),
            },
        );
    }
}

impl AcpAgent {
    async fn dispatch_line(&self, line: &str, events: &mpsc::UnboundedSender<AcpEvent>) {
        let message = match IncomingMessage::try_parse(line) {
            Ok(Some(message)) => message,
            Ok(None) => return,
            Err(error) => {
                log::warn!("discarding malformed line from the agent: {error}");
                return;
            }
        };

        match message {
            IncomingMessage::Response { id, outcome } => {
                let sender = self
                    .pending
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .remove(&id);
                match sender {
                    Some(sender) => {
                        if sender.send(outcome).is_err() {
                            log::debug!("caller abandoned agent request {id:?}; response discarded");
                        }
                    }
                    None => log::debug!("agent answered unknown request id {id:?}"),
                }
            }
            IncomingMessage::Request {
                id,
                method,
                params,
            } => self.handle_agent_request(id, method, params, events).await,
            IncomingMessage::Notification { method, params } => {
                self.handle_agent_notification(method, params, events)
            }
            IncomingMessage::Malformed(description) => {
                log::warn!("discarding malformed message from agent: {description}");
            }
        }
    }

    async fn handle_agent_request(
        &self,
        id: MessageId,
        method: String,
        params: Value,
        events: &mpsc::UnboundedSender<AcpEvent>,
    ) {
        match method.as_str() {
            METHOD_REQUEST_PERMISSION => {
                match serde_json::from_value::<PermissionRequest>(params) {
                    Ok(request) => {
                        if events
                            .unbounded_send(AcpEvent::PermissionRequest {
                                request_id: id.clone(),
                                request,
                            })
                            .is_err()
                        {
                            self.respond_rejecting(
                                id,
                                "cannot ask for permission: no session is listening",
                            );
                        }
                    }
                    Err(error) => self.respond_invalid(id, &error),
                }
            }
            METHOD_FS_READ_TEXT_FILE | METHOD_FS_WRITE_TEXT_FILE => {
                // Filesystem delegation is deliberately not advertised: agents
                // run inside the workspace and already touch files their own
                // way.
                self.respond_rejecting(id, "filesystem delegation is not enabled for agents");
            }
            _ => {
                log::debug!("agent called unsupported method `{method}`");
                self.respond_rejecting(id, "method not found");
            }
        }
    }

    fn handle_agent_notification(
        &self,
        method: String,
        params: Value,
        events: &mpsc::UnboundedSender<AcpEvent>,
    ) {
        match method.as_str() {
            METHOD_SESSION_UPDATE => {
                let session_id = params
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                match params.get("update") {
                    Some(update) => {
                        let update = parse_session_update(update);
                        if events
                            .unbounded_send(AcpEvent::SessionUpdate { session_id, update })
                            .is_err()
                        {
                            log::warn!("session listener dropped; discarded agent update");
                        }
                    }
                    None => log::warn!("agent sent session/update without an update payload"),
                }
            }
            _ => log::debug!("agent sent unsupported notification `{method}`"),
        }
    }
}

impl AcpSession {
    pub async fn prompt(&self, prompt: Vec<ContentBlock>) -> Result<StopReason> {
        let response = self
            .agent
            .request(
                METHOD_SESSION_PROMPT,
                json!({
                    "sessionId": self.session_id,
                    "prompt": prompt,
                }),
            )
            .await?;

        let parsed: PromptResponse = serde_json::from_value(response)
            .map_err(|error| anyhow!("agent returned an invalid prompt response: {error}"))?;

        match parsed.stop_reason.as_deref().and_then(StopReason::from_str) {
            Some(stop_reason) => Ok(stop_reason),
            None => {
                log::warn!(
                    "agent did not report a recognized stop reason; treating the turn as complete"
                );
                Ok(StopReason::END_TURN)
            }
        }
    }

    pub fn cancel(&self) {
        self.agent.notification(
            METHOD_SESSION_CANCEL,
            json!({ "sessionId": self.session_id }),
        );
    }

    /// Switches the agent's working mode (e.g. plan vs edit). Fails when the
    /// agent does not expose modes at all.
    pub async fn set_mode(&self, mode_id: &str) -> Result<()> {
        self.agent
            .request(
                METHOD_SESSION_SET_MODE,
                json!({
                    "sessionId": self.session_id,
                    "modeId": mode_id,
                }),
            )
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::io::Result as IoResult;
    use std::pin::Pin;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
    use std::task::{Context, Poll};
    use futures::task::AtomicWaker;

    struct MemPipeState {
        queue: Mutex<VecDeque<u8>>,
        reader_waker: AtomicWaker,
        closed: AtomicBool,
    }

    impl Default for MemPipeState {
        fn default() -> Self {
            Self {
                queue: Mutex::new(VecDeque::new()),
                reader_waker: AtomicWaker::default(),
                closed: AtomicBool::new(false),
            }
        }
    }

    fn mem_pipe() -> (MemWriter, MemReader) {
        let state = Arc::new(MemPipeState::default());
        (
            MemWriter { state: state.clone() },
            MemReader { state },
        )
    }

    /// Write end of a single-direction in-memory byte pipe.
    struct MemWriter {
        state: Arc<MemPipeState>,
    }

    impl Drop for MemWriter {
        fn drop(&mut self) {
            self.state.closed.store(true, AtomicOrdering::Relaxed);
            self.state.reader_waker.wake();
        }
    }

    impl futures::io::AsyncWrite for MemWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<IoResult<usize>> {
            self.state.queue.lock().unwrap().extend(buf.iter().copied());
            self.state.reader_waker.wake();
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<IoResult<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<IoResult<()>> {
            self.state.closed.store(true, AtomicOrdering::Relaxed);
            self.state.reader_waker.wake();
            Poll::Ready(Ok(()))
        }
    }

    /// Read end of a single-direction in-memory byte pipe.
    struct MemReader {
        state: Arc<MemPipeState>,
    }

    impl futures::io::AsyncRead for MemReader {
        fn poll_read(
            self: Pin<&mut Self>,
            context: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<IoResult<usize>> {
            let filled = {
                let mut queue = self.state.queue.lock().unwrap();
                let mut filled = 0usize;
                while let Some(byte) = queue.pop_front() {
                    if filled == buf.len() {
                        queue.push_front(byte);
                        break;
                    }

                    buf[filled] = byte;
                    filled += 1;
                }

                filled
            };

            if filled > 0 {
                return Poll::Ready(Ok(filled));
            }

            if self.state.closed.load(AtomicOrdering::Relaxed) {
                return Poll::Ready(Ok(0));
            }

            self.state.reader_waker.register(context.waker());
            Poll::Pending
        }
    }

    struct FakeAgentIo {
        /// Agent's end writing toward the connection.
        writer: MemWriter,
        /// Agent's end reading what the connection sends.
        reader: MemReader,
    }

    /// Builds a connected [`AcpAgent`] paired with a fake agent the test
    /// drives through `FakeAgentIo`.
    fn spawn_fake_agent() -> (SpawnedAgent, FakeAgentIo) {
        let (agent_writer, client_reader) = mem_pipe();
        let (client_writer, agent_reader) = mem_pipe();
        let agent = FakeAgentIo {
            writer: agent_writer,
            reader: agent_reader,
        };

        let connection = AcpAgent::connect_from_io(None, Some(client_writer), Some(client_reader));
        (connection, agent)
    }

    impl FakeAgentIo {
        async fn send(&mut self, message: OutgoingMessage) {
            use futures::AsyncWriteExt as _;

            let mut encoded = Vec::new();
            message.write_to(&mut encoded);
            self.writer.write_all(&encoded).await.unwrap();
            self.writer.flush().await.unwrap();
        }

        async fn notification(&mut self, method: &str, params: Value) {
            self.send(OutgoingMessage::Notification {
                method: method.into(),
                params,
            })
            .await;
        }

        async fn answer(&mut self, id: u64, result: Value) {
            self.send(OutgoingMessage::Response {
                id: MessageId::Number(id),
                outcome: ResponseOutcome::Success(result),
            })
            .await;
        }

        /// Reads the next newline-delimited JSON message the connection sent,
        /// or `None` once the connection is gone.
        async fn receive(&mut self) -> Option<Value> {
            use futures::AsyncReadExt as _;

            let mut line = Vec::new();
            let mut byte = [0_u8];

            loop {
                match self.reader.read(&mut byte).await {
                    Ok(0) | Err(_) => return None,
                    Ok(_) => {
                        if byte[0] == b'\n' {
                            break;
                        }

                        line.push(byte[0]);
                    }
                }
            }

            serde_json::from_slice(&line).ok()
        }
    }

    #[test]
    fn round_trips_a_request() {
        let (connection, mut fake) = spawn_fake_agent();
        let SpawnedAgent {
            agent,
            events: _events,
            driver,
        } = connection;

        futures::executor::block_on(async move {
            let caller = async {
                let answer = agent.request("ping", json!({ "x": 1 })).await.unwrap();
                assert_eq!(answer["done"], true);
            };

            let fake_agent = async {
                let incoming = fake.receive().await.expect("client request");
                assert_eq!(incoming["method"], "ping");
                fake.answer(incoming["id"].as_u64().unwrap(), json!({ "done": true }))
                    .await;
            };

            futures::join!(caller, fake_agent, driver);
        });
    }

    #[test]
    fn relays_session_updates_to_the_listener() {
        let (connection, mut fake) = spawn_fake_agent();
        let SpawnedAgent {
            agent, mut events, driver,
        } = connection;
        drop(agent);

        futures::executor::block_on(async move {
            let fake_agent = async {
                for params in [
                    json!({
                        "sessionId": "s1",
                        "update": {
                            "sessionUpdate": "plan",
                            "entries": [
                                { "content": "Step 1", "priority": "weird", "status": "pending" }
                            ],
                        },
                    }),
                    json!({
                        "sessionId": "s1",
                        "update": {
                            "sessionUpdate": "agent_message_chunk",
                            "content": { "type": "text", "text": "hello" },
                        },
                    }),
                    json!({
                        "sessionId": "s1",
                        "update": {
                            "sessionUpdate": "tool_call",
                            "toolCallId": "t1",
                            "title": "build the editor",
                            "kind": "execute",
                            "status": "in_progress",
                        },
                    }),
                ] {
                    fake.notification(METHOD_SESSION_UPDATE, params).await;
                }
            };

            futures::join!(fake_agent, driver);

            let mut updates = Vec::new();
            loop {
                match events.next().await {
                    Some(AcpEvent::SessionUpdate { update, .. }) => updates.push(update),
                    Some(AcpEvent::Terminated) => break,
                    Some(other) => panic!("unexpected event: {other:?}"),
                    None => break,
                }
            }

            assert_eq!(updates.len(), 3, "got {updates:?}");
            assert_eq!(
                updates[0],
                SessionUpdate::Plan {
                    entries: vec![PlanEntry {
                        content: "Step 1".to_string(),
                        priority: PlanEntryPriority::Primary,
                        status: PlanEntryStatus::Pending,
                    }],
                }
            );

            let SessionUpdate::AgentMessageChunk { content } = &updates[1] else {
                panic!("expected a message chunk, got {:?}", updates[1]);
            };
            assert_eq!(content.text_content(), Some("hello"));

            let SessionUpdate::ToolCall(tool_call) = &updates[2] else {
                panic!("expected a tool call, got {:?}", updates[2]);
            };
            assert_eq!(tool_call.id, "t1");
            assert_eq!(tool_call.kind, Some(ToolCallKind::Execute));
        });
    }

    #[test]
    fn answers_permission_requests() {
        let (connection, mut fake) = spawn_fake_agent();
        let SpawnedAgent {
            agent, mut events, driver,
        } = connection;

        futures::executor::block_on(async move {
            let caller = async {
                let AcpEvent::PermissionRequest { request_id, request } =
                    events.next().await.expect("permission request")
                else {
                    panic!("expected a permission request");
                };

                let allow = request
                    .options
                    .iter()
                    .find(|option| option.id == "allow_once")
                    .expect("the request carries an allow option");
                agent.acknowledge_permission(
                    request_id,
                    PermissionOutcome::Selected {
                        option_id: allow.id.clone(),
                    },
                );
            };

            let fake_agent = async {
                fake.notification(
                    METHOD_REQUEST_PERMISSION,
                    json!({
                        "sessionId": "s1",
                        "options": [
                            { "id": "allow_once", "name": "Allow this time", "kind": "allow_once" },
                            { "id": "reject", "name": "Reject", "kind": "reject_once" },
                        ],
                    }),
                )
                .await;

                let answer = fake.receive().await.expect("permission answer");
                let outcome = &answer["result"]["outcome"];
                assert_eq!(outcome["outcome"], "selected");
                assert_eq!(outcome["optionId"], "allow_once");
            };

            futures::join!(caller, fake_agent, driver);
        });
    }
}
