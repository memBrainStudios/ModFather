//! `modfather-ba2` — Bethesda BA2 (GNRL/DX10) archive extension.
//!
//! **Separate deliverable** from the `sevenzip-re` standalone package, same
//! rationale as `modfather-bsa`: BA2 is Bethesda's own container format,
//! not 7z, and must never be folded into the standalone engine.
//!
//! Redesigned from the reference implementation's known gaps:
//! - Codec dispatch is **version-aware**: the reference used
//!   `flate2::ZlibDecoder` unconditionally for every payload, which is
//!   wrong for Starfield-era (v2/v3) archives that use LZ4. This crate
//!   picks zlib vs. LZ4 from the archive version (see
//!   [`format::default_codec_for_version`]), and also honors
//!   `packedSize == 0` (Archive2's "no compression" option), which the
//!   reference reader did not handle explicitly.
//! - DX10 (texture) entries are modeled as a header plus a list of
//!   independently-decodable mip [`reader::TexChunk`]s, matching the
//!   real streaming-mip design instead of treating a texture as one blob.
//!
//! Status: Wave 0. Read path (GNRL + DX10, both codecs) and both write
//! paths (GNRL via [`write`], DX10 via [`write_dx10`]; both with real
//! compression, fixing the reference's uncompressed-only writer) are
//! implemented and unit-tested against synthetic fixtures, including a
//! full write-then-read round trip, plus cross-validation against an
//! independent oracle implementation (`ba2` crate, dev-only dependency;
//! see `tests/oracle_cross_validation.rs`) for v1/v2/v3 GNRL. That
//! cross-validation surfaced and fixed a real structural bug: v2/v3
//! (Starfield) archives have a header extension beyond the base 24 bytes,
//! and v3's codec is a genuine per-archive `compression_method` field
//! (not implied by version number) -- see `format::header_size_for_version`
//! for the full writeup. [`write_dx10`] is Wave-0 scope: exactly one
//! chunk per texture spanning the full mip range, rather than the up-to-4
//! independently-streamable chunks real Archive2 output may use -- see
//! that function's module-level doc comment for why this is still a
//! structurally valid DX10 archive and what multi-chunk streaming would
//! add on top.

pub mod container;
pub mod error;
pub mod format;
pub mod hash;
pub mod reader;
pub mod writer;

pub use error::{Error, Result};
pub use reader::{Ba2Archive, Ba2Entry, EntryKind, TexChunk};
pub use writer::{write, write_dx10, FileToPack, TextureToPack, WriteOptions};
