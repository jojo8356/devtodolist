//! Shared helpers for CLI integration tests.
//!
//! Each test uses a fresh `tempfile::TempDir` as the working directory so the
//! `.devtodo.db` file is fully isolated between tests.

// Each `tests/*.rs` is compiled as its own crate, so helpers that are unused
// in a given test file get flagged. Suppress the warning at module level.
#![allow(dead_code)]

use std::path::Path;
use std::process::Command;

use assert_cmd::Command as AssertCommand;
use tempfile::TempDir;

/// A scratch project: a temp directory with `.devtodo.db` already initialized.
/// Drop cleans up automatically.
pub struct TestProject {
    pub dir: TempDir,
}

impl TestProject {
    /// Create a temp dir and run `devtodo init` inside it.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("create tempdir");
        let project = Self { dir };
        project.cmd().arg("init").assert().success();
        project
    }

    /// Same as `new()` but also initializes a real git repo (needed by `proof`).
    pub fn new_with_git() -> Self {
        let project = Self::new();
        // Configure a deterministic identity so commits don't depend on global git config.
        for args in [
            &["init", "-q", "-b", "main"][..],
            &["config", "user.email", "test@example.com"][..],
            &["config", "user.name", "Test"][..],
            &["config", "commit.gpgsign", "false"][..],
        ] {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(project.path())
                    .status()
                    .expect("run git")
                    .success(),
                "git {:?} failed",
                args
            );
        }
        project
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Returns a fresh `assert_cmd::Command` for the `devtodo` binary already
    /// pointed at the temp project directory.
    pub fn cmd(&self) -> AssertCommand {
        let mut cmd = AssertCommand::cargo_bin("devtodo").expect("locate devtodo binary");
        cmd.current_dir(self.path());
        cmd
    }

    /// Create one commit in the git repo and return its full hash.
    pub fn commit(&self, message: &str) -> String {
        let file = self.path().join(format!("file-{message}.txt"));
        std::fs::write(&file, message).expect("write file");
        for args in [
            &["add", "."][..],
            &["commit", "-q", "-m", message][..],
        ] {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(self.path())
                    .status()
                    .expect("run git")
                    .success(),
                "git {:?} failed",
                args
            );
        }
        let out = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(self.path())
            .output()
            .expect("run git rev-parse");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }
}
