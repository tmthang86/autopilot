//! Autopilot: an unattended delivery runner.
//!
//! One task per invocation. The modules below are the runner; `main.rs` is a
//! thin argument parser that dispatches to them.

pub mod config;
pub mod config_types;
pub mod ctl;
pub mod gh;
pub mod git;
pub mod guard;
pub mod harness;
pub mod install;
pub mod install_labels;
pub mod intent;
pub mod journal;
pub mod label;
pub mod log;
pub mod pipeline;
pub mod pipeline_implement;
pub mod pipeline_land;
pub mod pipeline_settle;
pub mod pipeline_steps;
pub mod preflight;
pub mod queue;
pub mod queue_helpers;
pub mod role;
pub mod run_once;
pub mod settle;
pub mod spawn;
pub mod state;
pub mod tier;
pub mod verdict;
pub mod verify;
