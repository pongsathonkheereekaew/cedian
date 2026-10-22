//! `cedian-daemon`: serves windows on the socket in cedian's state directory until it is
//! killed. `--printenv` prints this process's environment as JSON, for reading the login
//! shell's environment.

use std::path::PathBuf;

use cedian_daemon::{Cedia, Config, login_environment, serve};

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--printenv") {
        util::shell_env::print_env();
        return;
    }
    log::set_logger(&LOGGER).ok();
    log::set_max_level(log::LevelFilter::Info);
    let env = login_environment();
    let home = env
        .get("CEDIA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(cedian_index::cedia_home);
    let config = Config {
        socket: cedian_wire::socket_path(),
        cedia: Cedia::find(env),
        home,
    };
    if let Err(error) = serve(config) {
        log::error!("{error:#}");
        std::process::exit(1);
    }
}

struct Logger;

static LOGGER: Logger = Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            eprintln!("{} {}: {}", record.level(), record.target(), record.args());
        }
    }

    fn flush(&self) {}
}
