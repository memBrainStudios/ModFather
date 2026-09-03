//! Parsing (and, eventually, writing) of the 7z `Header` structure:
//! `PackInfo`, `Folder` (`UnpackInfo`), `SubStreamsInfo`, `FilesInfo`.
//!
//! Layout reference: ip7z/7zip `DOC/7zFormat.txt`.

use crate::error::{Error, Result};
use crate::format::property_id::*;
use crate::varint::{read_number, read_number_usize};
use std::io::Read;

/// One coder inside a `Folder`: a codec id plus its properties and stream
/// arity (in/out streams; >1 only for "complex" coders like BCJ2, which
/// `sevenzip-re` does not implement).
#[derive(Debug, Clone)]
pub struct Coder {
    pub codec_id: Vec<u8>,
    pub num_in_streams: usize,
    pub num_out_streams: usize,
    pub properties: Vec<u8>,
}

/// A bind pair connects one coder's output stream to another coder's input
/// stream inside a multi-coder folder (e.g. filter -> LZMA).
#[derive(Debug, Clone, Copy)]
pub struct BindPair {
    pub in_index: usize,
    pub out_index: usize,
}

/// A `Folder` is 7z's unit of decompression: an ordered pipeline of coders
/// (usually just one: Copy, LZMA, or LZMA2) that together turn N packed
/// streams into one contiguous decoded byte stream.
#[derive(Debug, Clone, Default)]
pub struct Folder {
    pub coders: Vec<Coder>,
    pub bind_pairs: Vec<BindPair>,
    /// Indices (into the folder's pack-stream list) of in-streams *not*
    /// consumed by a bind pair, in order; these map 1:1 onto this folder's
    /// slice of the archive's global pack-stream list.
    pub packed_indices: Vec<usize>,
    /// Per-out-stream unpacked size, filled in from `CodersUnpackSize`.
    pub unpack_sizes: Vec<u64>,
    /// CRC32 of the folder's fully-decoded output, if present.
    pub crc: Option<u32>,
    /// How many files this folder's decoded stream is split into
    /// (from `SubStreamsInfo::NumUnpackStreamsInFolders`; defaults to 1).
    pub num_unpack_substreams: usize,
}

impl Folder {
    /// Index of the folder's single "final" output stream: the one output
    /// stream that is not consumed as another coder's input via a bind pair.
    /// For the single-coder folders `sevenzip-re` supports this is always
    /// output 0, but we compute it generally.
    pub fn find_final_out_stream(&self) -> Result<usize> {
        let mut total_out = 0usize;
        for c in &self.coders {
            total_out += c.num_out_streams;
        }
        for out_idx in 0..total_out {
            if !self.bind_pairs.iter().any(|bp| bp.out_index == out_idx) {
                return Ok(out_idx);
            }
        }
        Err(Error::Malformed("folder has no unbound output stream".into()))
    }

    /// Total decoded size of this folder (the final output stream's size).
    pub fn unpack_size(&self) -> Result<u64> {
        let idx = self.find_final_out_stream()?;
        self.unpack_sizes
            .get(idx)
            .copied()
            .ok_or_else(|| Error::Malformed("missing unpack size for final stream".into()))
    }

    /// How many packed (input) streams this folder consumes from the
    /// archive's global pack-stream list.
    pub fn num_pack_streams(&self) -> usize {
        self.packed_indices.len()
    }
}

/// `PackInfo`: where the packed (compressed) streams live in the file, and
/// their sizes (and optionally CRCs).
#[derive(Debug, Clone, Default)]
pub struct PackInfo {
    pub pack_pos: u64,
    pub pack_sizes: Vec<u64>,
    pub pack_crcs: Vec<Option<u32>>,
}

/// One file entry from `FilesInfo`.
#[derive(Debug, Clone, Default)]
pub struct FileEntry {
    pub name: String,
    pub has_stream: bool,
    pub is_empty_file: bool,
    pub is_anti: bool,
    pub attributes: Option<u32>,
    /// Windows FILETIME (100ns ticks since 1601-01-01), if present.
    pub mtime: Option<u64>,
    pub ctime: Option<u64>,
    pub atime: Option<u64>,
    /// Uncompressed size, resolved from the owning folder's substream sizes.
    pub size: u64,
    /// CRC32 of this file's decoded content, if known.
    pub crc: Option<u32>,
    /// Which folder (if any) holds this file's bytes; `None` for empty files.
    pub folder_index: Option<usize>,
    /// Byte offset of this file's content within its folder's decoded stream.
    pub offset_in_folder: u64,
}

