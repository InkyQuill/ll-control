//! `verify-galahad` — Phase 0 hardware test harness for the
//! Galahad II LCDV2.
//!
//! This crate is a binary-with-helper-lib: `main.rs` is the CLI entry
//! point; `lib.rs` exposes helper modules (codecs, parsers) used by
//! integration tests.

pub mod probe;
