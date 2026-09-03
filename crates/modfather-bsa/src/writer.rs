//! BSA v103-105 writer: packs a set of in-memory files into a spec-
//! conforming archive, per the UESP "Skyrim Mod:Archive File Format" layout
//! (`docs/SCHEDULE.md`'s Wave 0 gate: "pack a stem back to `{stem}.bsa` /
//! `{stem} - Textures.bsa`" — this module is the mechanics; the naming
//! split itself lives in `modfather-vestibule`'s packing orchestration).

use crate::error::{Error, Result};
use crate::format::{archive_flags, MAGIC};
use crate::hash;
use std::io::{Seek, SeekFrom, Write};

/// One file to pack, addressed by its VFS-relative folder and file name.
/// `folder` uses either slash separator (normalized internally); `name` is
/// the bare file name with extension.
#[derive(Debug, Clone)]
pub struct FileToPack {
    pub folder: String,
    pub name: String,
    pub data: Vec<u8>,
}

/// Writer-level policy. `version` selects both the on-disk layout
/// (v105 adds per-folder padding fields) and, when `compress` is set, the
/// payload codec (v105 -> LZ4, v103/v104 -> zlib) — mirroring the reader's
/// version-aware dispatch so a round-trip through this crate always agrees
/// with itself, and matching the real game's own version/codec pairing.
#[derive(Debug, Clone, Copy)]
pub struct WriteOptions {
    pub version: u32,
    /// Sets the archive's `COMPRESSED_ARCHIVE` default and compresses every
    /// file's payload with the version-appropriate codec. Per-file
    /// compression-inversion (the size field's 0x4000_0000 bit) is not
    /// produced by this writer — every file follows the archive default.
    pub compress: bool,
}

impl Default for WriteOptions {
    fn default() -> Self {
        WriteOptions {
            version: 105,
            compress: true,
        }
    }
}

fn write_u16_le<W: Write>(w: &mut W, v: u16) -> Result<()> {
    w.write_all(&v.to_le_bytes())?;
    Ok(())
}
fn write_u32_le<W: Write>(w: &mut W, v: u32) -> Result<()> {
    w.write_all(&v.to_le_bytes())?;
    Ok(())
}
fn write_u64_le<W: Write>(w: &mut W, v: u64) -> Result<()> {
    w.write_all(&v.to_le_bytes())?;
    Ok(())
}

/// bzstring: 1-byte length (name bytes + 1 for the trailing NUL), the name
/// bytes, then the NUL.
fn write_bzstring<W: Write>(w: &mut W, s: &str) -> Result<()> {
    let len = s.len() + 1;
    if len > 255 {
        return Err(Error::Malformed(format!(
            "folder name too long for a bzstring: {s:?}"
        )));
    }
    w.write_all(&[len as u8])?;
    w.write_all(s.as_bytes())?;
    w.write_all(&[0u8])?;
    Ok(())
}

fn write_cstr<W: Write>(w: &mut W, s: &str) -> Result<()> {
    w.write_all(s.as_bytes())?;
    w.write_all(&[0u8])?;
    Ok(())
}

fn normalize_folder(folder: &str) -> String {
    folder.replace('/', "\\").to_lowercase()
}

fn normalize_name(name: &str) -> String {
    name.to_lowercase()
}

/// v105 (Skyrim SE/AE) uses the **LZ4 frame format** (confirmed against an
/// independent oracle -- see the matching note on `reader::decode_lz4` --
/// not raw LZ4 blocks). Writing raw blocks here would produce archives the
/// real game (and any other LZ4-frame-expecting reader, including this
/// crate's own reader before this fix) cannot decompress.
fn compress_payload(version: u32, data: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(data.len() + 4);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    if version >= 105 {
        use lz4_flex::frame::FrameEncoder;
        let mut enc = FrameEncoder::new(Vec::new());
        enc.write_all(data)
            .map_err(|e| Error::Lz4(e.to_string()))?;
        let body = enc.finish().map_err(|e| Error::Lz4(e.to_string()))?;
        out.extend_from_slice(&body);
    } else {
        use flate2::write::ZlibEncoder;
        let mut enc = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(data)
            .map_err(|e| Error::Zlib(e.to_string()))?;
        let body = enc.finish().map_err(|e| Error::Zlib(e.to_string()))?;
        out.extend_from_slice(&body);
    }
    Ok(out)
}

/// One hash-sorted file within a folder: (normalized name, file hash, data).
type SortedFileEntry<'a> = (String, u64, &'a [u8]);
/// One hash-sorted folder: (normalized folder name, folder hash, its files).
type SortedFolderEntry<'a> = (String, u64, Vec<SortedFileEntry<'a>>);

