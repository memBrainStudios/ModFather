//! Top-level `Archive` type: open a 7z file, list its entries, extract
//! bytes, or create a brand-new archive from scratch.

use crate::codec::{self, PackCodec};
use crate::error::{Error, Result};
use crate::format::{property_id::*, SIGNATURE, START_HEADER_LEN};
use crate::header::{read_header, Folder, Header};
use crate::varint::write_number;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;

/// One entry as seen from the public API: just enough to list a directory
/// listing or drive a targeted extraction.
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
    pub crc: Option<u32>,
}

/// An opened 7z archive, ready for listing/extraction.
pub struct Archive<R> {
    reader: R,
    base_offset: u64,
    header: Header,
}

impl Archive<BufReader<File>> {
    /// Open a 7z file from disk.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        Self::from_reader(BufReader::new(file))
    }
}

impl<R: Read + Seek> Archive<R> {
    /// Open a 7z archive from any seekable reader.
    pub fn from_reader(mut reader: R) -> Result<Self> {
        let mut sig = [0u8; 6];
        reader.read_exact(&mut sig)?;
        if sig != SIGNATURE {
            return Err(Error::BadSignature);
        }

        // ArchiveVersion: 1 byte major, 1 byte minor. Not otherwise validated;
        // 7z has stayed at 0.4 for the whole life of the format.
        let mut version = [0u8; 2];
        reader.read_exact(&mut version)?;

        let mut start_header_crc_buf = [0u8; 4];
        reader.read_exact(&mut start_header_crc_buf)?;
        let start_header_crc = u32::from_le_bytes(start_header_crc_buf);

        let mut start_header = [0u8; START_HEADER_LEN];
        reader.read_exact(&mut start_header)?;

        if crc32fast::hash(&start_header) != start_header_crc {
            return Err(Error::StartHeaderCrcMismatch);
        }

        let next_header_offset = u64::from_le_bytes(start_header[0..8].try_into().unwrap());
        let next_header_size = u64::from_le_bytes(start_header[8..16].try_into().unwrap());
        let next_header_crc = u32::from_le_bytes(start_header[16..20].try_into().unwrap());

        // Base offset: everything in PackInfo/file offsets is relative to
        // just after the 32-byte SignatureHeader.
        let base_offset = 32u64;

        if next_header_size == 0 {
            // Empty archive: no header at all (0 files).
            return Ok(Archive {
                reader,
                base_offset,
                header: Header::default(),
            });
        }

        reader.seek(SeekFrom::Start(base_offset + next_header_offset))?;
        let mut next_header_bytes = vec![0u8; next_header_size as usize];
        reader.read_exact(&mut next_header_bytes)?;

        if crc32fast::hash(&next_header_bytes) != next_header_crc {
            return Err(Error::NextHeaderCrcMismatch);
        }

        // The next header may itself be `kEncodedHeader`-compressed.
        let header_bytes = decode_possibly_encoded_header(&next_header_bytes, &mut reader, base_offset)?;

        let header = read_header(&mut std::io::Cursor::new(header_bytes))?;

        Ok(Archive {
            reader,
            base_offset,
            header,
        })
    }

    /// List every entry (files and, implicitly, any empty "directory marker"
    /// entries) in the archive, in on-disk order.
    pub fn entries(&self) -> Vec<Entry> {
        self.header
            .files
            .iter()
            .map(|f| Entry {
                name: f.name.clone(),
                size: f.size,
                is_dir: !f.has_stream && !f.is_empty_file,
                crc: f.crc,
            })
            .collect()
    }

    /// Extract one file's decoded bytes by exact name match.
    pub fn read_file(&mut self, name: &str) -> Result<Vec<u8>> {
        let idx = self
            .header
            .files
            .iter()
            .position(|f| f.name == name)
            .ok_or_else(|| Error::NoSuchEntry(name.to_string()))?;
        self.read_file_at(idx)
    }

