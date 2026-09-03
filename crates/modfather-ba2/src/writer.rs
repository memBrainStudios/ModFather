//! BA2 (`BTDX`/GNRL) writer.
//!
//! Fixes the reference implementation's second known gap: its writer only
//! ever emitted uncompressed GNRL archives. This writer supports real
//! compression (zlib for v1/v7/v8, LZ4 for v2/v3 — matching the reader's
//! own version-aware codec choice) as well as an uncompressed mode.
//!
//! DX10 (texture-chunk) packing is out of scope for this writer: BA2
//! "Main" archives (which this writer targets, per `docs/VESTIBULE.md`'s
//! `{stem} - Main.ba2` naming) are GNRL; DX10 (`{stem} - Textures.ba2`) is
//! a separate, more involved packer (chunked streaming mips) tracked as
//! follow-up work.

use crate::error::{Error, Result};
use crate::format::{self, version, MAGIC, TYPE_GNRL};
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

/// Writer-level policy.
#[derive(Debug, Clone, Copy)]
pub struct WriteOptions {
    pub version: u32,
    /// When true, every payload is compressed with the version-appropriate
    /// codec (see [`format::default_codec_for_version`]) and its
    /// `packedSize` field reflects the compressed length. When false,
    /// `packedSize` is written as 0 (uncompressed, read verbatim), matching
    /// the reader's own handling of Archive2's "no compression" option.
    pub compress: bool,
}

impl Default for WriteOptions {
    fn default() -> Self {
        WriteOptions {
            version: version::V1,
            compress: true,
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
    let codec = format::default_codec_for_version(options.version);
    let num_files = files.len() as u32;

    // Normalize names up front; BA2's own on-disk order is not
    // hash-sorted the way BSA is (the `ba2` crate's own writer does not
    // sort by hash either — the game does not binary-search BA2 records
    // the way it does BSA folder/file records), so we keep insertion order.
    let normalized: Vec<String> = files
        .iter()
        .map(|f| f.name.replace('/', "\\").to_lowercase())
        .collect();

    // ---- Header (24 bytes) ----
    writer.write_all(&MAGIC)?;
    write_u32_le(&mut writer, options.version)?;
    writer.write_all(&TYPE_GNRL)?;
    write_u32_le(&mut writer, num_files)?;
    let name_table_offset_pos = current_pos(&mut writer)?;
    write_u64_le(&mut writer, 0)?; // nameTableOffset placeholder

    // ---- F4GeneralInfo records (36 bytes each), placeholders for
    // offset/packedSize/unpackedSize patched after payloads are written ----
    let mut offset_patch_positions = Vec::with_capacity(files.len());
    let mut packed_size_patch_positions = Vec::with_capacity(files.len());
    for name in &normalized {
        let h = hash::hash_path(name);
        write_u32_le(&mut writer, h.file)?;
        writer.write_all(&ext_field(name))?;
        write_u32_le(&mut writer, h.directory)?;
        write_u32_le(&mut writer, 0)?; // unk0C
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

fn current_pos<W: Seek>(w: &mut W) -> Result<u64> {
    Ok(w.stream_position()?)
}
