//! cedia's protocol, generated from `cedia/protocol.schema.json` at the revision in `cedia/REV`.
//! `script/pin-cedia` moves the pin.

mod event_ids;
mod line;

#[allow(clippy::all, clippy::pedantic, dead_code, unused_imports)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/protocol.rs"));
}

pub use generated::*;
pub use line::{Line, LineError};

/// The cedia revision these types and fixtures come from.
pub const CEDIA_REV: &str = include_str!("../cedia/REV");

/// Where cedia's recorded session fixtures live in this checkout.
pub fn fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("cedia/sessions")
}