    /// Extract one file's decoded bytes by index into [`Archive::entries`].
    pub fn read_file_at(&mut self, idx: usize) -> Result<Vec<u8>> {
        let file = self.header.files[idx].clone();
        if !file.has_stream {
            return Ok(Vec::new());
        }
        let folder_idx = file
            .folder_index
            .ok_or_else(|| Error::Malformed(format!("file {} has no folder", file.name)))?;
        let folder_bytes = self.decode_folder(folder_idx)?;

        let start = file.offset_in_folder as usize;
        let end = start + file.size as usize;
        if end > folder_bytes.len() {
            return Err(Error::Malformed(format!(
                "file {} extends past its folder's decoded size",
                file.name
            )));
        }
        let bytes = folder_bytes[start..end].to_vec();

        if let Some(expected) = file.crc {
            if crc32fast::hash(&bytes) != expected {
                return Err(Error::StreamCrcMismatch);
            }
        }

        Ok(bytes)
    }

    /// Extract every file in the archive under `dest_dir`, preserving
    /// relative paths (using `/` as read from the header, which is
    /// normalized to the host path separator).
    pub fn extract_all<P: AsRef<Path>>(&mut self, dest_dir: P) -> Result<()> {
        let dest_dir = dest_dir.as_ref();
        std::fs::create_dir_all(dest_dir)?;
        for idx in 0..self.header.files.len() {
            let file = self.header.files[idx].clone();
            let rel = file.name.replace('\\', "/");
            let out_path = dest_dir.join(&rel);
            if !file.has_stream {
                std::fs::create_dir_all(&out_path)?;
                continue;
            }
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let bytes = self.read_file_at(idx)?;
            std::fs::write(&out_path, bytes)?;
        }
        Ok(())
    }

    fn decode_folder(&mut self, folder_idx: usize) -> Result<Vec<u8>> {
        let folder = self.header.folders[folder_idx].clone();

        if folder.coders.len() != 1 {
            return Err(Error::UnsupportedCodec(
                "multi-coder folders (filters/BCJ2 chains) are not supported by sevenzip-re yet"
                    .into(),
            ));
        }
        let coder = &folder.coders[0];

        // Compute this folder's absolute pack-stream byte range. Pack
        // streams are laid out consecutively starting at `base_offset +
        // pack_pos`, in the same order as `pack_info.pack_sizes`; a folder
        // consumes a contiguous run of them.
        let global_pack_index = self.folder_pack_start_index(folder_idx);
        let pack_sizes = &self.header.pack_info.pack_sizes;
        let num_pack = folder.num_pack_streams();
        if num_pack != 1 {
            return Err(Error::UnsupportedCodec(
                "folders spanning multiple pack streams are not supported by sevenzip-re yet"
                    .into(),
            ));
        }
        let pack_size = pack_sizes[global_pack_index];
        let pack_offset: u64 = self.header.pack_info.pack_pos
            + pack_sizes[..global_pack_index].iter().sum::<u64>();

        self.reader
            .seek(SeekFrom::Start(self.base_offset + pack_offset))?;
        let packed = codec::read_exact_vec(&mut self.reader, pack_size as usize)?;

        let unpack_size = folder.unpack_size()?;
        let decoded = codec::decode(coder, &packed, unpack_size)?;

        if decoded.len() as u64 != unpack_size {
            return Err(Error::Malformed(format!(
                "folder {folder_idx}: decoded {} bytes, expected {}",
                decoded.len(),
                unpack_size
            )));
        }
        if let Some(expected_crc) = folder.crc {
            if crc32fast::hash(&decoded) != expected_crc {
                return Err(Error::StreamCrcMismatch);
            }
        }

        Ok(decoded)
    }

    fn folder_pack_start_index(&self, folder_idx: usize) -> usize {
        self.header.folders[..folder_idx]
            .iter()
            .map(Folder::num_pack_streams)
            .sum()
    }
}

