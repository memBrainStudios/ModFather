//! BA2 GNRL/DX10 reader with version-aware codec dispatch.
//!
//! Redesigned from the reference implementation's two known gaps:
//! 1. It used `flate2::ZlibDecoder` unconditionally for every payload,
//!    which is wrong for Starfield-era (v2/v3) archives that use LZ4.
//! 2. Its writer only ever emitted uncompressed GNRL archives.
//!    (The writer side is tracked as follow-up work in this crate; see
//!    `docs/SCHEDULE.md`.)
//!
//! Also fixes a third, later-discovered gap (via oracle cross-validation,
//! see `tests/oracle_cross_validation.rs` and `format::header_size_for_version`):
//! v2/v3 (Starfield) archives have a header extension beyond the base
//! 24 bytes, and v3's payload codec is a real per-archive
//! `compression_method` field, not implied by the version number alone.

use crate::error::{Error, Result};
use crate::format::*;
use std::io::{Read, Seek, SeekFrom};

/// One DX10 mip-chunk: an independently (de)compressible slice of a
/// texture's mip range, enabling streaming.
#[derive(Debug, Clone)]
pub struct TexChunk {
    pub offset: u64,
    pub packed_size: u32,
    pub unpacked_size: u32,
    pub start_mip: u16,
    pub end_mip: u16,
}

/// Which archive sub-format a file entry came from.
#[derive(Debug, Clone)]
pub enum EntryKind {
    /// General-purpose (non-texture) file: a single packed/unpacked blob.
    General {
        offset: u64,
        packed_size: u32,
        unpacked_size: u32,
    },
    /// DirectX texture: header metadata plus one or more mip chunks.
    Texture {
        height: u16,
        width: u16,
        num_mips: u8,
        format: u8,
        chunks: Vec<TexChunk>,
    },
}

/// One file entry as seen from the public API.
#[derive(Debug, Clone)]
pub struct Ba2Entry {
    pub name: String,
    pub kind: EntryKind,
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

fn read_u16_le<R: Read>(r: &mut R) -> Result<u16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b)?;
    Ok(u16::from_le_bytes(b))
}

fn read_u8<R: Read>(r: &mut R) -> Result<u8> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b)?;
    Ok(b[0])
}

/// An opened BA2 archive.
pub struct Ba2Archive<R> {
    reader: R,
    version: u32,
    codec: PayloadCodec,
    entries: Vec<Ba2Entry>,
}

impl<R: Read + Seek> Ba2Archive<R> {
    /// Parse a BA2 archive's header and file-record table (GNRL or DX10).
    /// Actual payload bytes are decoded lazily by [`Ba2Archive::read_file`].
    pub fn open(mut reader: R) -> Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if magic != MAGIC {
            return Err(Error::BadSignature);
        }

        let version = read_u32_le(&mut reader)?;

        let mut type_tag = [0u8; 4];
        reader.read_exact(&mut type_tag)?;

        let num_files = read_u32_le(&mut reader)? as usize;
        let name_table_offset = read_u64_le(&mut reader)?;

        // Starfield (v2/v3) header extension -- see the doc comment on
        // `format::header_size_for_version` for why this exists and how
        // it was discovered (oracle cross-validation against the `ba2`
        // crate, corroborated by ByroRedux's format notes).
        let codec = if version == version::V2 {
            // v2: 8 reserved bytes, no compression-method field. Always zlib.
            let mut reserved = [0u8; 8];
            reader.read_exact(&mut reserved)?;
            PayloadCodec::Zlib
        } else if version == version::V3 {
            // v3: 8 reserved bytes + a real compression_method: u32.
            let mut reserved = [0u8; 8];
            reader.read_exact(&mut reserved)?;
            let method = read_u32_le(&mut reader)?;
            codec_for_compression_method(method)
        } else {
            default_codec_for_version(version)
        };

        let entries = if type_tag == TYPE_GNRL {
            Self::read_gnrl_entries(&mut reader, num_files)?
        } else if type_tag == TYPE_DX10 {
            Self::read_dx10_entries(&mut reader, num_files)?
        } else {
            return Err(Error::UnsupportedType(
                String::from_utf8_lossy(&type_tag).into_owned(),
            ));
        };

        let names = Self::read_name_table(&mut reader, name_table_offset, num_files)?;

        let entries = entries
            .into_iter()
            .zip(names)
            .map(|(kind, name)| Ba2Entry { name, kind })
            .collect();