/// The fully parsed `Header`, ready to drive extraction.
#[derive(Debug, Clone, Default)]
pub struct Header {
    pub pack_info: PackInfo,
    pub folders: Vec<Folder>,
    pub files: Vec<FileEntry>,
}

/// Read a `BYTE BoolVector[NumFiles]` biterator packed as one bit per item.
fn read_bit_vector<R: Read>(r: &mut R, count: usize) -> Result<Vec<bool>> {
    let mut out = Vec::with_capacity(count);
    let mut b = 0u8;
    let mut mask = 0u8;
    for _ in 0..count {
        if mask == 0 {
            let mut byte = [0u8; 1];
            r.read_exact(&mut byte)?;
            b = byte[0];
            mask = 0x80;
        }
        out.push(b & mask != 0);
        mask >>= 1;
    }
    Ok(out)
}

/// Read `AllAreDefined` (BYTE) then either an implicit all-true vector or a
/// real `BoolVector`, per 7zFormat's `BitVector`/`OptionalBoolVector` idiom.
fn read_optional_bit_vector<R: Read>(r: &mut R, count: usize) -> Result<Vec<bool>> {
    let mut all_defined = [0u8; 1];
    r.read_exact(&mut all_defined)?;
    if all_defined[0] != 0 {
        Ok(vec![true; count])
    } else {
        read_bit_vector(r, count)
    }
}

fn read_u32_le<R: Read>(r: &mut R) -> Result<u32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

/// Read a `Digests[NumStreams]` block: `AllAreDefined` + optional Defined
/// bits, then one `UINT32` CRC per stream whose Defined bit is set.
fn read_digests<R: Read>(r: &mut R, count: usize) -> Result<Vec<Option<u32>>> {
    let defined = read_optional_bit_vector(r, count)?;
    let mut out = Vec::with_capacity(count);
    for is_defined in defined {
        if is_defined {
            out.push(Some(read_u32_le(r)?));
        } else {
            out.push(None);
        }
    }
    Ok(out)
}

pub(crate) fn read_pack_info<R: Read>(r: &mut R) -> Result<PackInfo> {
    let pack_pos = read_number(r)?;
    let num_pack_streams = read_number_usize(r)?;

    let mut pack_sizes = Vec::new();
    let mut pack_crcs = vec![None; num_pack_streams];

    loop {
        let mut id = [0u8; 1];
        r.read_exact(&mut id)?;
        match id[0] {
            K_END => break,
            K_SIZE => {
                pack_sizes = (0..num_pack_streams)
                    .map(|_| read_number(r))
                    .collect::<Result<Vec<_>>>()?;
            }
            K_CRC => {
                pack_crcs = read_digests(r, num_pack_streams)?;
            }
            other => return Err(Error::Malformed(format!("unexpected id {other:#x} in PackInfo"))),
        }
    }

    Ok(PackInfo {
        pack_pos,
        pack_sizes,
        pack_crcs,
    })
}

