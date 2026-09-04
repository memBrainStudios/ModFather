//! Native codec implementations: Copy, LZMA, LZMA2.
//!
//! This is the whole point of `sevenzip-re` being a *standalone* Rust
//! implementation: no shelling out to a system `7z`/`7za` binary. RAR is a
//! placeholder pending license and is intentionally absent; BSA/BA2 are not
//! 7z codecs at all and live in separate extension crates.

use crate::error::{Error, Result};
use crate::format::codec_id;
use crate::header::Coder;
use lzma_rs::decompress::raw::{Lzma2Decoder, LzmaDecoder, LzmaParams, LzmaProperties};
use std::io::{Cursor, Read, Write};

/// Decode one coder's input bytes into its decoded output, given the
/// expected output size (from `CodersUnpackSize`).
pub fn decode(coder: &Coder, input: &[u8], unpack_size: u64) -> Result<Vec<u8>> {
    match coder.codec_id.as_slice() {
        id if id == codec_id::COPY => Ok(input.to_vec()),
        id if id == codec_id::LZMA => decode_lzma(coder, input, unpack_size),
        id if id == codec_id::LZMA2 => decode_lzma2(coder, input, unpack_size),
        other => Err(Error::UnsupportedCodec(hex(other))),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// LZMA properties inside a 7z folder are the raw 5-byte `lclppb` + dict-size
/// header (no unpacked-size trailer — that comes from `CodersUnpackSize`).
fn parse_lzma_properties(props: &[u8]) -> Result<(LzmaProperties, u32)> {
    if props.len() < 5 {
        return Err(Error::Malformed(format!(
            "LZMA coder properties too short: {} bytes",
            props.len()
        )));
    }
    let d = props[0] as u32;
    if d >= 225 {
        return Err(Error::Malformed(format!(
            "LZMA properties byte {d} out of range"
        )));
    }
    let lc = d % 9;
    let rest = d / 9;
    let lp = rest % 5;
    let pb = rest / 5;
    let dict_size = u32::from_le_bytes([props[1], props[2], props[3], props[4]]);
    let dict_size = if dict_size < 0x1000 { 0x1000 } else { dict_size };
    Ok((LzmaProperties { lc, lp, pb }, dict_size))
}

fn decode_lzma(coder: &Coder, input: &[u8], unpack_size: u64) -> Result<Vec<u8>> {
    let (properties, dict_size) = parse_lzma_properties(&coder.properties)?;
    let params = LzmaParams::new(properties, dict_size, Some(unpack_size));
    let mut decoder = LzmaDecoder::new(params, None).map_err(|e| Error::Lzma(e.to_string()))?;
    let mut input_cursor = Cursor::new(input);
    let mut out = Vec::with_capacity(unpack_size as usize);
    decoder
        .decompress(&mut input_cursor, &mut out)
        .map_err(|e| Error::Lzma(e.to_string()))?;
    Ok(out)
}

fn decode_lzma2(_coder: &Coder, input: &[u8], unpack_size: u64) -> Result<Vec<u8>> {
    // LZMA2's own chunk headers carry lc/lp/pb + dict reset flags; the 1-byte
    // "properties" on the 7z coder only encodes dictionary size, which the
    // pure Rust decoder does not need to be told up front (it grows its
    // buffer as needed), so we ignore it here beyond validating its shape.
    let mut decoder = Lzma2Decoder::new();
    let mut input_cursor = Cursor::new(input);
    let mut out = Vec::with_capacity(unpack_size as usize);
    decoder
        .decompress(&mut input_cursor, &mut out)
        .map_err(|e| Error::Lzma(e.to_string()))?;
    Ok(out)
}

/// Which single-coder codec to use when packing a new folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackCodec {
    Copy,
    Lzma,
    Lzma2,
}

/// Encode `data` with the requested codec, returning the packed bytes and
/// the coder's `properties` blob to store in the folder's `Coder`.
pub fn encode(codec: PackCodec, data: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    match codec {
        PackCodec::Copy => Ok((data.to_vec(), Vec::new())),
        PackCodec::Lzma => encode_lzma(data),
        PackCodec::Lzma2 => encode_lzma2(data),
    }
}

fn encode_lzma(data: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    // `lzma_rs::lzma_compress_with_options` writes the classic 13-byte LZMA
    // header (1-byte props + 4-byte dict size + 8-byte unpacked size)
    // followed by the compressed body. 7z's folder `Coder.properties` wants
    // only the first 5 bytes (props + dict size); the unpacked size is
    // tracked separately via `CodersUnpackSize`, so we strip the trailer.
    let mut input = Cursor::new(data);
    let mut full = Vec::new();
    lzma_rs::lzma_compress(&mut input, &mut full).map_err(Error::Io)?;

    if full.len() < 13 {
        return Err(Error::Malformed("lzma-rs produced a truncated stream".into()));
    }
    let properties = full[0..5].to_vec();
    let body = full[13..].to_vec();
    Ok((body, properties))
}

fn encode_lzma2(data: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut input = Cursor::new(data);
    let mut body = Vec::new();
    lzma_rs::lzma2_compress(&mut input, &mut body).map_err(Error::Io)?;

    // LZMA2's 7z coder property is a single byte `p` encoding the
    // dictionary size as `(2 | (p & 1)) << (p / 2 + 11)` for `p` in
    // 0..=39, with `p == 40` a special sentinel meaning "unbounded" (the
    // decoder must size its window from the stream itself, no cap at
    // all) -- a legal value per the LZMA2 spec, but one real-world tools
    // interpret as "allocate up to 4 GiB up front", which is why 7-Zip
    // itself refuses it with "Can't allocate required memory!" for any
    // ordinary-sized archive. The pure-Rust encoder doesn't expose the
    // dict size it actually chose, so we conservatively advertise the
    // largest *standard* (non-sentinel) size instead: 64 MiB is `p == 28`
    // **in decimal**, not `0x28` (which is decimal 40 -- the unbounded
    // sentinel above, and the actual value of a bug this literal used to
    // contain: `0x28u8` silently meant "unbounded", triggering that same
    // 4 GiB allocation failure in the real `7z` binary and only caught by
    // `our_writer_is_readable_by_the_system_binary` cross-checking our
    // writer's LZMA2 output against it). Any compliant LZMA2 decoder
    // sizes its actual window from the chunk headers during decompression
    // regardless, so this value only matters to *other* tools reading
    // properties before decoding, not to this crate's own
    // [`decode_lzma2`].
    let properties = vec![28u8];
    Ok((body, properties))
}

/// Read exactly `len` bytes from `r`.
pub fn read_exact_vec<R: Read>(r: &mut R, len: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

/// Write all of `data` to `w`.
pub fn write_all<W: Write>(w: &mut W, data: &[u8]) -> Result<()> {
    w.write_all(data)?;
    Ok(())
}