        Ok(Ba2Archive {
            reader,
            version,
            codec,
            entries,
        })
    }

    fn read_gnrl_entries(reader: &mut R, num_files: usize) -> Result<Vec<EntryKind>> {
        let mut out = Vec::with_capacity(num_files);
        for _ in 0..num_files {
            // F4GeneralInfo, 36 bytes:
            //   nameHash(4) ext(4) dirHash(4) unk0C(4) offset(8) packedSize(4) unpackedSize(4) unk20(4)
            let _name_hash = read_u32_le(reader)?;
            let mut ext = [0u8; 4];
            reader.read_exact(&mut ext)?;
            let _dir_hash = read_u32_le(reader)?;
            let _unk0c = read_u32_le(reader)?;
            let offset = read_u64_le(reader)?;
            let packed_size = read_u32_le(reader)?;
            let unpacked_size = read_u32_le(reader)?;
            let _sentinel = read_u32_le(reader)?; // expected 0xBAADF00D, not enforced
            out.push(EntryKind::General {
                offset,
                packed_size,
                unpacked_size,
            });
        }
        Ok(out)
    }

    fn read_dx10_entries(reader: &mut R, num_files: usize) -> Result<Vec<EntryKind>> {
        let mut out = Vec::with_capacity(num_files);
        for _ in 0..num_files {
            // F4TexInfo, 24 bytes:
            //   nameHash(4) ext(4) dirHash(4) unk0C(1) numChunks(1) chunkHeaderSize(2)
            //   height(2) width(2) numMips(1) format(1) unk16(2)
            let _name_hash = read_u32_le(reader)?;
            let mut ext = [0u8; 4];
            reader.read_exact(&mut ext)?;
            let _dir_hash = read_u32_le(reader)?;
            let _unk0c = read_u8(reader)?;
            let num_chunks = read_u8(reader)?;
            let _chunk_header_size = read_u16_le(reader)?;
            let height = read_u16_le(reader)?;
            let width = read_u16_le(reader)?;
            let num_mips = read_u8(reader)?;
            let format = read_u8(reader)?;
            let _unk16 = read_u16_le(reader)?;

            let mut chunks = Vec::with_capacity(num_chunks as usize);
            for _ in 0..num_chunks {
                // F4TexChunk, 24 bytes:
                //   offset(8) packedSize(4) unpackedSize(4) startMip(2) endMip(2) unk14(4)
                let offset = read_u64_le(reader)?;
                let packed_size = read_u32_le(reader)?;
                let unpacked_size = read_u32_le(reader)?;
                let start_mip = read_u16_le(reader)?;
                let end_mip = read_u16_le(reader)?;
                let _sentinel = read_u32_le(reader)?;
                chunks.push(TexChunk {
                    offset,
                    packed_size,
                    unpacked_size,
                    start_mip,
                    end_mip,
                });
            }

            out.push(EntryKind::Texture {
                height,
                width,
                num_mips,
                format,
                chunks,
            });
        }
        Ok(out)
    }

    fn read_name_table(reader: &mut R, offset: u64, num_files: usize) -> Result<Vec<String>> {
        reader.seek(SeekFrom::Start(offset))?;
        let mut names = Vec::with_capacity(num_files);
        for _ in 0..num_files {
            let len = read_u16_le(reader)?;
            let mut buf = vec![0u8; len as usize];
            reader.read_exact(&mut buf)?;
            names.push(String::from_utf8_lossy(&buf).into_owned());
        }
        Ok(names)
    }

    /// List every entry in the archive.
    pub fn entries(&self) -> &[Ba2Entry] {
        &self.entries
    }

    /// Which base payload codec this archive's version implies
    /// (see [`default_codec_for_version`]; a per-chunk override may still
    /// apply — see [`Ba2Archive::read_chunk`]).
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Read and decode a general-purpose (GNRL) file's bytes by index.
    pub fn read_file(&mut self, idx: usize) -> Result<Vec<u8>> {
        let entry = self
            .entries
            .get(idx)
            .ok_or_else(|| Error::NoSuchEntry(format!("index {idx}")))?
            .clone();

        match entry.kind {
            EntryKind::General {
                offset,
                packed_size,
                unpacked_size,
            } => self.read_payload(offset, packed_size, unpacked_size),
            EntryKind::Texture { .. } => Err(Error::Malformed(
                "read_file called on a DX10 texture entry; use read_chunk for mip streaming"
                    .into(),
            )),
        }
    }

    /// Read and decode one mip chunk of a DX10 texture entry.
    pub fn read_chunk(&mut self, idx: usize, chunk_index: usize) -> Result<Vec<u8>> {
        let entry = self
            .entries
            .get(idx)
            .ok_or_else(|| Error::NoSuchEntry(format!("index {idx}")))?
            .clone();

        let EntryKind::Texture { chunks, .. } = entry.kind else {
            return Err(Error::Malformed(
                "read_chunk called on a non-texture entry".into(),
            ));
        };
        let chunk = chunks
            .get(chunk_index)
            .ok_or_else(|| Error::NoSuchEntry(format!("chunk {chunk_index}")))?;

        self.read_payload(chunk.offset, chunk.packed_size, chunk.unpacked_size)
    }

    /// Decode one packed blob at `offset`. `packed_size == 0` means the
    /// payload is stored uncompressed (a real possibility per Archive2's
    /// "no compression" option), in which case `unpacked_size` bytes are
    /// read directly with no codec applied.
    fn read_payload(&mut self, offset: u64, packed_size: u32, unpacked_size: u32) -> Result<Vec<u8>> {
        self.reader.seek(SeekFrom::Start(offset))?;

        if packed_size == 0 {
            let mut buf = vec![0u8; unpacked_size as usize];
            self.reader.read_exact(&mut buf)?;
            return Ok(buf);
        }

        let mut packed = vec![0u8; packed_size as usize];
        self.reader.read_exact(&mut packed)?;

        match self.codec {
            PayloadCodec::Zlib => decode_zlib(&packed, unpacked_size as usize),
            PayloadCodec::Lz4 => decode_lz4(&packed, unpacked_size as usize),
        }
    }
}

fn decode_zlib(body: &[u8], unpacked_size: usize) -> Result<Vec<u8>> {
    use std::io::Read as _;
    let mut dec = flate2::read::ZlibDecoder::new(body);
    let mut out = Vec::with_capacity(unpacked_size);
    dec.read_to_end(&mut out)
        .map_err(|e| Error::Malformed(format!("zlib decompress error: {e}")))?;
    Ok(out)
}

fn decode_lz4(body: &[u8], unpacked_size: usize) -> Result<Vec<u8>> {
    lz4_flex::block::decompress(body, unpacked_size)
        .map_err(|e| Error::Malformed(format!("lz4 decompress error: {e}")))
}
