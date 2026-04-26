//! lconnect-daemon library surface.
//!
//! The crate ships both a binary (`lconnectd`, see `src/main.rs`) and a
//! library so integration tests can exercise internal modules directly.
//!
//! Modules are pre-declared here so that follow-up tasks can fill them in
//! parallel without contention on this file.

#![forbid(unsafe_code)]

pub mod actor;
pub mod config;
pub mod persistence;
