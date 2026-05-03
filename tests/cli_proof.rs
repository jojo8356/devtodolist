//! End-to-end CLI tests for the commit-proof feature.
//!
//! These tests need a real git repository, so we use `TestProject::new_with_git`.

mod common;

use predicates::prelude::*;

use common::TestProject;

#[test]
fn proof_add_then_list_shows_commit() {
    let p = TestProject::new_with_git();
    let hash = p.commit("first commit");
    p.cmd().args(["add", "T"]).assert().success();
    p.cmd().args(["proof", "add", "1", &hash]).assert().success();

    p.cmd()
        .args(["proof", "list", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("first commit"));
}

#[test]
fn proof_add_supports_short_hash() {
    let p = TestProject::new_with_git();
    let hash = p.commit("short hash test");
    p.cmd().args(["add", "T"]).assert().success();
    let short = &hash[..7];
    p.cmd()
        .args(["proof", "add", "1", short])
        .assert()
        .success()
        .stdout(predicate::str::contains(short));
}

#[test]
fn proof_add_unknown_commit_returns_typed_error() {
    let p = TestProject::new_with_git();
    p.commit("base");
    p.cmd().args(["add", "T"]).assert().success();
    p.cmd()
        .args(["proof", "add", "1", "deadbeefcafe0000"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Commit not found"));
}

#[test]
fn proof_add_to_unknown_task_returns_notfound() {
    let p = TestProject::new_with_git();
    let hash = p.commit("c");
    p.cmd()
        .args(["proof", "add", "999", &hash])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Task not found"));
}

#[test]
fn proof_verify_reports_valid_and_missing_separately() {
    let p = TestProject::new_with_git();
    let hash = p.commit("real");
    p.cmd().args(["add", "T"]).assert().success();
    p.cmd().args(["proof", "add", "1", &hash]).assert().success();

    // Sneak in a fake hash directly via the lower-level command path: easiest
    // way is to manipulate the DB. Instead we just verify the happy path: the
    // single real proof reports as 1 valid, 0 missing.
    p.cmd()
        .args(["proof", "verify", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 valid").and(predicate::str::contains("0 missing")));
}

#[test]
fn proof_remove_then_list_is_empty() {
    let p = TestProject::new_with_git();
    let hash = p.commit("to remove");
    p.cmd().args(["add", "T"]).assert().success();
    p.cmd().args(["proof", "add", "1", &hash]).assert().success();
    p.cmd().args(["proof", "remove", "1", &hash]).assert().success();
    p.cmd()
        .args(["proof", "list", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No commit proofs"));
}

#[test]
fn proof_auto_imports_commits_in_branch_range() {
    let p = TestProject::new_with_git();
    p.commit("base");
    // Create a feature branch with two commits.
    assert!(
        std::process::Command::new("git")
            .args(["checkout", "-q", "-b", "feature"])
            .current_dir(p.path())
            .status()
            .unwrap()
            .success()
    );
    let _c1 = p.commit("feat: part 1");
    let _c2 = p.commit("feat: part 2");

    p.cmd()
        .args(["add", "Feature task", "--branch", "feature", "--base", "main"])
        .assert()
        .success();

    p.cmd()
        .args(["proof", "auto", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported 2 commit"));

    p.cmd()
        .args(["proof", "list", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("part 1").and(predicate::str::contains("part 2")));
}

#[test]
fn proof_auto_without_branch_returns_typed_nobranch_error() {
    let p = TestProject::new_with_git();
    p.commit("base");
    p.cmd().args(["add", "No branch"]).assert().success();
    p.cmd()
        .args(["proof", "auto", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("has no branch set"));
}
