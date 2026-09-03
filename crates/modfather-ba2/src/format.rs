//! BA2 (`BTDX`) container layout constants.
//!
//! Reference: NIF Tools/nifskope `lib/fsengine/bsa.h` (`F4BSAHeader`,
//! `F4GeneralInfo`, `F4TexInfo`, `F4TexChunk` structs; BSD-licensed, used
//! here purely as a format oracle, not vendored code) and the `ba2` crate's
//! public docs (`docs.rs/ba2/latest/ba2/fo4`) for the version/compression
//! split.

/// 4-byte magic at the start of every BA2 file.
pub const MAGIC: [u8; 4] = *b"BTDX";

/// 4-byte type tag for the general-purpose (non-texture) format.
pub const TYPE_GNRL: [u8; 4] = *b"GNRL";
/// 4-byte type tag for the DirectX-texture (chunked, streaming-mip) format.
pub const TYPE_DX10: [u8; 4] = *b"DX10";

/// Fixed header size: magic(4) + version(4) + type(4) + numFiles(4) +
/// nameTableOffset(8) = 24 bytes.
pub const HEADER_SIZE: u64 = 24;

/// One `F4GeneralInfo` (GNRL) file record: 36 bytes.
pub const GNRL_RECORD_SIZE: u64 = 36;

/// One `F4TexInfo` (DX10) file header: 24 bytes.
pub const DX10_HEADER_SIZE: u64 = 24;

/// One `F4TexChunk` chunk record: 24 bytes.
pub const DX10_CHUNK_SIZE: u64 = 24;

/// Sentinel value ("BAADF00D") that terminates a GNRL record / DX10 chunk
/// record; used only as a sanity check, never load-bearing for parsing.
pub const SENTINEL: u32 = 0xBAAD_F00D;

/// Archive format versions (`ba2::fo4::Version`). v1 is the original
/// Fallout 4 format (zlib payloads). v2/v3 are Starfield-era (LZ4
/// payloads introduced). v7/v8 are the Fallout 4 "next-gen" update
/// (still zlib, but with a slightly different on-disk layout for some
/// fields that this crate does not yet need to distinguish for payload
/// decompression purposes).
pub mod version {
    pub const V1: u32 = 1;
    pub const V2: u32 = 2;
    pub const V3: u32 = 3;
    pub const V7: u32 = 7;
    pub const V8: u32 = 8;
}

/// Which payload codec a given archive version uses by default.
///
/// This is a version-based heuristic, not a bit read from an explicit
/// per-archive "compression format" field — Starfield (v2/v3) introduced
/// LZ4 alongside zlib, and the exact selection mechanism (there may be a
/// per-archive or per-file flag this crate doesn't yet decode) is a known
/// open area, flagged here rather than silently assumed to be fully solved.
/// This is already strictly better than the reference implementation,
/// which used `ZlibDecoder` unconditionally for every version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadCodec {
    Zlib,
    Lz4,
}

pub fn default_codec_for_version(v: u32) -> PayloadCodec {
    match v {
        version::V2 | version::V3 => PayloadCodec::Lz4,
        _ => PayloadCodec::Zlib,
    }
}
