//! `cedian_fake_omp`: hermetic stand-in for `omp --mode rpc-ui` (§86, P2).
//!
//! One binary, two modes, chosen by marker files inside `--session-dir` (the
//! spawn profile scrubs the environment, so env vars cannot carry the mode):
//!
//! - `fake-omp.record` exists → **record**: proxy to the real OMP binary named
//!   in that file, tee every stdout/stdin line into `fake-omp.recorded.jsonl`.
//! - `fake-omp.replay.jsonl` exists → **replay**: emit the recorded server
//!   frames, consume the host's frames in order, fail loudly on divergence.
//!
//! cedian spawns it through the real `OmpRuntime` + `SpawnProfile` path, so a
//! replay exercises the full boundary (handshake, v2 negotiation, host tools,
//! router) with no model call.

mod fixture;
mod fs_effects;
mod record;
mod replay;

pub use fixture::{Dir, Placeholders, Record};

use std::path::{Path, PathBuf};

/// Marker: its content is the absolute path of the real `omp` to proxy.
pub const RECORD_MARKER: &str = "fake-omp.record";
/// Output of a record run (redacted fixture).
pub const RECORDED_FILE: &str = "fake-omp.recorded.jsonl";
/// Input of a replay run.
pub const REPLAY_FILE: &str = "fake-omp.replay.jsonl";

/// Exit code when the host diverges from the fixture.
pub const EXIT_DIVERGED: i32 = 3;

/// Entry point: `args` excludes the program name. Returns the exit code.
pub fn run(args: &[String]) -> i32 {
    let (Some(session_dir), Some(cwd)) = (flag(args, "--session-dir"), flag(args, "--cwd")) else {
        eprintln!("fake-omp: --session-dir and --cwd are required");
        return 2;
    };
    let placeholders = Placeholders::new(&session_dir, &cwd, std::env::var("HOME").ok());
    let marker = session_dir.join(RECORD_MARKER);
    if let Ok(real) = std::fs::read_to_string(&marker) {
        let real = PathBuf::from(real.trim());
        return record::run(
            &real,
            args,
            &session_dir.join(RECORDED_FILE),
            &cwd,
            &placeholders,
        );
    }
    let fixture = session_dir.join(REPLAY_FILE);
    if fixture.is_file() {
        return replay::run(&fixture, &cwd, &placeholders);
    }
    eprintln!(
        "fake-omp: neither {} nor {} in {}",
        RECORD_MARKER,
        REPLAY_FILE,
        session_dir.display()
    );
    2
}

fn flag(args: &[String], name: &str) -> Option<PathBuf> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
}

/// Install `fixture` (a committed, redacted JSONL file) as the replay input
/// for a runtime that will use `session_dir`.
pub fn install_replay(session_dir: &Path, fixture: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(session_dir)?;
    std::fs::copy(fixture, session_dir.join(REPLAY_FILE)).map(|_| ())
}

/// Arm record mode: the next spawn in `session_dir` proxies to `real_omp`.
pub fn arm_record(session_dir: &Path, real_omp: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(session_dir)?;
    std::fs::write(
        session_dir.join(RECORD_MARKER),
        real_omp.to_string_lossy().as_bytes(),
    )
}
