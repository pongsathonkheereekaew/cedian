//! The first call of every `harness = false` test `main` that doubles as
//! fake-omp: it serves the fake-omp entries, lets a test run through, and
//! exits 2 on anything else. Zed's environment loader re-executes the
//! current binary as `<exe> --printenv` in its own session; a test main that
//! ran its scenario for that started another OMP per launch (in a record
//! lane, another model turn).

const ACCEPTS: &str = "no arguments, or the libtest flags cargo forwards \
     (--nocapture, --show-output, --test-threads[=]N with N > 0, --quiet/-q, --exact, \
     --color[=]auto|always|never) and filter strings not starting with '-' (ignored: the \
     whole binary runs)";

/// Serves `--mode` / `--version` / `config` as fake-omp and exits; returns
/// when the arguments are a test run; exits 2 otherwise.
pub fn enter(args: &[String]) {
    match args.first().map(String::as_str) {
        Some("--mode") => std::process::exit(crate::run(args)),
        Some("--version") => std::process::exit(crate::version()),
        Some("config") => std::process::exit(crate::config_get(args)),
        _ => {}
    }
    if let Err(arg) = test_run(args) {
        eprintln!("{arg}: not a test run; this binary accepts {ACCEPTS}");
        std::process::exit(2);
    }
}

/// `Err` names the first argument a test run does not take.
pub fn test_run(args: &[String]) -> Result<(), String> {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--nocapture" | "--show-output" | "--quiet" | "-q" | "--exact" => {}
            "--test-threads" | "--color" => match args.next() {
                Some(value) if valid_value(arg, value) => {}
                Some(value) => return Err(value.clone()),
                None => return Err(arg.clone()),
            },
            a if a
                .split_once('=')
                .is_some_and(|(flag, value)| valid_value(flag, value)) => {}
            a if !a.is_empty() && !a.starts_with('-') => {}
            _ => return Err(arg.clone()),
        }
    }
    Ok(())
}

fn valid_value(flag: &str, value: &str) -> bool {
    match flag {
        "--test-threads" => value.parse::<u32>().is_ok_and(|n| n > 0),
        "--color" => matches!(value, "auto" | "always" | "never"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::test_run;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn accepts_a_test_run() {
        for ok in [
            &[][..],
            &["--nocapture"],
            &["--test-threads", "1", "-q"],
            &["--test-threads=4", "--exact", "some_filter"],
            &["--color", "never", "--show-output"],
        ] {
            assert_eq!(test_run(&args(ok)), Ok(()), "{ok:?}");
        }
    }

    #[test]
    fn refuses_anything_else() {
        for (bad, first) in [
            (&["--printenv"][..], "--printenv"),
            (&["--nocapture", "--printenv"], "--printenv"),
            (&["--list"], "--list"),
            (&["--test-threads"], "--test-threads"),
            (&["--test-threads", "--printenv"], "--printenv"),
            (&["--test-threads", "0"], "0"),
            (&["--color", "--printenv"], "--printenv"),
            (&["--test-threads=x"], "--test-threads=x"),
            (&["--color=sometimes"], "--color=sometimes"),
            (&[""], ""),
        ] {
            assert_eq!(test_run(&args(bad)), Err(first.to_string()), "{bad:?}");
        }
    }
}
