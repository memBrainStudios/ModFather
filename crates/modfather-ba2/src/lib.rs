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
//! Status: Wave 0. Read path (GNRL + DX10, both codecs) and the GNRL write
//! path (with real compression, fixing the reference's uncompressed-only
//! writer) are implemented and unit-tested against synthetic fixtures,
//! including a full write-then-read round trip. A DX10 (texture-chunk)
//! writer and gating against real-world BA2 fixtures are tracked as
//! follow-up work (see `docs/SCHEDULE.md`, Wave 0).

pub mod error;
pub mod format;
pub mod hash;
pub mod reader;
pub mod writer;

pub use error::{Error, Result};
pub use reader::{Ba2Archive, Ba2Entry, EntryKind, TexChunk};
pub use writer::{write, FileToPack, WriteOptions};