fn read_folder<R: Read>(r: &mut R) -> Result<Folder> {
    let num_coders = read_number_usize(r)?;
    let mut coders = Vec::with_capacity(num_coders);
    let mut total_in = 0usize;
    let mut total_out = 0usize;

    for _ in 0..num_coders {
        let mut flag = [0u8; 1];
        r.read_exact(&mut flag)?;
        let flag = flag[0];
        let codec_id_size = (flag & 0x0F) as usize;
        let is_complex = flag & 0x10 != 0;
        let has_attributes = flag & 0x20 != 0;

        let mut codec_id = vec![0u8; codec_id_size];
        r.read_exact(&mut codec_id)?;

        let (num_in_streams, num_out_streams) = if is_complex {
            (read_number_usize(r)?, read_number_usize(r)?)
        } else {
            (1, 1)
        };

        let properties = if has_attributes {
            let prop_size = read_number_usize(r)?;
            let mut buf = vec![0u8; prop_size];
            r.read_exact(&mut buf)?;
            buf
        } else {
            Vec::new()
        };

        total_in += num_in_streams;
        total_out += num_out_streams;

        coders.push(Coder {
            codec_id,
            num_in_streams,
            num_out_streams,
            properties,
        });
    }

    let num_bind_pairs = total_out.saturating_sub(1);
    let mut bind_pairs = Vec::with_capacity(num_bind_pairs);
    for _ in 0..num_bind_pairs {
        let in_index = read_number_usize(r)?;
        let out_index = read_number_usize(r)?;
        bind_pairs.push(BindPair { in_index, out_index });
    }

    let num_packed_streams = total_in - num_bind_pairs;
    let mut packed_indices = Vec::with_capacity(num_packed_streams);
    if num_packed_streams == 1 {
        // The single packed stream is the one in-index with no bind pair.
        let idx = (0..total_in)
            .find(|i| !bind_pairs.iter().any(|bp| bp.in_index == *i))
            .ok_or_else(|| Error::Malformed("folder: no free input stream".into()))?;
        packed_indices.push(idx);
    } else {
        for _ in 0..num_packed_streams {
            packed_indices.push(read_number_usize(r)?);
        }
    }

    Ok(Folder {
        coders,
        bind_pairs,
        packed_indices,
        unpack_sizes: Vec::new(),
        crc: None,
        num_unpack_substreams: 1,
    })
}

pub(crate) fn read_unpack_info<R: Read>(r: &mut R) -> Result<Vec<Folder>> {
    let mut id = [0u8; 1];
    r.read_exact(&mut id)?;
    if id[0] != K_FOLDER {
        return Err(Error::Malformed("expected kFolder in UnpackInfo".into()));
    }
    let num_folders = read_number_usize(r)?;
    let mut external = [0u8; 1];
    r.read_exact(&mut external)?;
    if external[0] != 0 {
        return Err(Error::Malformed(
            "external folder data streams are not supported".into(),
        ));
    }
    let mut folders: Vec<Folder> = (0..num_folders).map(|_| read_folder(r)).collect::<Result<_>>()?;

    r.read_exact(&mut id)?;
    if id[0] != K_CODERS_UNPACK_SIZE {
        return Err(Error::Malformed("expected kCodersUnpackSize".into()));
    }
    for folder in folders.iter_mut() {
        let total_out: usize = folder.coders.iter().map(|c| c.num_out_streams).sum();
        folder.unpack_sizes = (0..total_out)
            .map(|_| read_number(r))
            .collect::<Result<Vec<_>>>()?;
    }

    loop {
        r.read_exact(&mut id)?;
        match id[0] {
            K_END => break,
            K_CRC => {
                let crcs = read_digests(r, num_folders)?;
                for (f, crc) in folders.iter_mut().zip(crcs) {
                    f.crc = crc;
                }
            }
            other => {
                return Err(Error::Malformed(format!(
                    "unexpected id {other:#x} in UnpackInfo"
                )))
            }
        }
    }

    Ok(folders)
}

struct SubStreamsInfo {
    /// Per-folder count of unpack streams (files sharing that folder).
    nums_per_folder: Vec<usize>,
    /// Sizes of all-but-the-last substream in each folder (last is implied).
    sizes: Vec<u64>,
    /// CRCs for substreams whose CRC isn't already known from the folder CRC
    /// (i.e. every substream except a lone (count==1) substream, which
    /// reuses the folder's own CRC).
    crcs: Vec<Option<u32>>,
}

