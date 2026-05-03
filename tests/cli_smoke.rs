//! Cross-cutting smoke tests: existing core flows still work after the new
//! features and the migration system.

mod common;

use predicates::prelude::*;

use common::TestProject;

#[test]
fn init_creates_db_and_help_lists_new_commands() {
    let p = TestProject::new();
    assert!(p.path().join(".devtodo.db").exists());

    p.cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("deps")
                .and(predicate::str::contains("role"))
                .and(predicate::str::contains("proof")),
        );
}

#[test]
fn init_is_idempotent_thanks_to_versioned_migrations() {
    let p = TestProject::new();
    // Re-running init must not corrupt the DB or bump versions past expected.
    p.cmd().arg("init").assert().success();
    p.cmd().arg("init").assert().success();
    p.cmd().args(["add", "Still works"]).assert().success();
    p.cmd()
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Still works"));
}

#[test]
fn add_then_list_then_show_basic_flow() {
    let p = TestProject::new();
    p.cmd()
        .args(["add", "First task", "--priority", "high"])
        .assert()
        .success();
    p.cmd()
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("First task"));
    p.cmd()
        .args(["show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("First task").and(predicate::str::contains("high")));
}

#[test]
fn unknown_task_returns_notfound_not_generic_db_error() {
    let p = TestProject::new();
    p.cmd()
        .args(["show", "999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Task not found"));
}
