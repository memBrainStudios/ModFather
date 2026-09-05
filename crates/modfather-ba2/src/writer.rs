//! BA2 (`BTDX`/GNRL) writer.
//!
//! Fixes the reference implementation's second known gap: its writer only
//! ever emitted uncompressed GNRL archives. This writer supports real
//! compression (zlib for v1/v2/v7/v8; v3 defaults to zlib but can opt into
//! LZ4 via [`WriteOptions::force_lz4_v3`] -- see the header-layout note
//! below) as well as an uncompressed mode.
//!
//! **Starfield (v2/v3) header extension**: fixed to match the real
//! on-disk layout after oracle cross-validation surfaced that v2/v3 are
//! longer than the base 24-byte header this writer used to emit
//! unconditionally -- see `format::header_size_for_version` and its doc
//! comment for the full explanation and the two independent sources that
//! confirmed it (the `ba2` crate's own writer, and ByroRedux's format
//! notes). v2 gets 8 reserved zero bytes; v3 gets 8 reserved zero bytes
//! plus a real `compression_method: u32` field.
//!
//! DX10 (texture-chunk) packing: [`write_dx10`] below. Wave 0 scope --
//! emits exactly **one** chunk per texture, spanning the whole mip range
//! (`start_mip = 0`, `end_mip = num_mips - 1`). Real Archive2 output may
//! split a large texture's mips across up to 4 independently streamable
//! chunks (per NifTools' `F4TexInfo`/`F4TexChunk` notes and the `ba2`
//! crate's own `make_chunks`, which packs up to 4); multi-chunk streaming
//! packing is tracked as further follow-up work once it is actually
//! needed, since a single full-range chunk is a structurally valid DX10
//! file that this crate's own [`crate::reader::Ba2Archive::read_chunk`]
//! round-trips correctly (chunk count is a real per-file field, not
//! assumed to be 1 by the reader).
//!
//! **Scope boundary, deliberately not crossed here:** this writer takes
//! texture metadata (`height`/`width`/`num_mips`/`format`) as caller-
//! supplied fields on [`TextureToPack`], not by parsing a raw `.dds`
//! file itself. Real DDS files encode that metadata in a 128-byte (or
//! 148-byte, when a `DX10` FourCC extension header is present) header
//! that this crate deliberately does not parse: DDS is a texture *file*
//! format (Microsoft's public spec), not part of BA2's own container
//! layout, which is this crate's sole concern per its module doc comment
//! (`docs/CRUCIBLE.md` already carves out a dedicated "DDS view/convert/
//! mip job" as the intended home for that parsing). Real v3 archives are
//! DX10 by convention, but [`write`] (GNRL) still emits a structurally-
//! correct v3 *GNRL* header extension if a caller passes `version: 3`,
//! since nothing in the format forbids it and it keeps the reader/writer
//! symmetric for testing.

use crate::error::{Error, Result};
use crate::format::{self, version, MAGIC, TYPE_DX10, TYPE_GNRL};
use crate::hash;
use std::io::{Seek, SeekFrom, Write};

/// One file to pack into a GNRL (general-purpose) BA2 archive.
/// `name` is the archive-relative path using either slash direction (e.g.
/// `Interface\HUDMenu.swf` or `Interface/HUDMenu.swf`); it is normalized
/// to backslash + lowercase for hashing and to the on-disk name table.
#[derive(Debug, Clone)]
pub struct FileToPack {
    pub name: String,
    pub data: Vec<u8>,
}

/// One texture to pack into a DX10 (`TYPE_DX10`) BA2 archive.
/// `name` follows the same normalization rule as [`FileToPack::name`].
/// `data` is the already-decoded texture payload -- the raw mip bytes
/// that belong in the single full-mip-range chunk this writer emits (see
/// the module doc comment's Wave-0 single-chunk scope note), **not** a
/// raw `.dds` file with its own header still attached; `height`/`width`/
/// `num_mips`/`format` are the caller-supplied `F4TexInfo` fields this
/// writer cannot derive on its own without parsing DDS (out of scope
/// here -- see the module doc comment).
#[derive(Debug, Clone)]
pub struct TextureToPack {
    pub name: String,
    pub data: Vec<u8>,
    pub height: u16,
    pub width: u16,
    pub num_mips: u8,
    /// `DXGI_FORMAT` value (e.g. 71 = `BC1_UNORM`, 77 = `BC3_UNORM`, 98 =
    /// `BC7_UNORM`); stored verbatim in `F4TexInfo::format`, never
    /// interpreted by this crate.
    pub format: u8,
}

