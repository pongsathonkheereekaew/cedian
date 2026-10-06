//! `fake-omp`: see the crate docs. Argv is whatever cedian's spawn profile
//! passes (`--mode rpc-ui --session-dir … --cwd … --approval-mode … --config …`).

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(cedian_fake_omp::run(&args));
}
