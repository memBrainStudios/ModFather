//! BA2 (`BTDX`) container layout constants.
//!
//! Reference: NIF Tools/nifskope `lib/fsengine/bsa.h` (`F4BSAHeader`,
//! `F4GeneralInfo`, `F4TexInfo`, `F4TexChunk` structs; BSD-licensed, used
//! here purely as a format oracle, not vendored code) and the `ba2` crate's
//! public docs (`docs.rs/ba2/latest/ba2/fo4`) for the version/compression
//! split.
//!
//! **Starfield (v2/v3) header extension** (fixed after oracle
//! cross-validation surfaced it -- see `tests/oracle_cross_validation.rs`
//! and `reader.rs`/`writer.rs`): the base 24-byte header above is only
//! correct for v1/v7/v8. Real v2 and v3 archives are longer:
//! - v2: 24 + 8 = 32 bytes (`HEADER_SIZE_V2` below). The extra 8 bytes are
//!   two `u32`s the `ba2` crate's own source treats as unread/reserved on
//!   the read path (it still consumes them, it just discards the values).
//! - v3: 24 + 8 + 4 = 36 bytes (`HEADER_SIZE_V3` below). The extra 4 bytes
//!   beyond v2 are a `compression_method: u32` (0 = zlib, 3 = LZ4 block)
//!   that is a genuine **per-archive** field read from disk, not a
//!   version-based guess.
//!
//! This was confirmed against two independent sources: the `ba2` crate's
//! own `fo4::archive` module (`constants::HEADER_SIZE_V2`/`HEADER_SIZE_V3`,
//! and `read_header`'s explicit `if matches!(version, Version::v2 |
//! Version::v3) { source.read::<u64>() }` / `if version == Version::v3 {
//! let format: u32 = source.read(...); }`), and the ByroRedux project's
//! published archive-format notes, which independently document the same
//! byte counts and explicitly call out this exact bug class ("This bit me
//! during M26: gating the 8-byte extension on `version >= 2` broke FO4
//! v8... The v3 compression method was discovered in session 7 -- the
//! real issue was a missing 4-byte field shifting the reader past the
//! header, plus zlib being used for LZ4-compressed chunks").
//!
//! Before this fix, `modfather-ba2` read/wrote only the base 24-byte
//! header for every version, which would misalign every field after the
//! header for any real v2/v3 (Starfield) archive, and picked the payload
//! codec from a static version-only table instead of the per-archive
//! `compression_method` field -- silently wrong for a v3 archive that
//! chose zlib (method 0) instead of LZ4.

/// 4-byte magic at the start of every BA2 file.
pub const MAGIC: [u8; 4] = *b"BTDX";

/// 4-byte type tag for the general-purpose (non-texture) format.
pub const TYPE_GNRL: [u8; 4] = *b"GNRL";
/// 4-byte type tag for the DirectX-texture (chunked, streaming-mip) format.
pub const TYPE_DX10: [u8; 4] = *b"DX10";

/// Fixed base header size: magic(4) + version(4) + type(4) + numFiles(4) +
/// nameTableOffset(8) = 24 bytes. Correct for v1/v7/v8. v2/v3 (Starfield)
/// archives are longer -- see [`HEADER_SIZE_V2`], [`HEADER_SIZE_V3`], and
/// the module-level doc comment above.
pub const HEADER_SIZE: u64 = 24;

/// v2 (Starfield GNRL) header size: base 24 bytes + 8 bytes (two `u32`s,
/// unread/reserved on the read path per the `ba2` crate's own source).
pub const HEADER_SIZE_V2: u64 = 32;

/// v3 (Starfield DX10) header size: base 24 bytes + 8 (as v2) + 4 bytes
/// (`compression_method: u32`; 0 = zlib, 3 = LZ4 block).
pub const HEADER_SIZE_V3: u64 = 36;

/// Header size in bytes for a given archive version, per the byte counts
/// documented above.
pub fn header_size_for_version(v: u32) -> u64 {
    match v {
        version::V2 => HEADER_SIZE_V2,
        version::V3 => HEADER_SIZE_V3,
        _ => HEADER_SIZE,
    }
}

/// One `F4GeneralInfo` (GNRL) file record: 36 bytes.
pub const GNRL_RECORD_SIZE: u64 = 36;

/// The `chunk_size` sub-field of a GNRL file record's 4-byte
/// `unk0C`/`numChunks`/`chunkHeaderSize` group (bytes 12-15 of the
/// record): real BA2 readers (confirmed against the `ba2` crate's
/// `constants::FILE_HEADER_SIZE_GNRL`) validate this as the size, in
/// bytes, of the per-chunk header that follows -- 0x10 for GNRL. A third
/// bug, found via `tests/oracle_cross_validation.rs`: this crate's writer
/// used to write all 4 bytes as a blind zero `u32`, which our own lenient
/// reader tolerated but the independent oracle's reader correctly
/// rejected ("invalid chunk size read from file header: 0"), since GNRL
/// files in the real format always declare exactly one chunk of this
/// size.
pub const GNRL_CHUNK_HEADER_SIZE: u16 = 0x10;

/// The `numChunks` sub-field of the same 4-byte group: GNRL files always
/// have exactly one (non-streamed) chunk.
pub const GNRL_NUM_CHUNKS: u8 = 1;

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

/// Which payload codec an archive uses.
///
/// Fixed after oracle cross-validation (see the module-level doc comment):
/// this is **not** purely a function of version. v1/v2/v7/v8 are always
/// zlib. v3 carries an explicit per-archive `compression_method: u32`
/// field in its header extension (0 = zlib, 3 = LZ4 block) -- so a v3
/// archive can legitimately be either codec, and the codec must be read
/// from that field, not guessed from the version number alone.
///
/// An earlier revision of this enum/function pair guessed LZ4 for *both*
/// v2 and v3 purely from the version number. That was wrong on two
/// counts, confirmed against the `ba2` crate's own `read_header` (only
/// `Version::v3` reads a `compression_format` field; v2 has no such field
/// and is unconditionally zlib) and the ByroRedux project's format table
/// ("Starfield | BA2 BTDX v2 GNRL / v3 DX10 | zlib + LZ4 block"): v2 is
/// always zlib, and v3's codec is a real per-archive field, not implied
/// by the version number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadCodec {
    Zlib,
    Lz4,
}

/// Codec implied by a v3 archive's `compression_method` field (0 = zlib,
/// 3 = LZ4 block; any other value is treated as zlib, matching the `ba2`
/// crate's own fallback behavior in `read_header`).
pub fn codec_for_compression_method(method: u32) -> PayloadCodec {
    match method {
        3 => PayloadCodec::Lz4,
        _ => PayloadCodec::Zlib,
    }
}

/// Fallback-only guess when no explicit per-archive codec field is
/// available (i.e. every version except v3). v1/v2/v7/v8 are always zlib;
/// callers must not use this for v3 -- see [`codec_for_compression_method`].
pub fn default_codec_for_version(_v: u32) -> PayloadCodec {
    PayloadCodec::Zlib
}
