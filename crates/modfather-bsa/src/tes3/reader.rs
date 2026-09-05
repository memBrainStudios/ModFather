//! Morrowind (TES III) BSA reader.
//!
//! Layout (see [`crate::tes3::format`] for the cross-checked source list):
//! 1. Header (12 bytes): magic(4) + hashOffset(4) + fileCount(4).
//! 2. File-size/offset table: `fileCount` entries of (size: u32, offset: u32).
//!    `offset` is **relative to `dataOffset`**, not absolute from the start
//!    of the file -- confirmed against nifskope's `bsa.cpp` reference
//!    reader (`dataOffset = 12 + hashOffset + fileCount*8`), a detail the
//!    UESP prose alone does not spell out unambiguously.
//! 3. Name-offset table: `fileCount` entries of (offset: u32), each an
//!    offset into the name block below.
//! 4. Name block: `fileCount` NUL-terminated names, back-to-back.
//! 5. Hash table: `fileCount` entries of (lo: u32, hi: u32) -- sort/lookup
//!    key only, never load-bearing for parsing (this reader does not use it
//!    at all, matching `ba2::tes3`'s own reader, which only consults it to
//!    reconstruct the [`crate::tes3::hash::hash_path`] key for its map).
//! 6. File data: raw, **uncompressed** bytes, back-to-back, starting at
//!    `dataOffset`.
//!
//! Unlike [`crate::tes4`], Morrowind BSA has no folder records at all --
//! every name is a full relative path (e.g. `meshes\armor\cuirass.nif`)
//! stored as a single string, which is also why [`crate::tes3::hash`]
//! hashes the whole path in one pass instead of folder and file name
//! separately.

use crate::error::{Error, Result};
use crate::tes3::format::*;
use std::io::{Read, Seek, SeekFrom};

/// One file entry as seen from the public API. `path` is the full
/// slash-normalized relative path as stored in the archive (e.g.
/// `meshes\armor\cuirass.nif`) -- Morrowind BSA has no separate
/// folder/name split.
#[derive(Debug, Clone)]
pub struct Tes3Entry {
    pub path: String,
    pub size: u64,
}

struct RawFileRecord {
    size: u32,
    offset: u32,
}

/// An opened Morrowind BSA archive. File data is uncompressed, so
/// [`Tes3Archive::read_file`] is a plain seek + read with no codec
/// dispatch.
pub struct Tes3Archive<R> {
    reader: R,
    data_offset: u64,
    entries: Vec<(Tes3Entry, RawFileRecord)>,
}

fn read_u32_le<R: Read>(r: &mut R) -> Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

impl<R: Read + Seek> Tes3Archive<R> {
    /// Parse a Morrowind BSA's header, size/offset table, name-offset
    /// table, and name block. The hash table is skipped entirely (it is
    /// never needed to resolve an entry by index or by path -- lookup by
    /// path is a linear scan over the parsed names, mirroring the fact
    /// that this crate's own [`crate::tes3::hash::hash_path`] is only
    /// needed by the writer, to reproduce the on-disk sort order).
    pub fn open(mut reader: R) -> Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if magic != MAGIC {
            return Err(Error::BadSignature);
        }

        let hash_offset = read_u32_le(&mut reader)? as u64;
        let file_count = read_u32_le(&mut reader)? as usize;

        // ---- File-size/offset table ----
        let mut raw_records = Vec::with_capacity(file_count);
        for _ in 0..file_count {
            let size = read_u32_le(&mut reader)?;
            let offset = read_u32_le(&mut reader)?;
            raw_records.push(RawFileRecord { size, offset });
        }

        // ---- Name-offset table ----
        let mut name_offsets = Vec::with_capacity(file_count);
        for _ in 0..file_count {
            name_offsets.push(read_u32_le(&mut reader)? as u64);
        }

        // ---- Name block ----
        // Starts immediately after the name-offset table; each `name_offset`
        // is relative to the start of this block.
        let name_block_start = reader.stream_position()?;
        let mut names = Vec::with_capacity(file_count);
        for &rel in &name_offsets {
            reader.seek(SeekFrom::Start(name_block_start + rel))?;
            names.push(read_cstr(&mut reader)?);
        }

        // ---- Derive dataOffset, per nifskope's reference reader ----
        // `12` is the fixed header size; the hash table (skipped) sits
        // immediately before the file data, so dataOffset must account for
        // it even though this reader never touches its bytes.
        let data_offset = HEADER_SIZE + hash_offset + (file_count as u64) * HASH_ENTRY_SIZE;

        let entries = raw_records
            .into_iter()
            .zip(names)
            .map(|(raw, path)| {
                (
                    Tes3Entry {
                        path,
                        size: raw.size as u64,
                    },
                    raw,
                )
            })
            .collect();

        Ok(Tes3Archive {
            reader,
            data_offset,
            entries,
        })
    }

    /// List every entry in the archive.
    pub fn entries(&self) -> Vec<Tes3Entry> {
        self.entries.iter().map(|(e, _)| e.clone()).collect()
    }

    /// Read one file's raw (always-uncompressed) bytes by index into
    /// [`Tes3Archive::entries`].
    pub fn read_file(&mut self, idx: usize) -> Result<Vec<u8>> {
        let (_, raw) = self
            .entries
            .get(idx)
            .ok_or_else(|| Error::NoSuchEntry(format!("index {idx}")))?;

        self.reader
            .seek(SeekFrom::Start(self.data_offset + raw.offset as u64))?;
        let mut buf = vec![0u8; raw.size as usize];
        self.reader.read_exact(&mut buf)?;
        Ok(buf)
    }
}

fn read_cstr<R: Read>(r: &mut R) -> Result<String> {
    let mut buf = Vec::new();
    loop {
        let mut b = [0u8; 1];
        r.read_exact(&mut b)?;
        if b[0] == 0 {
            break;
        }
        buf.push(b[0]);
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}