fn read_substreams_info<R: Read>(r: &mut R, folders: &[Folder]) -> Result<SubStreamsInfo> {
    let mut nums_per_folder = vec![1usize; folders.len()];
    let mut id = [0u8; 1];
    r.read_exact(&mut id)?;

    if id[0] == K_NUM_UNPACK_STREAM {
        nums_per_folder = (0..folders.len())
            .map(|_| read_number_usize(r))
            .collect::<Result<Vec<_>>>()?;
        r.read_exact(&mut id)?;
    }

    let mut sizes = Vec::new();
    if id[0] == K_SIZE {
        for (fi, &n) in nums_per_folder.iter().enumerate() {
            if n == 0 {
                continue;
            }
            let mut sum = 0u64;
            for _ in 0..(n - 1) {
                let s = read_number(r)?;
                sizes.push(s);
                sum += s;
            }
            // Last substream size = folder total - sum of the others.
            let folder_total = folders[fi].unpack_size()?;
            sizes.push(folder_total.saturating_sub(sum));
        }
        r.read_exact(&mut id)?;
    } else {
        // No explicit sizes: every folder with exactly one substream uses
        // the folder's own total size; folders with >1 (but no kSize) would
        // be malformed, but real archives never emit that combination.
        for (fi, &n) in nums_per_folder.iter().enumerate() {
            if n == 1 {
                sizes.push(folders[fi].unpack_size()?);
            } else if n > 1 {
                return Err(Error::Malformed(
                    "SubStreamsInfo: multiple substreams without kSize".into(),
                ));
            }
        }
    }

    // Number of substreams whose CRC is *not* already implied by a
    // single-substream folder's own folder-level CRC.
    let num_digests_needed: usize = nums_per_folder
        .iter()
        .zip(folders.iter())
        .map(|(&n, f)| if n == 1 && f.crc.is_some() { 0 } else { n })
        .sum();

    let mut crcs = vec![None; sizes.len()];
    if id[0] == K_CRC {
        let digests = read_digests(r, num_digests_needed)?;
        let mut di = 0usize;
        let mut si = 0usize;
        for (&n, f) in nums_per_folder.iter().zip(folders.iter()) {
            if n == 1 && f.crc.is_some() {
                crcs[si] = f.crc;
                si += 1;
            } else {
                for _ in 0..n {
                    crcs[si] = digests[di];
                    di += 1;
                    si += 1;
                }
            }
        }
        r.read_exact(&mut id)?;
    } else {
        // Fill in folder-level CRC for single-substream folders even
        // without an explicit kCRC block.
        let mut si = 0usize;
        for (&n, f) in nums_per_folder.iter().zip(folders.iter()) {
            if n == 1 {
                crcs[si] = f.crc;
            }
            si += n;
        }
    }

    while id[0] != K_END {
        return Err(Error::Malformed(format!(
            "unexpected id {:#x} in SubStreamsInfo",
            id[0]
        )));
    }

    Ok(SubStreamsInfo {
        nums_per_folder,
        sizes,
        crcs,
    })
}

struct StreamsInfo {
    pack_info: PackInfo,
    folders: Vec<Folder>,
}

fn read_streams_info<R: Read>(r: &mut R) -> Result<(StreamsInfo, Option<SubStreamsInfo>)> {
    let mut pack_info = PackInfo::default();
    let mut folders: Vec<Folder> = Vec::new();
    let mut substreams = None;

    let mut id = [0u8; 1];
    r.read_exact(&mut id)?;
    if id[0] == K_PACK_INFO {
        pack_info = read_pack_info(r)?;
        r.read_exact(&mut id)?;
    }
    if id[0] == K_UNPACK_INFO {
        folders = read_unpack_info(r)?;
        r.read_exact(&mut id)?;
    }
    if id[0] == K_SUBSTREAMS_INFO {
        let ss = read_substreams_info(r, &folders)?;
        for (f, &n) in folders.iter_mut().zip(ss.nums_per_folder.iter()) {
            f.num_unpack_substreams = n;
        }
        substreams = Some(ss);
        r.read_exact(&mut id)?;
    } else {
        // Default: exactly one substream per folder, whole-folder size/CRC.
        let nums_per_folder = vec![1usize; folders.len()];
        let mut sizes = Vec::with_capacity(folders.len());
        let mut crcs = Vec::with_capacity(folders.len());
        for f in &folders {
            sizes.push(f.unpack_size()?);
            crcs.push(f.crc);
        }
        if !folders.is_empty() {
            substreams = Some(SubStreamsInfo {
                nums_per_folder,
                sizes,
                crcs,
            });
        }
    }

    if id[0] != K_END {
        return Err(Error::Malformed(format!(
            "unexpected id {:#x} in StreamsInfo",
            id[0]
        )));
    }

    Ok((StreamsInfo { pack_info, folders }, substreams))
}

