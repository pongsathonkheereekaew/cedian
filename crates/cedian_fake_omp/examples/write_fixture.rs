//! Scrub a saved recording into a fixture, as a record lane would, without
//! a model turn: `cargo run -p cedian_fake_omp --example write_fixture --
//! <recorded.jsonl> <fixture.jsonl> <provider/model>`.

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [recorded, fixture, model] = args.as_slice() else {
        eprintln!("usage: write_fixture <recorded.jsonl> <fixture.jsonl> <provider/model>");
        std::process::exit(2);
    };
    cedian_fake_omp::recording::write_fixture_on(Path::new(recorded), Path::new(fixture), model);
}
