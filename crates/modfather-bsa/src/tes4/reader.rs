//! BSA v103-105 reader with version-aware codec dispatch.

use crate::error::{Error, Result};
use crate::tes4::format::*;
use std::io::{Read, Seek, SeekFrom};

/// One file entry as seen from the public API.
#[derive(Debug, Clone)]
pub struct BsaEntry {
    pub folder: String,
    pub name: String,
    pub size: u64,
}

struct RawFileRecord {
    #[allow(dead_code)] // kept for future hash-based lookup/sorting
    name_hash: u64,
    size: u32,
    offset: u32,
}

/// An opened BSA archive.
pub struct BsaArchive<R> {
    reader: R,
    version: u32,
    archive_flags: u32,
    entries: Vec<(BsaEntry, RawFileRecord)>,
}

fn read_u32_le<R: Read>(r: &mut R) -> Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn read_u64_le<R: Read>(r: &mut R) -> Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
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

/// bzstring: 1-byte length (including the trailing NUL), then the bytes,
/// then the NUL itself.
fn read_bzstring<R: Read>(r: &mut R) -> Result<String> {
    let mut len = [0u8; 1];
    r.read_exact(&mut len)?;
    let len = len[0] as usize;
    if len == 0 {
        return Ok(String::new());
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    // Last byte is the NUL terminator.
    let body = &buf[..len.saturating_sub(1)];
    Ok(String::from_utf8_lossy(body).into_owned())
}

impl<R: Read + Seek> BsaArchive<R> {
    /// Parse a BSA archive's header, folder records, and file records.
    /// Actual file bytes are decoded lazily by [`BsaArchive::read_file`].
    pub fn open(mut reader: R) -> Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if magic != MAGIC {
            return Err(Error::BadSignature);
        }

        let version = read_u32_le(&mut reader)?;
        if !(103..=105).contains(&version) {
            return Err(Error::UnsupportedVersion(version));
        }

        let _offset = read_u32_le(&mut reader)?; // always 36
        let archive_flags = read_u32_le(&mut reader)?;
        let folder_count = read_u32_le(&mut reader)? as usize;
        let file_count = read_u32_le(&mut reader)? as usize;
        let _total_folder_name_len = read_u32_le(&mut reader)?;
        let _total_file_name_len = read_u32_le(&mut reader)?;
        let _file_flags = read_u32_le(&mut reader)?;

        let include_dir_names = archive_flags & archive_flags::INCLUDE_DIR_NAMES != 0;
        let include_file_names = archive_flags & archive_flags::INCLUDE_FILE_NAMES != 0;

        // Folder records: nameHash(8) + count(4) [+ pad(4) v105] + offset(4) [+ pad(4) v105].
        struct FolderRec {
            count: u32,
            #[allow(dead_code)]
            name_hash: u64,
        }
        let mut folder_recs = Vec::with_capacity(folder_count);
        for _ in 0..folder_count {
            let name_hash = read_u64_le(&mut reader)?;
            let count = read_u32_le(&mut reader)?;
            if version >= 105 {
                let mut pad = [0u8; 4];
                reader.read_exact(&mut pad)?;
            }
            let _offset = read_u32_le(&mut reader)?;
            if version >= 105 {
                let mut pad = [0u8; 4];
                reader.read_exact(&mut pad)?;
            }
            folder_recs.push(FolderRec { count, name_hash });
        }

        // File-record blocks: one per folder, optionally preceded by the
        // folder's bzstring name; each is `count` file records.
        let mut raw_entries: Vec<(String, RawFileRecord)> = Vec::with_capacity(file_count);
        for frec in &folder_recs {
            let folder_name = if include_dir_names {
                read_bzstring(&mut reader)?
            } else {
                String::new()
            };
            for _ in 0..frec.count {
                let name_hash = read_u64_le(&mut reader)?;
                let size = read_u32_le(&mut reader)?;
                let offset = read_u32_le(&mut reader)?;
                raw_entries.push((
                    folder_name.clone(),
                    RawFileRecord {
                        name_hash,
                        size,
                        offset,
                    },
                ));
            }
        }

        // File-names block: one NUL-terminated lowercase name per file, in
        // the same order as the file records above.
        let mut names = Vec::with_capacity(file_count);
        if include_file_names {
            for _ in 0..file_count {
                names.push(read_cstr(&mut reader)?);
            }
        } else {
            names.resize(file_count, String::new());
        }

        let entries = raw_entries
            .into_iter()
            .zip(names)
            .map(|((folder, raw), name)| {
                (
                    BsaEntry {
                        folder,
                        name,
                        size: 0, // resolved lazily (depends on compression)
                    },
                    raw,
                )
            })
            .collect();

        Ok(BsaArchive {
            reader,
            version,
            archive_flags,
            entries,
        })
    }

    /// List every entry in the archive.
    pub fn entries(&self) -> Vec<BsaEntry> {
        self.entries.iter().map(|(e, _)| e.clone()).collect()
    }

    /// Read and decode one file's bytes by index into [`BsaArchive::entries`].
    pub fn read_file(&mut self, idx: usize) -> Result<Vec<u8>> {
        let (_, raw) = self
            .entries
            .get(idx)
            .ok_or_else(|| Error::NoSuchEntry(format!("index {idx}")))?;

        let archive_default_compressed =
            self.archive_flags & archive_flags::COMPRESSED_ARCHIVE != 0;
        let inverted = raw.size & FILE_SIZE_COMPRESSION_INVERT_BIT != 0;
        let is_compressed = archive_default_compressed ^ inverted;
        let on_disk_size = (raw.size & FILE_SIZE_MASK) as u64;

        self.reader.seek(SeekFrom::Start(raw.offset as u64))?;

        let embed_names = self.archive_flags & archive_flags::EMBED_FILE_NAMES != 0;
        let mut remaining = on_disk_size;
        if embed_names {
            let mut len = [0u8; 1];
            self.reader.read_exact(&mut len)?;
            let name_len = len[0] as u64;
            let mut skip = vec![0u8; name_len as usize];
            self.reader.read_exact(&mut skip)?;
            remaining -= 1 + name_len;
        }

        if !is_compressed {
            let mut buf = vec![0u8; remaining as usize];
            self.reader.read_exact(&mut buf)?;
            return Ok(buf);
        }

        // Compressed payload: 4-byte little-endian original size, then the
        // codec-specific compressed body. Codec choice is version-dependent:
        // v103/v104 use zlib; v105 uses LZ4 — unlike the reference
        // implementation, which used `ZlibDecoder` unconditionally and
        // would silently produce garbage on real v105 (Skyrim SE/AE) BSAs.
        let original_size = read_u32_le(&mut self.reader)? as usize;
        let body_len = remaining - 4;
        let mut body = vec![0u8; body_len as usize];
        self.reader.read_exact(&mut body)?;

        let decoded = if self.version >= 105 {
            decode_lz4(&body, original_size)?
        } else {
            decode_zlib(&body, original_size)?
        };

        Ok(decoded)
    }
}

