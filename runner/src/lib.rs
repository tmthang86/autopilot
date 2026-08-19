//! Autopilot: an unattended delivery runner.
//!
//! One task per invocation. The modules below are the runner; `main.rs` is a
//! thin argument parser that dispatches to them.

pub mod config;
pub mod config_types;
pub mod gh;
pub mod git;
pub mod guard;
pub mod harness;
pub mod intent;
pub mod journal;
pub mod label;
pub mod log;
pub mod queue;
pub mod queue_helpers;
pub mod spawn;
pub mod state;
pub mod tier;
pub mod verify;