/// Writer-level policy.
#[derive(Debug, Clone, Copy)]
pub struct WriteOptions {
    pub version: u32,
    /// When true, every payload is compressed and its `packedSize` field
    /// reflects the compressed length. When false, `packedSize` is
    /// written as 0 (uncompressed, read verbatim), matching the reader's
    /// own handling of Archive2's "no compression" option.
    pub compress: bool,
    /// Only meaningful when `version == 3`: selects LZ4 block compression
    /// (writing `compression_method = 3` in the v3 header extension)
    /// instead of the default zlib (`compression_method = 0`). Ignored
    /// for every other version, which are always zlib -- see the
    /// module-level doc comment and `format::codec_for_compression_method`.
    pub force_lz4_v3: bool,
}

impl Default for WriteOptions {
    fn default() -> Self {
        WriteOptions {
            version: version::V1,
            compress: true,
            force_lz4_v3: false,
        }
    }
}

fn write_u32_le<W: Write>(w: &mut W, v: u32) -> Result<()> {
    w.write_all(&v.to_le_bytes())?;
    Ok(())
}
fn write_u64_le<W: Write>(w: &mut W, v: u64) -> Result<()> {
    w.write_all(&v.to_le_bytes())?;
    Ok(())
}
fn write_u16_le<W: Write>(w: &mut W, v: u16) -> Result<()> {
    w.write_all(&v.to_le_bytes())?;
    Ok(())
}

/// v3's on-disk `compression_method` field value for a given codec choice
/// (0 = zlib, 3 = LZ4 block; see `format::codec_for_compression_method`
/// for the read-side counterpart).
fn compression_method_field(codec: format::PayloadCodec) -> u32 {
    match codec {
        format::PayloadCodec::Lz4 => 3,
        format::PayloadCodec::Zlib => 0,
    }
}

fn compress_payload(codec: format::PayloadCodec, data: &[u8]) -> Result<Vec<u8>> {
    match codec {
        format::PayloadCodec::Lz4 => Ok(lz4_flex::block::compress(data)),
        format::PayloadCodec::Zlib => {
            use flate2::write::ZlibEncoder;
            let mut enc = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            enc.write_all(data)
                .map_err(|e| Error::Malformed(format!("zlib compress error: {e}")))?;
            enc.finish()
                .map_err(|e| Error::Malformed(format!("zlib compress error: {e}")))
        }
    }
}

/// Extension (first 4 bytes, per `F4GeneralInfo::ext`) extracted from a
/// (already-normalized, lowercase) file name.
fn ext_field(normalized_name: &str) -> [u8; 4] {
    let ext = normalized_name
        .rsplit('\\')
        .next()
        .unwrap_or(normalized_name)
        .rsplit_once('.')
        .map(|(_, e)| e)
        .unwrap_or("");
    let mut out = [0u8; 4];
    for (i, b) in ext.as_bytes().iter().take(4).enumerate() {
        out[i] = *b;
    }
    out
}

