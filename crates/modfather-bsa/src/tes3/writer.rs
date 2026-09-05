//! Morrowind (TES III) BSA writer.
//!
//! Section order matches [`crate::tes3::reader`]'s read order: header,
//! file-size/offset table, name-offset table, name block, hash table, file
//! data. All uncompressed -- Morrowind BSA predates any per-file codec.
//!
//! Entries are written **hash-sorted** (ascending by
//! [`crate::tes3::hash::hash_path`]), mirroring `ba2::tes3::Archive`'s own
//! writer, which stores entries in a sorted map keyed by hash and iterates
//! that map for every section. This crate's own reader does not require
//! that order (it trusts the stored size/offset/name-offset tables
//! directly, same as `ba2`'s reader does), but matching it keeps output
//! byte-for-byte comparable to the independent oracle and to real Morrowind
//! tooling.
//!
//! Per UESP's "Morrowind Mod:BSA File Format": hash collisions between two
//! distinct paths are a hard error, not a silent overwrite -- this writer
//! rejects them via [`crate::error::Error::HashCollision`].

use crate::error::{Error, Result};
use crate::tes3::format::*;
use crate::tes3::hash::hash_path;
use std::io::Write;

/// One file to pack, addressed by its full archive-relative path (e.g.
/// `meshes\armor\cuirass.nif` or `meshes/armor/cuirass.nif` -- normalized
/// internally). Morrowind BSA has no folder/name split, unlike
/// [`crate::tes4::writer::FileToPack`].
#[derive(Debug, Clone)]
pub struct Tes3FileToPack {
    pub path: String,
    pub data: Vec<u8>,
}

fn write_u32_le<W: Write>(w: &mut W, v: u32) -> Result<()> {
    w.write_all(&v.to_le_bytes())?;
    Ok(())
}

fn write_cstr<W: Write>(w: &mut W, s: &str) -> Result<()> {
    w.write_all(s.as_bytes())?;
    w.write_all(&[0u8])?;
    Ok(())
}

fn normalize_path(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

/// Pack `files` into a Morrowind BSA archive and write it to `writer`.
pub fn write<W: Write>(mut writer: W, files: &[Tes3FileToPack]) -> Result<()> {
    // Normalize, hash, and sort ascending by hash (matches `ba2::tes3`'s
    // sorted-map iteration order; see this module's doc comment).
    let mut entries: Vec<(String, u64, &[u8])> = files
        .iter()
        .map(|f| {
            let path = normalize_path(&f.path);
            let h = hash_path(&path);
            (path, h, f.data.as_slice())
        })
        .collect();
    entries.sort_by_key(|(_, h, _)| *h);

    for w in entries.windows(2) {
        let (path_a, hash_a, _) = &w[0];
        let (path_b, hash_b, _) = &w[1];
        if hash_a == hash_b {
            return Err(Error::HashCollision(path_a.clone(), path_b.clone(), *hash_a));
        }
    }

    let file_count = entries.len() as u32;

    // hashOffset is defined relative to the *end of the header*, i.e. it is
    // the byte length of every section between the header and the hash
    // table: the file-size/offset table, the name-offset table, and the
    // name block. Cross-checked against `ba2::tes3::Archive::make_header`,
    // whose `names_offset = 0xC * file_count` is exactly
    // `(FILE_SIZE_OFFSET_ENTRY_SIZE + NAME_OFFSET_ENTRY_SIZE) * file_count`
    // (`0xC == 12 == 8 + 4`) -- i.e. it also folds in the file-size/offset
    // table's length, which this crate's first draft omitted, producing a
    // `dataOffset` that undershot the real file-data start by exactly that
    // table's size.
    let file_size_offset_table_len: u64 = FILE_SIZE_OFFSET_ENTRY_SIZE * entries.len() as u64;
    let name_offsets_len: u64 = NAME_OFFSET_ENTRY_SIZE * entries.len() as u64;
    let names_len: u64 = entries
        .iter()
        .map(|(path, _, _)| path.len() as u64 + 1)
        .sum();
    let hash_offset = (file_size_offset_table_len + name_offsets_len + names_len) as u32;

    // ---- Header (12 bytes) ----
    writer.write_all(&MAGIC)?;
    write_u32_le(&mut writer, hash_offset)?;
    write_u32_le(&mut writer, file_count)?;

    // ---- File-size/offset table ----
    // `offset` is relative to dataOffset, i.e. the running total of
    // preceding files' sizes -- not an absolute file position.
    let mut running_offset: u32 = 0;
    for (_, _, data) in &entries {
        write_u32_le(&mut writer, data.len() as u32)?;
        write_u32_le(&mut writer, running_offset)?;
        running_offset = running_offset
            .checked_add(data.len() as u32)
            .ok_or_else(|| Error::Malformed("archive exceeds 4GiB for TES3 BSA".into()))?;
    }

    // ---- Name-offset table ----
    let mut running_name_offset: u32 = 0;
    for (path, _, _) in &entries {
        write_u32_le(&mut writer, running_name_offset)?;
        running_name_offset += path.len() as u32 + 1;
    }

    // ---- Name block ----
    for (path, _, _) in &entries {
        write_cstr(&mut writer, path)?;
    }

    // ---- Hash table ----
    for (_, h, _) in &entries {
        let lo = (*h >> 32) as u32;
        let hi = (*h & 0xFFFF_FFFF) as u32;
        write_u32_le(&mut writer, lo)?;
        write_u32_le(&mut writer, hi)?;
    }

    // ---- File data ----
    for (_, _, data) in &entries {
        writer.write_all(data)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;

    #[test]
    fn rejects_hash_colliding_paths() {
        // Two different-cased spellings of the *same* normalized path
        // always collide (case-insensitive hash) -- a cheap, deterministic
        // way to exercise the collision-detection path without needing a
        // real two-distinct-path collision, which the hash's collision
        // probability makes impractical to construct by hand.
        let files = vec![
            Tes3FileToPack {
                path: "Meshes\\Sword.NIF".to_string(),
                data: b"first".to_vec(),
            },
            Tes3FileToPack {
                path: "meshes\\sword.nif".to_string(),
                data: b"second, different bytes, same normalized path".to_vec(),
            },
        ];
        let mut buf = Vec::new();
        let result = write(&mut buf, &files);
        assert!(matches!(result, Err(Error::HashCollision(_, _, _))));
    }
}
