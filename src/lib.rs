//! Library crate for `devtodo`, exposing the modules so integration tests
//! under `tests/` can exercise the public API.
//!
//! The binary (`src/main.rs`) also pulls these modules in via `pub mod`
//! declarations; this file exists purely so `cargo test` can compile a
//! library target against which the integration tests link.

pub mod cli;
pub mod commands;
pub mod db;
pub mod display;
pub mod error;
pub mod gamification;
pub mod models;
pub mod providers;