/// Pack `files` into a GNRL BA2 archive and write it to `writer`.
pub fn write<W: Write + Seek>(
    mut writer: W,
    files: &[FileToPack],
    options: &WriteOptions,
) -> Result<()> {
    let codec = if options.version == version::V3 {
        if options.force_lz4_v3 {
            format::PayloadCodec::Lz4
        } else {
            format::PayloadCodec::Zlib
        }
    } else {
        format::default_codec_for_version(options.version)
    };
    let num_files = files.len() as u32;

    // Normalize names up front; BA2's own on-disk order is not
    // hash-sorted the way BSA is (the `ba2` crate's own writer does not
    // sort by hash either — the game does not binary-search BA2 records
    // the way it does BSA folder/file records), so we keep insertion order.
    let normalized: Vec<String> = files
        .iter()
        .map(|f| f.name.replace('/', "\\").to_lowercase())
        .collect();

    // ---- Header (24 bytes, plus a version-dependent extension) ----
    writer.write_all(&MAGIC)?;
    write_u32_le(&mut writer, options.version)?;
    writer.write_all(&TYPE_GNRL)?;
    write_u32_le(&mut writer, num_files)?;
    let name_table_offset_pos = current_pos(&mut writer)?;
    write_u64_le(&mut writer, 0)?; // nameTableOffset placeholder

    // Starfield (v2/v3) header extension -- see the module-level doc
    // comment and `format::header_size_for_version` for why this exists.
    if options.version == version::V2 {
        write_u64_le(&mut writer, 0)?; // 8 reserved bytes, no known meaning
    } else if options.version == version::V3 {
        write_u64_le(&mut writer, 0)?; // 8 reserved bytes
        write_u32_le(&mut writer, compression_method_field(codec))?;
    }

    // ---- F4GeneralInfo records (36 bytes each), placeholders for
    // offset/packedSize/unpackedSize patched after payloads are written ----
    let mut offset_patch_positions = Vec::with_capacity(files.len());
    let mut packed_size_patch_positions = Vec::with_capacity(files.len());
    for name in &normalized {
        let h = hash::hash_path(name);
        write_u32_le(&mut writer, h.file)?;
        writer.write_all(&ext_field(name))?;
        write_u32_le(&mut writer, h.directory)?;
        // unk0C(u8)=0, numChunks(u8), chunkHeaderSize(u16): real readers
        // (confirmed against the independent `ba2` crate oracle) validate
        // numChunks/chunkHeaderSize for GNRL as (1, 0x10) -- see
        // `format::GNRL_NUM_CHUNKS`/`format::GNRL_CHUNK_HEADER_SIZE`.
        writer.write_all(&[0u8, format::GNRL_NUM_CHUNKS])?;
        write_u16_le(&mut writer, format::GNRL_CHUNK_HEADER_SIZE)?;
        offset_patch_positions.push(current_pos(&mut writer)?);
        write_u64_le(&mut writer, 0)?; // offset placeholder
        packed_size_patch_positions.push(current_pos(&mut writer)?);
        write_u32_le(&mut writer, 0)?; // packedSize placeholder
        write_u32_le(&mut writer, 0)?; // unpackedSize placeholder, patched below too
        write_u32_le(&mut writer, format::SENTINEL)?;
    }

    // ---- Payloads ----
    let mut offsets = Vec::with_capacity(files.len());
    let mut packed_sizes = Vec::with_capacity(files.len());
    let mut unpacked_sizes = Vec::with_capacity(files.len());
    for file in files {
        let data_offset = current_pos(&mut writer)?;
        offsets.push(data_offset);
        unpacked_sizes.push(file.data.len() as u32);

        if options.compress {
            let packed = compress_payload(codec, &file.data)?;
            packed_sizes.push(packed.len() as u32);
            writer.write_all(&packed)?;
        } else {
            packed_sizes.push(0u32); // packedSize == 0 means uncompressed
            writer.write_all(&file.data)?;
        }
    }

    // ---- Name table ----
    let name_table_offset = current_pos(&mut writer)?;
    for name in &normalized {
        if name.len() > u16::MAX as usize {
            return Err(Error::Malformed(format!(
                "file name too long for BA2 name table: {name:?}"
            )));
        }
        write_u16_le(&mut writer, name.len() as u16)?;
        writer.write_all(name.as_bytes())?;
    }

    // ---- Patch pass ----
    writer.seek(SeekFrom::Start(name_table_offset_pos))?;
    write_u64_le(&mut writer, name_table_offset)?;

    for i in 0..files.len() {
        writer.seek(SeekFrom::Start(offset_patch_positions[i]))?;
        write_u64_le(&mut writer, offsets[i])?;
        writer.seek(SeekFrom::Start(packed_size_patch_positions[i]))?;
        write_u32_le(&mut writer, packed_sizes[i])?;
        write_u32_le(&mut writer, unpacked_sizes[i])?;
    }

    Ok(())
}

