//! `OmpRuntime`: owned handle to one bundled OMP sidecar process.
//!
//! Plan §5 target shape (adapted — blocking client behind a thread bridge):
//!
//! ```rust,ignore
//! struct OmpRuntime {
//!     client: omp_rpc::Client, // blocking; lives on a dedicated I/O thread
//!     child: Child,
//!     state: OmpRuntimeState,
//! }
//! ```
//!
//! Phase 1 is headless by design: spawn → prompt/abort → open/new session →
//! set_model → images → ask-dialog opt-in → host tools/URIs. Every method is
//! blocking and MUST be called off the UI thread; async/GPUI bridging lands
//! with the panel in Phase 2.

use crate::{EventRouter, OmpError, RespawnRequest, SessionBinding};
use omp_rpc::{
    AbortCommand, Client, ClientOptions, Event, GetStateCommand, HostTool, HostUri, ImageContent,
    NewSessionCommand, OpenSessionCommand, OpenSessionResult, PromptCommand, PromptTurn,
    RpcNotification, SessionState, SetAskDialogCommand, SetModelCommand,
};
use std::{
    path::PathBuf,
    process::Command as Process,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

/// How to find the `omp` binary. Bundled path in product; PATH fallback in dev.
#[derive(Debug, Clone)]
pub enum OmpBinary {
    /// Exact bundled path (`cedian.app/Contents/Resources/omp`).
    Bundled(PathBuf),
    /// Resolve via `PATH` (dev only — never in product).
    Path(String),
}

/// Static config for one runtime.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// OMP binary location.
    pub binary: OmpBinary,
    /// `--session-dir` for this workspace (adopt-newest, leaf ephemeral).
    pub session_dir: PathBuf,
    /// Workspace root (cwd for the sidecar).
    pub cwd: PathBuf,
    /// Opt in to the `ask` tool dialog (off by default upstream — without it,
    /// approval-needing tools fail closed; plan §10 + spike row).
    pub ask_dialog: bool,
    /// Prompt round-trip deadline.
    pub prompt_timeout: Duration,
}

/// Lifecycle of the sidecar process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    /// Spawned, handshake done, ready for prompts.
    Ready,
    /// A turn is streaming.
    Streaming,
    /// OMP disconnected / stdout closed — user-visible, restart required.
    Disconnected,
}

/// Owned OMP sidecar: blocking client + pump thread + shared router.
///
/// The router is shared (`Arc`): the pump thread dispatches notification frames
/// into it, panel/task subscribers (Phase 2) read from it. Blocking
/// `prompt_and_wait` runs inline on the caller thread — callers MUST be off the
/// UI thread.
pub struct OmpRuntime {
    client: Option<Arc<Client>>,
    router: Arc<EventRouter>,
    state: RuntimeState,
    session: Option<SessionBinding>,
    stop: Arc<AtomicBool>,
    pump: Option<JoinHandle<()>>,
    config: RuntimeConfig,
}

impl OmpRuntime {
    /// Spawn the sidecar, wait `ready`, negotiate v2 (inside vendored client),
    /// apply `ask_dialog`, start the pump thread.
    pub fn spawn(config: RuntimeConfig) -> Result<Self, OmpError> {
        let binary = match &config.binary {
            OmpBinary::Bundled(path) => path.to_string_lossy().into_owned(),
            OmpBinary::Path(name) => name.clone(),
        };
        let mut process = Process::new(&binary);
        process.args(["--mode", "rpc-ui"]);
        process.arg("--session-dir").arg(&config.session_dir);
        process.arg("--cwd").arg(&config.cwd);
        let options = ClientOptions {
            default_timeout: Duration::from_secs(30),
            ..Default::default()
        };
        let (client, events) = Client::spawn(process, options).map_err(|e| match e {
            omp_rpc::client::Error::Io(io) => OmpError::Spawn(io.to_string()),
            other => OmpError::Handshake(other.to_string()),
        })?;
        let client = Arc::new(client);
        let router = Arc::new(EventRouter::new());
        let stop = Arc::new(AtomicBool::new(false));

        // Pump: notification frames → router classification. Correlated
        // responses are consumed inside the vendored client, never here.
        let pump_router = Arc::clone(&router);
        let pump_stop = Arc::clone(&stop);
        let pump = std::thread::spawn(move || {
            for event in events {
                if pump_stop.load(Ordering::Relaxed) {
                    break;
                }
                if let Event::Notification(frame) = event {
                    pump_router.dispatch_notification(&frame);
                }
            }
        });

        let runtime = Self {
            client: Some(client),
            router,
            state: RuntimeState::Ready,
            session: None,
            stop,
            pump: Some(pump),
            config,
        };
        if runtime.config.ask_dialog {
            runtime
                .client()
                .call(&SetAskDialogCommand { enabled: true })
                .map_err(OmpError::from)?;
        }
        Ok(runtime)
    }

    /// Validated spawn from a [`RespawnRequest`] (crash recovery path).
    pub fn respawn(
        req: &RespawnRequest,
        ask_dialog: bool,
        prompt_timeout: Duration,
    ) -> Result<Self, OmpError> {
        let v = crate::respawn::validate(req)?;
        // argv[0] is the validated binary path.
        let config = RuntimeConfig {
            binary: OmpBinary::Bundled(PathBuf::from(&v.argv[0])),
            session_dir: req.session_dir.clone(),
            cwd: req.cwd.clone(),
            ask_dialog,
            prompt_timeout,
        };
        Self::spawn(config)
    }
    /// Borrow the client (shutdown takes it; all ops require it present).
    fn client(&self) -> &Arc<Client> {
        self.client.as_ref().expect("client present until shutdown")
    }

