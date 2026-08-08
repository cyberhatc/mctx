//! Packaging wrapper for the standalone, single-file `.mctx` implementation
//! at the repository root (`src/mctx.rs`). That file is the canonical,
//! zero-dependency source — it also compiles directly with `rustc` — and is
//! re-used here so the format logic lives in exactly one place.

#[path = "../../src/mctx.rs"]
pub mod mctx_core;

pub use mctx_core::*;
