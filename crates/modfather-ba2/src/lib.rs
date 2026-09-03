//! `modfather-ba2` — Bethesda BA2 (GNRL/DX10) archive extension.
//!
//! **Separate deliverable** from the `sevenzip-re` standalone package, same
//! rationale as `modfather-bsa`: BA2 is Bethesda's own container format,
//! not 7z, and must never be folded into the standalone engine.
//!
//! Status: Wave 0 scaffold only. The reference implementation's GNRL/DX10
//! byte layouts (36-byte GNRL records, 24-byte DX10 headers + 24-byte chunk
//! records) and its two known gaps — (a) `ZlibDecoder` used unconditionally
//! even though Starfield-era BA2 chunks use LZ4, and (b) the writer only
//! ever emits uncompressed GNRL archives — are documented in the project's
//! working notes as the redesign targets for this crate's read/write paths.
//! Implementing and gate-testing them against real BA2 fixtures is tracked
//! as immediate follow-up work (see `docs/SCHEDULE.md`, Wave 0).

pub mod error;

pub use error::{Error, Result};
