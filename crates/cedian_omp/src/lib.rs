//! `cedian_omp`: process + protocol boundary to the bundled OMP sidecar.
//!
//! Plan §§4–6: `cedian.app` spawns `omp --mode rpc-ui` over stdio, negotiates
//! RPC v2 through the vendored upstream client (`vendor/omp-rpc`), and fans
//! session events out through [`EventRouter`]. Blocking client calls live on
//! caller threads off-UI — never the UI thread (plan §5).

pub mod errors;
pub mod event_router;
pub mod runtime;
pub mod session;
pub mod spawn_profile;

pub use errors::OmpError;
pub use event_router::{
    DeltaKind, EventRouter, FinishedToolCall, LogEntry, PromptStatus, RouterEvent,
};
pub use runtime::{
    image_content, last_text, OmpBinary, OmpRuntime, RuntimeConfig, RuntimeControl, RuntimeState,
};
pub use session::{
    validate_binding, ResumeState, SessionBinding, SnapshotVersionMismatch, SNAPSHOT_VERSION,
};
pub use spawn_profile::{
    resolve_on_path, scrub_env, ApprovalMode, BashRule, SpawnPlan, SpawnPolicy, SpawnProfile,
    ToolPolicy,
};
