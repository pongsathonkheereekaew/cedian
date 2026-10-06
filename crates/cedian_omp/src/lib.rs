//! `cedian_omp`: process + protocol boundary to the bundled OMP sidecar.
//!
//! Plan §§4–6: `cedian.app` spawns `omp --mode rpc-ui` over stdio, negotiates
//! RPC v2 through the vendored upstream client (`vendor/omp-rpc`), and fans
//! session events out through [`EventRouter`]. Blocking client calls live on
//! caller threads off-UI — never the UI thread (plan §5).

pub mod errors;
pub mod event_router;
pub mod respawn;
pub mod runtime;
pub mod session;

pub use errors::OmpError;
pub use event_router::{DeltaKind, EventRouter, LogEntry, PromptStatus, RouterEvent};
pub use respawn::{validate, RespawnRequest, RespawnValidation};
pub use runtime::{image_content, last_text, OmpBinary, OmpRuntime, RuntimeConfig, RuntimeState};
pub use session::{ResumeState, SessionBinding, SNAPSHOT_VERSION};
