//! `modfather-bsa` — Bethesda BSA (v103-105) archive extension.
//!
//! This is a **separate deliverable** from the `sevenzip-re` standalone
//! package: BSA is Bethesda's own container format, not 7z. This crate
//! depends on `sevenzip-re` for shared primitives (CRC handling, error
//! conventions) but is never folded into it.
//!
//! Redesigned from the reference implementation's known gaps:
//! - Codec dispatch is **version-aware**: v103/v104 payloads use zlib
//!   (`flate2`), v105 payloads use LZ4 (`lz4_flex`) — the reference crate
//!   used `ZlibDecoder` unconditionally regardless of version, which is
//!   simply wrong for v105 (Skyrim SE/AE) archives.
//! - TES3 (Morrowind) BSA is a different, older format and is intentionally
//!   out of scope here (see `docs/SCHEDULE.md`); this crate only claims
//!   v103-105 (Gamebryo/Creation-engine BSA).
//!
//! Status: Wave 0, read path implemented and gated by tests against a
//! synthetic fixture; write/pack path and real-world fixture gating are
//! tracked as follow-up work.

pub mod error;
pub mod format;
pub mod reader;

pub use error::{Error, Result};
pub use reader::{BsaArchive, BsaEntry};
