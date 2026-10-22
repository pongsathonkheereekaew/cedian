use std::{path::Path, process::Command};

use cedian_review::{Decisions, HunkState, Shadow, reject, review};

/// Takes a snapshot the way cedia does: the work tree added to the shadow index, written as a
/// tree.
fn take(shadow: &Path, work: &Path) -> String {
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .env("GIT_DIR", shadow)
            .env("GIT_WORK_TREE", work)
            .env("GIT_INDEX_FILE", shadow.join("cedia-index"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .current_dir(work)
            .output()
            .expect("git");
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).expect("utf8")
    };
    if !shadow.join("HEAD").exists() {
        git(&["init", "-q"]);
    }
    git(&["add", "-A", "."]);
    git(&["write-tree"]).trim().to_owned()
}

struct Repo {
    _dir: tempfile::TempDir,
    shadow: std::path::PathBuf,
    work: std::path::PathBuf,
}

fn repo() -> Repo {
    let dir = tempfile::tempdir().expect("tempdir");
    let shadow = dir.path().join("shadow");
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).expect("work");
    Repo {
        _dir: dir,
        shadow,
        work,
    }
}

const BEFORE: &str = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\neleven\ntwelve\n";

#[test]
fn hunks_go_stale_when_edited_and_reject_puts_the_old_lines_back() {
    let r = repo();
    std::fs::write(r.work.join("f.txt"), BEFORE).expect("write");
    let base = take(&r.shadow, &r.work);
    let after = BEFORE
        .replace("two\n", "TWO\n")
        .replace("eleven\n", "ELEVEN\n");
    std::fs::write(r.work.join("f.txt"), &after).expect("write");
    std::fs::write(r.work.join("new.txt"), "fresh").expect("write");
    let head = take(&r.shadow, &r.work);
    let shadow = Shadow::new(r.shadow.clone());

    let first = review(&shadow, &base, &head, &r.work, &Decisions::new()).expect("review");
    let paths: Vec<_> = first.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["f.txt", "new.txt"]);
    let f = &first.files[0];
    assert_eq!((f.added, f.removed, f.hunks.len()), (2, 2, 2));
    assert!(f.hunks.iter().all(|h| h.state == HunkState::Pending));

    // The person edits the second hunk by hand: it is not the agent's any more.
    std::fs::write(r.work.join("f.txt"), after.replace("ELEVEN\n", "Eleven!\n")).expect("write");
    let second = review(&shadow, &base, &head, &r.work, &Decisions::new()).expect("review");
    let states: Vec<_> = second.files[0].hunks.iter().map(|h| h.state).collect();
    assert_eq!(states, [HunkState::Pending, HunkState::Stale]);
    assert!(reject(&r.work, &second.files[0].hunks[1]).is_err());

    reject(&r.work, &second.files[0].hunks[0]).expect("reject");
    assert_eq!(
        std::fs::read_to_string(r.work.join("f.txt")).expect("read"),
        BEFORE.replace("eleven\n", "Eleven!\n")
    );

    // A file the agent added goes away, and the newline it lacked does not come back.
    assert_eq!(second.files[1].hunks[0].lines, ["+fresh"]);
    reject(&r.work, &second.files[1].hunks[0]).expect("reject");
    assert!(!r.work.join("new.txt").exists());

    let mut decisions = Decisions::new();
    decisions.insert(second.files[0].hunks[0].id.clone(), HunkState::Rejected);
    let third = review(&shadow, &base, &head, &r.work, &decisions).expect("review");
    assert_eq!(third.files[0].hunks[0].state, HunkState::Rejected);
    assert_eq!(third.files[0].hunks[0].id, first.files[0].hunks[0].id);
}

#[test]
fn a_deleted_file_comes_back_whole() {
    let r = repo();
    std::fs::write(r.work.join("gone.txt"), "a\nb").expect("write");
    let base = take(&r.shadow, &r.work);
    std::fs::remove_file(r.work.join("gone.txt")).expect("remove");
    let head = take(&r.shadow, &r.work);
    let shadow = Shadow::new(r.shadow.clone());
    let found = review(&shadow, &base, &head, &r.work, &Decisions::new()).expect("review");
    let hunk = &found.files[0].hunks[0];
    assert_eq!(
        (found.files[0].path.as_str(), hunk.state),
        ("gone.txt", HunkState::Pending)
    );
    reject(&r.work, hunk).expect("reject");
    assert_eq!(
        std::fs::read_to_string(r.work.join("gone.txt")).expect("read"),
        "a\nb"
    );
}