/// If the `next_header_bytes` decode to a single `kEncodedHeader` folder,
/// decode that folder and return the real header bytes; otherwise return the
/// input unchanged (it's already a plain `kHeader`).
fn decode_possibly_encoded_header<R: Read + Seek>(
    bytes: &[u8],
    reader: &mut R,
    base_offset: u64,
) -> Result<Vec<u8>> {
    let mut cursor = std::io::Cursor::new(bytes);
    let mut id = [0u8; 1];
    cursor.read_exact(&mut id)?;

    if id[0] == K_ENCODED_HEADER {
        // Parse this as a StreamsInfo (PackInfo + UnpackInfo, no SubStreamsInfo).
        let mut pack_info = crate::header::PackInfo::default();
        let mut folders: Vec<Folder> = Vec::new();

        let mut next = [0u8; 1];
        cursor.read_exact(&mut next)?;
        if next[0] == K_PACK_INFO {
            pack_info = crate::header::read_pack_info(&mut cursor)?;
            cursor.read_exact(&mut next)?;
        }
        if next[0] == K_UNPACK_INFO {
            folders = crate::header::read_unpack_info(&mut cursor)?;
            cursor.read_exact(&mut next)?;
        }
        if next[0] != K_END {
            return Err(Error::Malformed(
                "unexpected trailing data after encoded-header StreamsInfo".into(),
            ));
        }

        if folders.len() != 1 || folders[0].coders.len() != 1 {
            return Err(Error::UnsupportedCodec(
                "compressed header with multiple folders/coders is not supported".into(),
            ));
        }
        let folder = &folders[0];
        let coder = &folder.coders[0];

        reader.seek(SeekFrom::Start(base_offset + pack_info.pack_pos))?;
        let pack_size = pack_info.pack_sizes[0];
        let mut packed = vec![0u8; pack_size as usize];
        reader.read_exact(&mut packed)?;

        let unpack_size = folder.unpack_size()?;
        let decoded = codec::decode(coder, &packed, unpack_size)?;
        Ok(decoded)
    } else if id[0] == K_HEADER {
        Ok(bytes.to_vec())
    } else {
        Err(Error::Malformed(format!(
            "unexpected top-level id {:#x} in NextHeader",
            id[0]
        )))
    }
}

// ---------------------------------------------------------------------------
// Writer: build a new 7z archive from a set of (name, bytes) entries.
// ---------------------------------------------------------------------------

/// One entry to pack into a new archive.
pub struct NewEntry {
    pub name: String,
    pub data: Vec<u8>,
}