fn decode_zlib(body: &[u8], original_size: usize) -> Result<Vec<u8>> {
    use std::io::Read as _;
    let mut dec = flate2::read::ZlibDecoder::new(body);
    let mut out = Vec::with_capacity(original_size);
    dec.read_to_end(&mut out).map_err(|e| Error::Zlib(e.to_string()))?;
    Ok(out)
}

/// v105 (Skyrim SE/AE) BSA payloads use the **LZ4 frame format**, not raw
/// LZ4 blocks. This was confirmed against an independently-written oracle
/// (the `ba2` crate's `tes4::File` codec, which uses `lzzzz::lz4f`) and a
/// second, independent real-game-verified source (ByroRedux's BSA reader
/// notes, which explicitly document "v105: LZ4 frame format" as confirmed
/// against actual Skyrim SE archives). An earlier revision of this reader
/// used `lz4_flex::block::decompress` (raw block, no frame header/checksum)
/// which would silently fail or produce garbage against real v105 BSAs.
fn decode_lz4(body: &[u8], original_size: usize) -> Result<Vec<u8>> {
    use lz4_flex::frame::FrameDecoder;
    use std::io::Read as _;
    let mut dec = FrameDecoder::new(body);
    let mut out = Vec::with_capacity(original_size);
    dec.read_to_end(&mut out)
        .map_err(|e| Error::Lz4(e.to_string()))?;
    Ok(out)
}
