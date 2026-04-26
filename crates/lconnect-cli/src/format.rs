//! Output formatters: human-readable text vs --json mode.

// Unused until P1E-2..P1E-7 land the per-subcommand bodies that call them.
#![allow(dead_code)]

use serde::Serialize;

/// Print a value either as JSON (when `json` is true) or fall back to a
/// `Debug` rendering. Per-command pretty-printers replace the Debug path
/// as P1E-2..P1E-7 land.
pub fn print<T: Serialize + std::fmt::Debug>(value: &T, json: bool) {
    if json {
        match serde_json::to_string(value) {
            Ok(s) => println!("{s}"),
            Err(e) => eprintln!("error: failed to serialise as JSON: {e}"),
        }
    } else {
        // For now, fall back to Debug. Per-command pretty-printers replace this
        // as P1E-2..P1E-7 land. Each command can call print_human(...) instead.
        println!("{value:#?}");
    }
}

/// Emit a single NDJSON line for streaming subcommands (`watch`).
pub fn print_ndjson_line<T: Serialize>(value: &T) {
    match serde_json::to_string(value) {
        Ok(s) => println!("{s}"),
        Err(e) => eprintln!("error: failed to serialise NDJSON: {e}"),
    }
}