/// Pack `textures` into a DX10 BA2 archive and write it to `writer`.
/// Produces exactly the `F4TexInfo`(24 bytes) + one `F4TexChunk`(24
/// bytes) per-file layout `reader::Ba2Archive::read_dx10_entries` already
/// parses -- see the module doc comment's Wave-0 single-chunk scope note
/// for why this writer never emits more than one chunk per texture.
pub fn write_dx10<W: Write + Seek>(
    mut writer: W,
    textures: &[TextureToPack],
    options: &WriteOptions,
) -> Result<()> {
    let codec = if options.version == version::V3 {
        if options.force_lz4_v3 {
            format::PayloadCodec::Lz4
        } else {
            format::PayloadCodec::Zlib
        }
    } else {
        format::default_codec_for_version(options.version)
    };
    let num_files = textures.len() as u32;

    let normalized: Vec<String> = textures
        .iter()
        .map(|f| f.name.replace('/', "\\").to_lowercase())
        .collect();

    // ---- Header (24 bytes, plus a version-dependent extension) ----
    writer.write_all(&MAGIC)?;
    write_u32_le(&mut writer, options.version)?;
    writer.write_all(&TYPE_DX10)?;
    write_u32_le(&mut writer, num_files)?;
    let name_table_offset_pos = current_pos(&mut writer)?;
    write_u64_le(&mut writer, 0)?; // nameTableOffset placeholder

    // Starfield (v2/v3) header extension -- same rationale as `write`
    // (GNRL) above; see the module-level doc comment on that function
    // and `format::header_size_for_version`.
    if options.version == version::V2 {
        write_u64_le(&mut writer, 0)?; // 8 reserved bytes, no known meaning
    } else if options.version == version::V3 {
        write_u64_le(&mut writer, 0)?; // 8 reserved bytes
        write_u32_le(&mut writer, compression_method_field(codec))?;
    }

    // ---- F4TexInfo records (24 bytes each) + one F4TexChunk (24 bytes)
    // per file, placeholders for the chunk's offset/packedSize/
    // unpackedSize patched after payloads are written ----
    let mut offset_patch_positions = Vec::with_capacity(textures.len());
    let mut packed_size_patch_positions = Vec::with_capacity(textures.len());
    for (name, tex) in normalized.iter().zip(textures) {
        let h = hash::hash_path(name);
        write_u32_le(&mut writer, h.file)?;
        writer.write_all(&ext_field(name))?;
        write_u32_le(&mut writer, h.directory)?;
        // unk0C(u8)=0, numChunks(u8)=1 (see the module doc comment's
        // single-chunk scope note), chunkHeaderSize(u16)=0x18 (24 bytes,
        // matching `format::DX10_CHUNK_SIZE`; confirmed against the same
        // NifTools `F4TexChunk` reference `format.rs` cites).
        writer.write_all(&[0u8, 1u8])?;
        write_u16_le(&mut writer, format::DX10_CHUNK_SIZE as u16)?;
        write_u16_le(&mut writer, tex.height)?;
        write_u16_le(&mut writer, tex.width)?;
        writer.write_all(&[tex.num_mips, tex.format])?;
        write_u16_le(&mut writer, 0)?; // unk16, no known meaning

        // ---- F4TexChunk (24 bytes): the single full-mip-range chunk ----
        offset_patch_positions.push(current_pos(&mut writer)?);
        write_u64_le(&mut writer, 0)?; // offset placeholder
        packed_size_patch_positions.push(current_pos(&mut writer)?);
        write_u32_le(&mut writer, 0)?; // packedSize placeholder
        write_u32_le(&mut writer, 0)?; // unpackedSize placeholder, patched below too
        let end_mip = tex.num_mips.saturating_sub(1) as u16;
        write_u16_le(&mut writer, 0)?; // startMip
        write_u16_le(&mut writer, end_mip)?; // endMip
        write_u32_le(&mut writer, format::SENTINEL)?;
    }

    // ---- Payloads (one per texture, per its single chunk) ----
    let mut offsets = Vec::with_capacity(textures.len());
    let mut packed_sizes = Vec::with_capacity(textures.len());
    let mut unpacked_sizes = Vec::with_capacity(textures.len());
    for tex in textures {
        let data_offset = current_pos(&mut writer)?;
        offsets.push(data_offset);
        unpacked_sizes.push(tex.data.len() as u32);

        if options.compress {
            let packed = compress_payload(codec, &tex.data)?;
            packed_sizes.push(packed.len() as u32);
            writer.write_all(&packed)?;
        } else {
            packed_sizes.push(0u32); // packedSize == 0 means uncompressed
            writer.write_all(&tex.data)?;
        }
    }

    // ---- Name table ----
    let name_table_offset = current_pos(&mut writer)?;
    for name in &normalized {
        if name.len() > u16::MAX as usize {
            return Err(Error::Malformed(format!(
                "file name too long for BA2 name table: {name:?}"
            )));
        }
        write_u16_le(&mut writer, name.len() as u16)?;
        writer.write_all(name.as_bytes())?;
    }

    // ---- Patch pass ----
    writer.seek(SeekFrom::Start(name_table_offset_pos))?;
    write_u64_le(&mut writer, name_table_offset)?;

    for i in 0..textures.len() {
        writer.seek(SeekFrom::Start(offset_patch_positions[i]))?;
        write_u64_le(&mut writer, offsets[i])?;
        writer.seek(SeekFrom::Start(packed_size_patch_positions[i]))?;
        write_u32_le(&mut writer, packed_sizes[i])?;
        write_u32_le(&mut writer, unpacked_sizes[i])?;
    }

    Ok(())
}

fn current_pos<W: Seek>(w: &mut W) -> Result<u64> {
    Ok(w.stream_position()?)
}
