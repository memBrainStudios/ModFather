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
//!   out of scope here for now, pending a scope decision (see
//!   `docs/SCHEDULE.md`'s Wave 0 "TES3 (Morrowind) BSA scope" note for the
//!   two options and why it does not gate Wave 0 either way); this crate
//!   only claims v103-105 (Gamebryo/Creation-engine BSA).
//!
//! Status: Wave 0. Read path (v103-105, version-aware zlib/LZ4) and write
//! path (spec-conforming hash-sorted pack, matching the reader's codec
//! choice) are both implemented and unit-tested against synthetic
//! fixtures, including a full write-then-read round trip. Real-world
//! fixture gating (against an actual game BSA) is tracked as follow-up
//! work.

pub mod container;
pub mod error;
pub mod format;
pub mod hash;
pub mod reader;
pub mod writer;

pub use error::{Error, Result};
pub use reader::{BsaArchive, BsaEntry};
pub use writer::{write, FileToPack, WriteOptions};