/// Pack `files` into a BSA archive and write it to `writer`.
///
/// Folders and files are sorted by their BSA name-hash, matching the real
/// game's expected on-disk ordering ("Files and Dirs order" in the UESP
/// spec) even though this crate's own reader does not require that order.
pub fn write<W: Write + Seek>(
    mut writer: W,
    files: &[FileToPack],
    options: &WriteOptions,
) -> Result<()> {
    if !(103..=105).contains(&options.version) {
        return Err(Error::UnsupportedVersion(options.version));
    }

    // Group by normalized folder, preserving a stable per-folder file order
    // before the final hash-sort.
    use std::collections::BTreeMap;
    let mut by_folder: BTreeMap<String, Vec<(String, &[u8])>> = BTreeMap::new();
    for f in files {
        let folder = normalize_folder(&f.folder);
        let name = normalize_name(&f.name);
        by_folder
            .entry(folder)
            .or_default()
            .push((name, f.data.as_slice()));
    }

    // Sort folders by hash, and files within each folder by hash, per spec.
    let mut folders: Vec<SortedFolderEntry> = by_folder
        .into_iter()
        .map(|(folder, mut entries)| {
            entries.sort_by_key(|(name, _)| hash::hash_file(name));
            let folder_hash = hash::hash_folder(&folder);
            let entries = entries
                .into_iter()
                .map(|(name, data)| {
                    let h = hash::hash_file(&name);
                    (name, h, data)
                })
                .collect();
            (folder, folder_hash, entries)
        })
        .collect();
    folders.sort_by_key(|(_, h, _)| *h);

    let folder_count = folders.len() as u32;
    let file_count: u32 = folders.iter().map(|(_, _, e)| e.len() as u32).sum();
    let total_folder_name_len: u32 = folders.iter().map(|(f, _, _)| (f.len() + 1) as u32).sum();
    let total_file_name_len: u32 = folders
        .iter()
        .flat_map(|(_, _, e)| e.iter())
        .map(|(name, _, _)| (name.len() + 1) as u32)
        .sum();

    let archive_flags = archive_flags::INCLUDE_DIR_NAMES
        | archive_flags::INCLUDE_FILE_NAMES
        | if options.compress {
            archive_flags::COMPRESSED_ARCHIVE
        } else {
            0
        };

    // ---- Header (36 bytes) ----
    writer.write_all(&MAGIC)?;
    write_u32_le(&mut writer, options.version)?;
    write_u32_le(&mut writer, 36)?; // offset to folder records
    write_u32_le(&mut writer, archive_flags)?;
    write_u32_le(&mut writer, folder_count)?;
    write_u32_le(&mut writer, file_count)?;
    write_u32_le(&mut writer, total_folder_name_len)?;
    write_u32_le(&mut writer, total_file_name_len)?;
    write_u16_le(&mut writer, 0)?; // fileFlags
    write_u16_le(&mut writer, 0)?; // padding

    // ---- Folder records ----
    // Each folder record's `offset` field is patched in a second pass once
    // we know where its file-record block starts; per the UESP spec note,
    // the stored value is (actual byte offset) + totalFileNameLength.
    let folder_record_size: u64 = if options.version >= 105 { 24 } else { 16 };
    let folder_records_start = 36u64;
    let mut folder_offset_patch_positions = Vec::with_capacity(folders.len());

    for (_, folder_hash, entries) in &folders {
        write_u64_le(&mut writer, *folder_hash)?;
        write_u32_le(&mut writer, entries.len() as u32)?;
        if options.version >= 105 {
            write_u32_le(&mut writer, 0)?; // pad
        }
        let patch_pos = folder_records_start
            + folder_offset_patch_positions.len() as u64 * folder_record_size
            + if options.version >= 105 { 16 } else { 12 };
        folder_offset_patch_positions.push(patch_pos);
        write_u32_le(&mut writer, 0)?; // offset placeholder
        if options.version >= 105 {
            write_u32_le(&mut writer, 0)?; // pad
        }
    }

    // ---- File-record blocks ----
    // size/offset fields are placeholders, patched in a final pass once
    // file data offsets are known.
    let mut size_patch_positions: Vec<u64> = Vec::with_capacity(file_count as usize);
    let mut offset_patch_positions: Vec<u64> = Vec::with_capacity(file_count as usize);
    let mut folder_block_starts: Vec<u64> = Vec::with_capacity(folders.len());

    for (folder_name, _, entries) in &folders {
        folder_block_starts.push(current_pos(&mut writer)?);
        write_bzstring(&mut writer, folder_name)?;
        for (_, file_hash, _) in entries {
            write_u64_le(&mut writer, *file_hash)?;
            size_patch_positions.push(current_pos(&mut writer)?);
            write_u32_le(&mut writer, 0)?; // size placeholder
            offset_patch_positions.push(current_pos(&mut writer)?);
            write_u32_le(&mut writer, 0)?; // offset placeholder
        }
    }

    // ---- File name block ----
    for (_, _, entries) in &folders {
        for (name, _, _) in entries {
            write_cstr(&mut writer, name)?;
        }
    }

    // ---- File data ----
    let mut sizes: Vec<u32> = Vec::with_capacity(file_count as usize);
    let mut offsets: Vec<u32> = Vec::with_capacity(file_count as usize);
    for (_, _, entries) in &folders {
        for (_, _, data) in entries {
            let data_offset = current_pos(&mut writer)?;
            if data_offset > u32::MAX as u64 {
                return Err(Error::Malformed(
                    "archive exceeds 4GiB offset range for v103-105 BSA".into(),
                ));
            }
            offsets.push(data_offset as u32);

            if options.compress {
                let packed = compress_payload(options.version, data)?;
                let size = packed.len() as u32;
                sizes.push(size);
                writer.write_all(&packed)?;
            } else {
                sizes.push(data.len() as u32);
                writer.write_all(data)?;
            }
        }
    }

    // ---- Patch pass: folder offsets, file sizes/offsets ----
    for (patch_pos, block_start) in folder_offset_patch_positions
        .iter()
        .zip(folder_block_starts.iter())
    {
        let stored = *block_start + total_file_name_len as u64;
        writer.seek(SeekFrom::Start(*patch_pos))?;
        write_u32_le(&mut writer, stored as u32)?;
    }
    for (patch_pos, size) in size_patch_positions.iter().zip(sizes.iter()) {
        writer.seek(SeekFrom::Start(*patch_pos))?;
        write_u32_le(&mut writer, *size)?;
    }
    for (patch_pos, offset) in offset_patch_positions.iter().zip(offsets.iter()) {
        writer.seek(SeekFrom::Start(*patch_pos))?;
        write_u32_le(&mut writer, *offset)?;
    }

    Ok(())
}

fn current_pos<W: Seek>(w: &mut W) -> Result<u64> {
    Ok(w.stream_position()?)
}