    /// Current lifecycle state.
    pub fn state(&self) -> RuntimeState {
        self.state
    }

    /// Shared router for panel/task subscribers (Phase 2).
    pub fn router(&self) -> Arc<EventRouter> {
        Arc::clone(&self.router)
    }

    /// Full router log for crash reconciliation (§74 R3).
    pub fn event_log(&self) -> Vec<crate::LogEntry> {
        self.router.log_snapshot()
    }

    /// Send a prompt; block until its `prompt_result` (or timeout). Images are
    /// base64 payloads (`ImageContent`), same shape the spike proved.
    pub fn prompt(
        &mut self,
        message: &str,
        images: Vec<ImageContent>,
    ) -> Result<PromptTurn, OmpError> {
        self.state = RuntimeState::Streaming;
        let cmd = PromptCommand {
            message: message.to_string(),
            images: if images.is_empty() {
                None
            } else {
                Some(images)
            },
            streaming_behavior: None,
        };
        let turn = self
            .client()
            .prompt_and_wait(&cmd, self.config.prompt_timeout)
            .map_err(OmpError::from)?;
        self.state = RuntimeState::Ready;
        Ok(turn)
    }

    /// Abort the running turn.
    pub fn abort(&mut self) -> Result<(), OmpError> {
        self.client()
            .call(&AbortCommand {})
            .map_err(OmpError::from)?;
        self.state = RuntimeState::Ready;
        Ok(())
    }

    /// Adopt (or start) the newest session in `session_dir`; records the
    /// workspace↔session binding. Call before first prompt when restoring
    /// (plan §75: directory-adopt, not file-restore).
    pub fn open_session(&mut self, task: &str) -> Result<OpenSessionResult, OmpError> {
        let result: OpenSessionResult = self
            .client()
            .call(&OpenSessionCommand {
                session_dir: self.config.session_dir.to_string_lossy().into_owned(),
                provider: None,
                model_id: None,
            })
            .map_err(OmpError::from)?;
        self.session = Some(SessionBinding::new(
            self.config.cwd.clone(),
            self.config.session_dir.clone(),
            result.session_id.clone(),
            task.to_string(),
        ));
        Ok(result)
    }

    /// Start a fresh session, optionally under a parent. Returns `true` when
    /// the switch happened (`cancelled: false`).
    pub fn new_session(&mut self, parent: Option<String>, task: &str) -> Result<bool, OmpError> {
        let result = self
            .client()
            .call(&NewSessionCommand {
                parent_session: parent,
            })
            .map_err(OmpError::from)?;
        if result.cancelled {
            return Ok(false);
        }
        let state: SessionState = self
            .client()
            .call(&GetStateCommand {})
            .map_err(OmpError::from)?;
        self.session = Some(SessionBinding::new(
            self.config.cwd.clone(),
            self.config.session_dir.clone(),
            state.session_id.clone(),
            task.to_string(),
        ));
        Ok(true)
    }

    /// Switch the session model (`provider/id` pair, both required upstream).
    pub fn set_model(&self, provider: &str, model_id: &str) -> Result<(), OmpError> {
        self.client()
            .call(&SetModelCommand {
                provider: provider.to_string(),
                model_id: model_id.to_string(),
            })
            .map_err(OmpError::from)?;
        Ok(())
    }

    /// Current session snapshot (model, streaming, queue, todos, usage).
    pub fn get_state(&self) -> Result<SessionState, OmpError> {
        self.client()
            .call(&GetStateCommand {})
            .map_err(OmpError::from)
    }

    /// Replace the host-owned tool set (whole-set replace semantics upstream).
    pub fn set_host_tools(&self, tools: Vec<HostTool>) -> Result<Vec<String>, OmpError> {
        self.client()
            .set_custom_tools(tools)
            .map_err(OmpError::from)
    }

    /// Replace the host-owned URI scheme set.
    pub fn set_host_uris(&self, uris: Vec<HostUri>) -> Result<Vec<String>, OmpError> {
        self.client().set_host_uris(uris).map_err(OmpError::from)
    }

    /// Active workspace↔session binding, if adopted.
    pub fn session(&self) -> Option<&SessionBinding> {
        self.session.as_ref()
    }

    /// Shut down: close the client first (stdin EOF + SIGTERM→1s→SIGKILL
    /// group teardown, which closes the event channel), then join the pump.
    /// Order matters — joining the pump first deadlocks: the channel only
    /// closes after the client does.
    pub fn shutdown(mut self) -> Result<(), OmpError> {
        self.stop.store(true, Ordering::Relaxed);
        let client = self.client.take().expect("shutdown consumes the client");
        let client = Arc::try_unwrap(client)
            .map_err(|_| OmpError::Transport("client still shared".to_string()))?;
        let result = client.close().map_err(OmpError::from);
        if let Some(pump) = self.pump.take() {
            let _ = pump.join();
        }
        result
    }
}

/// Build an image payload from raw bytes (base64-encoded on the wire).
pub fn image_content(mime_type: &str, bytes: &[u8]) -> ImageContent {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    ImageContent {
        r#type: None,
        data: Some(STANDARD.encode(bytes)),
        mime_type: Some(mime_type.to_string()),
        detail: None,
        url: None,
        provider_file: Default::default(),
        extra: Default::default(),
    }
}

/// Last assistant text of a prompt turn (thinking excluded by the client).
pub fn last_text(turn: &PromptTurn) -> Option<&str> {
    turn.assistant_text.as_deref()
}

/// Suppress unused-import lint while `RpcNotification` shapes the Phase 2 API.
#[allow(dead_code)]
fn _notification_shape(_: &RpcNotification) {}
