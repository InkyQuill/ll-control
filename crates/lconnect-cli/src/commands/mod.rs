//! Subcommand implementations.
//!
//! Each submodule is a stub until its corresponding P1E-* task lands.
//! Stubs expose the function signatures `main.rs` dispatches to so the
//! binary builds today; bodies return `anyhow::bail!` until replaced.

pub mod devices;
pub mod lcd;
pub mod lighting;
pub mod maintenance;
pub mod pwm;
pub mod watch;
