//! End-to-end CLI tests for date-range filtering on the `list` command.
//!
//! We can't backdate tasks via the CLI, so we exercise the parser and the
//! filter wiring; the unit tests in `db.rs` cover the actual SQL date math.

mod common;

use predicates::prelude::*;
use rstest::rstest;

use common::TestProject;

#[rstest]
#[case("2025-01-01")]
#[case("2025-01-15T10:30:00")]
#[case("yesterday")]
#[case("today")]
#[case("now")]
#[case("7d")]
#[case("2w")]
#[case("3 days ago")]
#[case("1 week ago")]
#[case("1 year ago")]
fn list_accepts_date_input(#[case] input: &str) {
    let p = TestProject::new();
    p.cmd().args(["add", "T"]).assert().success();
    p.cmd()
        .args(["list", "--created-from", input])
        .assert()
        .success();
}

#[test]
fn list_rejects_garbage_date_with_typed_error() {
    let p = TestProject::new();
    p.cmd().args(["add", "T"]).assert().success();
    p.cmd()
        .args(["list", "--created-from", "definitely-not-a-date"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cannot parse date"));
}

#[test]
fn list_in_recent_range_includes_just_created_task() {
    let p = TestProject::new();
    p.cmd().args(["add", "Recent task"]).assert().success();
    p.cmd()
        .args(["list", "--created-from", "1d"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recent task"));
}

#[test]
fn list_in_distant_past_excludes_today_task() {
    let p = TestProject::new();
    p.cmd().args(["add", "Today task"]).assert().success();
    // Upper bound far in the past should yield no rows.
    let output = p
        .cmd()
        .args(["list", "--created-to", "2000-01-01"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(!stdout.contains("Today task"), "stdout was:\n{stdout}");
}
