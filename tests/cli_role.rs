//! End-to-end CLI tests for the developer-role feature.

mod common;

use predicates::prelude::*;
use rstest::rstest;

use common::TestProject;

#[test]
fn role_set_and_list_round_trip() {
    let p = TestProject::new();
    p.cmd().args(["role", "set", "alice", "backend"]).assert().success();
    p.cmd().args(["role", "set", "bob", "frontend"]).assert().success();

    p.cmd()
        .args(["role", "list"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("alice")
                .and(predicate::str::contains("backend"))
                .and(predicate::str::contains("bob"))
                .and(predicate::str::contains("frontend")),
        );
}

#[test]
fn role_set_is_an_upsert() {
    let p = TestProject::new();
    p.cmd().args(["role", "set", "alice", "backend"]).assert().success();
    p.cmd().args(["role", "set", "alice", "fullstack"]).assert().success();
    p.cmd()
        .args(["role", "get", "alice"])
        .assert()
        .success()
        .stdout(predicate::str::contains("fullstack"));
}

#[test]
fn role_remove_unknown_user_fails_with_notfound() {
    let p = TestProject::new();
    p.cmd()
        .args(["role", "remove", "ghost"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[rstest]
#[case::backend("backend", "Backend task", vec!["Frontend task", "Unassigned"])]
#[case::frontend("frontend", "Frontend task", vec!["Backend task", "Unassigned"])]
fn list_filter_by_role(
    #[case] role: &str,
    #[case] expected: &str,
    #[case] absent: Vec<&str>,
) {
    let p = TestProject::new();
    p.cmd().args(["add", "Backend task", "--assignee", "alice"]).assert().success();
    p.cmd().args(["add", "Frontend task", "--assignee", "bob"]).assert().success();
    p.cmd().args(["add", "Unassigned"]).assert().success();
    p.cmd().args(["role", "set", "alice", "backend"]).assert().success();
    p.cmd().args(["role", "set", "bob", "frontend"]).assert().success();

    let output = p.cmd().args(["list", "--role", role]).assert().success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout).to_string();
    assert!(stdout.contains(expected), "expected '{expected}':\n{stdout}");
    for missing in absent {
        assert!(!stdout.contains(missing), "should not contain '{missing}':\n{stdout}");
    }
}