fn filetime_from_bytes(buf: [u8; 8]) -> u64 {
    u64::from_le_bytes(buf)
}

fn read_files_info<R: Read>(r: &mut R) -> Result<Vec<FileEntry>> {
    let num_files = read_number_usize(r)?;
    let mut files: Vec<FileEntry> = (0..num_files).map(|_| FileEntry::default()).collect();
    for f in files.iter_mut() {
        f.has_stream = true;
    }

    let mut empty_stream: Vec<bool> = vec![false; num_files];

    loop {
        let mut id = [0u8; 1];
        if r.read(&mut id)? == 0 {
            return Err(Error::Malformed("truncated FilesInfo".into()));
        }
        if id[0] == K_END {
            break;
        }

        let size = read_number_usize(r)?;
        let mut prop = vec![0u8; size];
        r.read_exact(&mut prop)?;
        let mut pr = std::io::Cursor::new(prop);

        match id[0] {
            K_EMPTY_STREAM => {
                empty_stream = read_bit_vector(&mut pr, num_files)?;
                for (f, es) in files.iter_mut().zip(empty_stream.iter()) {
                    f.has_stream = !es;
                }
            }
            K_EMPTY_FILE => {
                let num_empty = empty_stream.iter().filter(|b| **b).count();
                let flags = read_bit_vector(&mut pr, num_empty)?;
                let mut fi = flags.into_iter();
                for (f, es) in files.iter_mut().zip(empty_stream.iter()) {
                    if *es {
                        f.is_empty_file = fi.next().unwrap_or(false);
                    }
                }
            }
            K_ANTI => {
                let num_empty = empty_stream.iter().filter(|b| **b).count();
                let flags = read_bit_vector(&mut pr, num_empty)?;
                let mut fi = flags.into_iter();
                for (f, es) in files.iter_mut().zip(empty_stream.iter()) {
                    if *es {
                        f.is_anti = fi.next().unwrap_or(false);
                    }
                }
            }
            K_NAME => {
                let mut ext = [0u8; 1];
                pr.read_exact(&mut ext)?;
                if ext[0] != 0 {
                    return Err(Error::Malformed(
                        "external name data streams are not supported".into(),
                    ));
                }
                // Remaining bytes are UTF-16LE, NUL-terminated per name.
                let rest = pr.into_inner();
                let body = &rest[1..];
                let mut units: Vec<u16> = Vec::with_capacity(body.len() / 2);
                let mut i = 0usize;
                while i + 1 < body.len() {
                    units.push(u16::from_le_bytes([body[i], body[i + 1]]));
                    i += 2;
                }
                let mut idx = 0usize;
                let mut cur: Vec<u16> = Vec::new();
                for &u in &units {
                    if u == 0 {
                        if idx < files.len() {
                            files[idx].name = String::from_utf16_lossy(&cur);
                        }
                        cur.clear();
                        idx += 1;
                    } else {
                        cur.push(u);
                    }
                }
            }
            K_WIN_ATTRIBUTES => {
                let defined = read_optional_bit_vector(&mut pr, num_files)?;
                let mut ext = [0u8; 1];
                pr.read_exact(&mut ext)?;
                if ext[0] != 0 {
                    return Err(Error::Malformed(
                        "external attribute streams are not supported".into(),
                    ));
                }
                for (f, is_def) in files.iter_mut().zip(defined.iter()) {
                    if *is_def {
                        f.attributes = Some(read_u32_le(&mut pr)?);
                    }
                }
            }
            K_MTIME | K_CTIME | K_ATIME => {
                let defined = read_optional_bit_vector(&mut pr, num_files)?;
                let mut ext = [0u8; 1];
                pr.read_exact(&mut ext)?;
                if ext[0] != 0 {
                    return Err(Error::Malformed(
                        "external time streams are not supported".into(),
                    ));
                }
                for (f, is_def) in files.iter_mut().zip(defined.iter()) {
                    if *is_def {
                        let mut buf = [0u8; 8];
                        pr.read_exact(&mut buf)?;
                        let ft = filetime_from_bytes(buf);
                        match id[0] {
                            K_MTIME => f.mtime = Some(ft),
                            K_CTIME => f.ctime = Some(ft),
                            K_ATIME => f.atime = Some(ft),
                            _ => unreachable!(),
                        }
                    }
                }
            }
            _ => {
                // kDummy, kStartPos, kComment, and anything else we don't
                // need for extraction: already fully consumed via `size`.
            }
        }
    }

    Ok(files)
}

