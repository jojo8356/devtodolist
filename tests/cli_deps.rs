//! End-to-end CLI tests for the dependency feature.

mod common;

use predicates::prelude::*;
use rstest::rstest;

use common::TestProject;

fn add_task(p: &TestProject, title: &str) {
    p.cmd().args(["add", title]).assert().success();
}

#[test]
fn deps_add_then_list_shows_dependency() {
    let p = TestProject::new();
    add_task(&p, "Setup auth");
    add_task(&p, "Build login");
    p.cmd().args(["deps", "add", "2", "1"]).assert().success();

    p.cmd()
        .args(["deps", "list", "2"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Setup auth"));
}

#[test]
fn deps_self_loop_is_rejected_with_clear_message() {
    let p = TestProject::new();
    add_task(&p, "Solo");
    p.cmd()
        .args(["deps", "add", "1", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot depend on itself"));
}

#[test]
fn deps_direct_cycle_is_rejected() {
    let p = TestProject::new();
    add_task(&p, "A");
    add_task(&p, "B");
    p.cmd().args(["deps", "add", "1", "2"]).assert().success();
    p.cmd()
        .args(["deps", "add", "2", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("create a cycle"));
}

#[test]
fn deps_transitive_cycle_is_rejected() {
    let p = TestProject::new();
    add_task(&p, "A");
    add_task(&p, "B");
    add_task(&p, "C");
    p.cmd().args(["deps", "add", "1", "2"]).assert().success();
    p.cmd().args(["deps", "add", "2", "3"]).assert().success();
    // 1 -> 2 -> 3; closing 3 -> 1 must fail.
    p.cmd()
        .args(["deps", "add", "3", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cycle"));
}

#[test]
fn deps_remove_then_list_is_empty() {
    let p = TestProject::new();
    add_task(&p, "A");
    add_task(&p, "B");
    p.cmd().args(["deps", "add", "2", "1"]).assert().success();
    p.cmd().args(["deps", "remove", "2", "1"]).assert().success();
    p.cmd()
        .args(["deps", "list", "2"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no dependencies"));
}

#[test]
fn deps_dependents_lists_blocking_relationships() {
    let p = TestProject::new();
    add_task(&p, "A");
    add_task(&p, "B");
    add_task(&p, "C");
    p.cmd().args(["deps", "add", "2", "1"]).assert().success();
    p.cmd().args(["deps", "add", "3", "1"]).assert().success();

    p.cmd()
        .args(["deps", "dependents", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("B").and(predicate::str::contains("C")));
}

#[test]
fn deps_tree_renders_recursively() {
    let p = TestProject::new();
    add_task(&p, "root");
    add_task(&p, "mid");
    add_task(&p, "leaf");
    p.cmd().args(["deps", "add", "1", "2"]).assert().success();
    p.cmd().args(["deps", "add", "2", "3"]).assert().success();

    p.cmd()
        .args(["deps", "tree", "1"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("root")
                .and(predicate::str::contains("mid"))
                .and(predicate::str::contains("leaf")),
        );
}

#[rstest]
#[case::has_deps("--has-deps", vec!["task-beta"], vec!["task-alpha", "task-gamma"])]
#[case::no_deps("--no-deps", vec!["task-alpha", "task-gamma"], vec!["task-beta"])]
fn list_deps_filters(
    #[case] flag: &str,
    #[case] expected_present: Vec<&str>,
    #[case] expected_absent: Vec<&str>,
) {
    // Use distinctive titles so substring matches don't collide with table
    // headers like "Branch" or "Assignee".
    let p = TestProject::new();
    add_task(&p, "task-alpha");
    add_task(&p, "task-beta");
    add_task(&p, "task-gamma");
    p.cmd().args(["deps", "add", "2", "1"]).assert().success();

    let output = p.cmd().args(["list", flag]).assert().success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout).to_string();
    for s in expected_present {
        assert!(stdout.contains(s), "expected '{s}' in output:\n{stdout}");
    }
    for s in expected_absent {
        assert!(
            !stdout.contains(s),
            "did not expect '{s}' in output:\n{stdout}"
        );
    }
}

#[test]
fn list_blocked_and_ready_track_parent_status() {
    let p = TestProject::new();
    add_task(&p, "parent");
    add_task(&p, "child");
    p.cmd().args(["deps", "add", "2", "1"]).assert().success();

    // Parent is open ⇒ child is blocked.
    p.cmd()
        .args(["list", "--blocked"])
        .assert()
        .success()
        .stdout(predicate::str::contains("child"));

    // Merge parent ⇒ child becomes ready.
    p.cmd().args(["status", "1", "merged"]).assert().success();
    p.cmd()
        .args(["list", "--ready"])
        .assert()
        .success()
        .stdout(predicate::str::contains("child"));
}
