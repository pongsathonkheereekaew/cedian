//! The daemon: owns every thread's runs and answers windows over a Unix socket. It runs one
//! cedia per prompt, streams the run's lines to the windows that have the thread open, and
//! rebuilds a thread's history from cedia's own files.

mod cedia;
mod daemon;
mod environment;
mod keep_awake;
pub mod launch_agent;
mod run;
mod server;

pub use cedia::Cedia;
pub use environment::login_environment;
pub use server::{Config, serve};
