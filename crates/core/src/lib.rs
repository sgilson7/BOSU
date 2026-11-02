//! tool_core: every application rule of the tool.
//!
//! Builder requirement: this crate performs no input or output. It takes
//! values and returns values, so `cargo test` reaches every rule without a
//! browser. `tests/boundary.rs` enforces that, along with the dependency
//! allowlist and the reference ban on floats, hash maps and clocks.
//!
//! `game` is the one door the shim calls through; `play` judges, `chart`
//! reads charts, `beats` and `generate` make them at build time.

pub mod beats;
pub mod catalog;
pub mod chart;
pub mod draw;
pub mod fx;
pub mod game;
pub mod generate;
pub mod glossary;
pub mod page;
pub mod play;
pub mod rng;
pub mod sound;
pub mod tuning;
