//! `modfather-bsa` — Bethesda BSA archive extension, covering every BSA
//! generation Bethesda has shipped.
//!
//! This is a **separate deliverable** from the `sevenzip-re` standalone
//! package: BSA is Bethesda's own container format, not 7z. This crate
//! depends on `sevenzip-re` for shared primitives (CRC handling, error
//! conventions) but is never folded into it.
//!
//! # Architecture: version-named submodules, header-driven dispatch
//!
//! Per this project's own architecture rule -- "each format should contain
//! every version named as such; where feasible they share code, otherwise
//! they're separate partitions gated by whatever test determines which
//! path they take" -- this crate is split into two submodules, one per
//! structurally distinct BSA generation, never by a config flag or file
//! extension:
//!
//! - [`tes3`]: Morrowind's BSA. The **ground truth**: the oldest format,
//!   and the one every later generation either would have built on or, as
//!   actually happened, replaced outright. Uncompressed, no folder
//!   records, a different 64-bit hash algorithm, no version field at all.
//! - [`tes4`]: Oblivion through Skyrim SE/AE (v103-105). A genuine replace,
//!   not an extension of `tes3` -- which is exactly why it is its own
//!   module rather than a branch inside `tes3`'s reader. v103/v104/v105
//!   *do* share code with each other here (folder+file hash-sorted layout,
//!   differing only in per-version codec and padding), because unlike the
//!   tes3/tes4 split, that variation is feasible to express as branches
//!   within one reader/writer.
//!
//! The two families are told apart purely by their on-disk magic bytes --
//! TES3 is `0x0000_0100` little-endian, TES4-and-later is `b"BSA\0"` --
//! never by file extension or caller hint. See [`container`] for the
//! [`sevenzip_re::container::Registry`] strategies that perform this probe.
//!
//! Redesigned from the reference implementation's known gaps:
//! - `tes4` codec dispatch is **version-aware**: v103/v104 payloads use
//!   zlib (`flate2`), v105 payloads use LZ4 (`lz4_flex`) — the reference
//!   crate used `ZlibDecoder` unconditionally regardless of version, which
//!   is simply wrong for v105 (Skyrim SE/AE) archives.
//!
//! Status: Wave 0. `tes4`'s read/write paths (version-aware zlib/LZ4,
//! spec-conforming hash-sorted pack) and `tes3`'s read/write paths
//! (uncompressed, hash-sorted-on-write per UESP) are both implemented and
//! unit/oracle-tested against synthetic fixtures and an independent oracle
//! crate. Real-world fixture gating (against an actual game BSA) is
//! tracked as follow-up work.

pub mod container;
pub mod error;
pub mod tes3;
pub mod tes4;

pub use error::{Error, Result};

// Re-exported for backward compatibility with existing callers (e.g.
// `modfather-vestibule::packing`) written before the tes3/tes4 split, all
// of which only ever meant the tes4 (v103-105) family -- `modfather-bsa`
// had no other family until this turn. New code should prefer the
// explicit `modfather_bsa::tes4::*` / `modfather_bsa::tes3::*` paths so it
// is unambiguous which BSA generation is in play.
pub use tes4::{write, BsaArchive, BsaEntry, FileToPack, WriteOptions};