/// Create a brand-new 7z archive at `path` containing `entries`, each folder
/// encoded independently with `codec` (one folder per file, for simplicity
/// and to keep Wave 0's writer straightforward; solid multi-file folders are
/// a later optimization, not a format requirement).
pub fn create<P: AsRef<Path>>(path: P, entries: &[NewEntry], codec_choice: PackCodec) -> Result<()> {
    let mut out = File::create(path)?;

    let mut pack_sizes: Vec<u64> = Vec::new();
    let mut pack_bytes: Vec<u8> = Vec::new();
    let mut folder_headers: Vec<u8> = Vec::new();
    let mut coders_unpack_sizes: Vec<u8> = Vec::new();
    let mut folder_crcs: Vec<u32> = Vec::new();
    let mut file_sizes: Vec<u64> = Vec::new();
    let mut file_crcs: Vec<u32> = Vec::new();

    for entry in entries {
        let (packed, properties) = codec::encode(codec_choice, &entry.data)?;
        pack_sizes.push(packed.len() as u64);
        pack_bytes.extend_from_slice(&packed);

        // One coder per folder: flag byte with codec-id-size + hasAttributes.
        let codec_id: &[u8] = match codec_choice {
            PackCodec::Copy => crate::format::codec_id::COPY,
            PackCodec::Lzma => crate::format::codec_id::LZMA,
            PackCodec::Lzma2 => crate::format::codec_id::LZMA2,
        };
        write_number(&mut folder_headers, 1)?; // NumCoders
        let flag = (codec_id.len() as u8) | if properties.is_empty() { 0 } else { 0x20 };
        folder_headers.push(flag);
        folder_headers.extend_from_slice(codec_id);
        if !properties.is_empty() {
            write_number(&mut folder_headers, properties.len() as u64)?;
            folder_headers.extend_from_slice(&properties);
        }
        // No bind pairs (single coder), no packed-stream-index list needed
        // (implied when NumPackedStreams == 1 for the folder).

        write_number(&mut coders_unpack_sizes, entry.data.len() as u64)?;
        folder_crcs.push(crc32fast::hash(&entry.data));
        file_sizes.push(entry.data.len() as u64);
        file_crcs.push(crc32fast::hash(&entry.data));
    }

    let num_folders = entries.len();

    // --- Assemble the (uncompressed) Header stream ---
    let mut header = Vec::new();
    header.push(K_HEADER);

    header.push(K_MAIN_STREAMS_INFO);

    // PackInfo
    header.push(K_PACK_INFO);
    write_number(&mut header, 0)?; // PackPos, relative to base offset
    write_number(&mut header, pack_sizes.len() as u64)?;
    header.push(K_SIZE);
    for s in &pack_sizes {
        write_number(&mut header, *s)?;
    }
    header.push(K_END); // end PackInfo

    // UnpackInfo
    header.push(K_UNPACK_INFO);
    header.push(K_FOLDER);
    write_number(&mut header, num_folders as u64)?;
    header.push(0); // External = 0
    header.extend_from_slice(&folder_headers);
    header.push(K_CODERS_UNPACK_SIZE);
    header.extend_from_slice(&coders_unpack_sizes);
    header.push(K_CRC);
    header.push(1); // AllAreDefined = true
    for crc in &folder_crcs {
        header.extend_from_slice(&crc.to_le_bytes());
    }
    header.push(K_END); // end UnpackInfo

    // No SubStreamsInfo needed: exactly one substream (== the whole folder)
    // per folder, which is the implicit default.
    header.push(K_END); // end MainStreamsInfo

    // FilesInfo
    header.push(K_FILES_INFO);
    write_number(&mut header, entries.len() as u64)?;

    // kName
    let mut name_prop = Vec::new();
    name_prop.push(0u8); // External = 0
    for entry in entries {
        for unit in entry.name.encode_utf16() {
            name_prop.extend_from_slice(&unit.to_le_bytes());
        }
        name_prop.extend_from_slice(&0u16.to_le_bytes());
    }
    header.push(K_NAME);
    write_number(&mut header, name_prop.len() as u64)?;
    header.extend_from_slice(&name_prop);

    header.push(K_END); // end FilesInfo
    header.push(K_END); // end Header

    let next_header_crc = crc32fast::hash(&header);
    let next_header_offset = pack_bytes.len() as u64; // right after pack data
    let next_header_size = header.len() as u64;

    // --- Write SignatureHeader + pack data + header ---
    out.write_all(&SIGNATURE)?;
    out.write_all(&[0u8, 4u8])?; // ArchiveVersion 0.4

    let mut start_header = Vec::with_capacity(START_HEADER_LEN);
    start_header.extend_from_slice(&next_header_offset.to_le_bytes());
    start_header.extend_from_slice(&next_header_size.to_le_bytes());
    start_header.extend_from_slice(&next_header_crc.to_le_bytes());
    let start_header_crc = crc32fast::hash(&start_header);

    out.write_all(&start_header_crc.to_le_bytes())?;
    out.write_all(&start_header)?;
    out.write_all(&pack_bytes)?;
    out.write_all(&header)?;

    let _ = file_sizes;
    let _ = file_crcs;
    Ok(())
}