/// Parse the fully-decoded `Header` stream (the bytes after resolving any
/// `kEncodedHeader` compression) into a [`Header`].
pub fn read_header<R: Read>(r: &mut R) -> Result<Header> {
    let mut id = [0u8; 1];
    r.read_exact(&mut id)?;
    if id[0] != K_HEADER {
        return Err(Error::Malformed("expected kHeader".into()));
    }

    let mut pack_info = PackInfo::default();
    let mut folders: Vec<Folder> = Vec::new();
    let mut substreams: Option<SubStreamsInfo> = None;
    let mut files: Vec<FileEntry> = Vec::new();

    r.read_exact(&mut id)?;
    if id[0] == K_ARCHIVE_PROPERTIES {
        // Skip: repeated (PropertyType:NUMBER, size:NUMBER, data) until kEnd.
        loop {
            let mut t = [0u8; 1];
            r.read_exact(&mut t)?;
            if t[0] == K_END {
                break;
            }
            let size = read_number_usize(r)?;
            let mut buf = vec![0u8; size];
            r.read_exact(&mut buf)?;
        }
        r.read_exact(&mut id)?;
    }

    if id[0] == K_ADDITIONAL_STREAMS_INFO {
        return Err(Error::Malformed(
            "AdditionalStreamsInfo (external header data) is not supported".into(),
        ));
    }

    if id[0] == K_MAIN_STREAMS_INFO {
        let (si, ss) = read_streams_info(r)?;
        pack_info = si.pack_info;
        folders = si.folders;
        substreams = ss;
        r.read_exact(&mut id)?;
    }

    if id[0] == K_FILES_INFO {
        files = read_files_info(r)?;
        r.read_exact(&mut id)?;
    }

    if id[0] != K_END {
        return Err(Error::Malformed(format!(
            "unexpected trailing id {:#x} in Header",
            id[0]
        )));
    }

    // Resolve each file's (folder_index, offset_in_folder, size, crc) from
    // the folder/substream layout, in file order, skipping empty-stream files.
    let ss = substreams.unwrap_or(SubStreamsInfo {
        nums_per_folder: vec![],
        sizes: vec![],
        crcs: vec![],
    });
    let mut sub_iter = 0usize;
    let mut folder_for_file: Vec<usize> = Vec::new();
    for (fi, &n) in ss.nums_per_folder.iter().enumerate() {
        for _ in 0..n {
            folder_for_file.push(fi);
        }
    }

    let mut stream_file_indices: Vec<usize> = Vec::new();
    for (idx, f) in files.iter().enumerate() {
        if f.has_stream {
            stream_file_indices.push(idx);
        }
    }

    // Running per-folder offset accumulator: substreams are laid out
    // consecutively within each folder's decoded stream, in the same order
    // they appear in `sizes`/`crcs` (which is itself folder-major).
    let mut offset_in_current_folder = 0u64;
    let mut prev_folder_idx: Option<usize> = None;

    for (rank, &file_idx) in stream_file_indices.iter().enumerate() {
        let folder_idx = *folder_for_file.get(rank).ok_or_else(|| {
            Error::Malformed("more streamed files than substreams".into())
        })?;
        let size = *ss.sizes.get(sub_iter).ok_or_else(|| {
            Error::Malformed("missing substream size".into())
        })?;
        let crc = ss.crcs.get(sub_iter).copied().flatten();

        if prev_folder_idx != Some(folder_idx) {
            offset_in_current_folder = 0;
            prev_folder_idx = Some(folder_idx);
        }

        files[file_idx].folder_index = Some(folder_idx);
        files[file_idx].offset_in_folder = offset_in_current_folder;
        files[file_idx].size = size;
        files[file_idx].crc = crc;

        offset_in_current_folder += size;
        sub_iter += 1;
    }

    Ok(Header {
        pack_info,
        folders,
        files,
    })
}
