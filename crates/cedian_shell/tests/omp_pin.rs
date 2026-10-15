//! ADR-0075 decision 5 and ADR-0077: the app runs the OMP fork's build it
//! ships. A set `CEDIAN_OMP_BINARY` runs as given (tests, dev); else the
//! bundled build (`cedian.app/Contents/Resources/cedian-agent`, or `target/omp/omp` in
//! a dev build) runs only when its stamp names the pinned fork commit
//! and the binary's sha256; else the launch is refused. The person's own
//! `omp` on PATH or in `~/.local/bin` is never run. One test: it sets the
//! process environment.

use sha2::{Digest as _, Sha256};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

fn fake(dir: &Path, version: &str) -> PathBuf {
    fake_named(dir, "omp", version)
}

fn fake_named(dir: &Path, name: &str, version: &str) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\necho omp/{version}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn stamp(omp: &Path, commit: &str) {
    let sha = format!("{:x}", Sha256::digest(std::fs::read(omp).unwrap()));
    let body = serde_json::json!({ "commit": commit, "sha256": sha });
    let name = format!("{}.stamp", omp.file_name().unwrap().to_str().unwrap());
    std::fs::write(omp.with_file_name(name), body.to_string()).unwrap();
}

fn refused(exe: &Path) -> String {
    cedian_shell::launch::omp_binary_for(Some(exe)).expect_err("refused")
}

#[test]
fn the_app_runs_only_the_stamped_fork_build() {
    let root = std::env::temp_dir().join(format!("cedian-omp-pin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let pinned = cedian_shell::launch::pinned_omp_commit();
    assert_eq!(pinned.len(), 40, "the pin names a full fork commit");
    let home = root.join("home");
    fake(&root.join("bin"), "18.8.6");
    fake(&home.join(".local/bin"), "18.8.6");
    let set = |env: Option<&Path>| unsafe {
        // SAFETY: the only test in this binary; nothing else reads the env.
        std::env::set_var("PATH", root.join("bin"));
        std::env::set_var("HOME", &home);
        match env {
            Some(env) => std::env::set_var("CEDIAN_OMP_BINARY", env),
            None => std::env::remove_var("CEDIAN_OMP_BINARY"),
        }
    };

    let app = root.join("bundle/cedian.app");
    let exe = app.join("Contents/MacOS/cedian");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    std::fs::write(&exe, "the app binary").unwrap();

    set(None);
    let error = refused(&exe);
    assert!(
        error.contains("script/cedian-omp-build"),
        "no bundled the agent: refused with the fix, PATH's pinned omp is not run: {error}"
    );

    let sidecar = fake_named(&app.join("Contents/Resources"), "cedian-agent", "18.8.6");
    let error = refused(&exe);
    assert!(
        error.contains("cedian-agent.stamp"),
        "no stamp: refused: {error}"
    );

    stamp(&sidecar, &pinned);
    let choice = cedian_shell::launch::omp_binary_for(Some(&exe)).unwrap();
    assert_eq!(choice.binary, sidecar, "the stamped bundle runs");
    assert_eq!(choice.warning, None);

    let other = "0".repeat(40);
    stamp(&sidecar, &other);
    let error = refused(&exe);
    assert!(
        error.contains(&pinned) && error.contains(&other),
        "a build of another commit is refused, naming both: {error}"
    );

    stamp(&sidecar, &pinned);
    std::fs::write(&sidecar, "#!/bin/sh\necho omp/18.8.6 # changed\n").unwrap();
    let error = refused(&exe);
    assert!(
        error.contains("sha256"),
        "a binary changed after its stamp is refused: {error}"
    );

    // A dev build: `target/<profile>/cedian` and a test's
    // `target/<profile>/deps/<test>` both run `target/omp/omp`.
    let target = root.join("target");
    let dev = fake(&target.join("omp"), "18.8.6");
    stamp(&dev, &pinned);
    for exe in [target.join("debug/cedian"), target.join("debug/deps/t-1")] {
        assert_eq!(
            cedian_shell::launch::omp_binary_for(Some(&exe))
                .unwrap()
                .binary,
            dev,
            "{} runs the dev build",
            exe.display()
        );
    }
    assert!(
        refused(&root.join("elsewhere/bin/cedian")).contains("script/cedian-omp-build"),
        "an executable with no build near it is refused"
    );

    let off_pin = fake(&root.join("env"), "18.9.0");
    set(Some(&off_pin));
    let choice = cedian_shell::launch::omp_binary_for(Some(&exe)).unwrap();
    assert_eq!(
        choice.binary, off_pin,
        "a set CEDIAN_OMP_BINARY runs as given"
    );
    let warning = choice.warning.expect("an off-pin CEDIAN_OMP_BINARY warns");
    assert!(
        warning.contains("18.9.0") && warning.contains("18.8.6"),
        "{warning}"
    );
    let _ = std::fs::remove_dir_all(&root);
}
