//! Synthetic-fixture tests for the GNRL reader's version-aware header
//! layout and codec dispatch (zlib for v1/v2/v7/v8; v3 selects its codec
//! from a real per-archive `compression_method` field) plus the
//! uncompressed (`packedSize == 0`) path.
//!
//! The v2/v3 header-extension handling here was added after oracle
//! cross-validation (see `tests/oracle_cross_validation.rs`) surfaced
//! that this crate previously read/wrote only the base 24-byte header
//! for every version -- see `format::header_size_for_version`'s doc
//! comment for the full writeup and independent sources.

use modfather_ba2::Ba2Archive;
use std::io::{Cursor, Write};

/// Build a minimal single-file GNRL archive by hand, matching the
/// `F4BSAHeader` + `F4GeneralInfo` + name-table layout documented in
/// `crates/modfather-ba2/src/format.rs`, including the version-dependent
/// header extension (v2: +8 reserved bytes; v3: +8 reserved bytes + a
/// real `compression_method: u32`).
fn build_gnrl_archive(
    version: u32,
    compression_method: Option<u32>,
    packed: &[u8],
    packed_size: u32,
    unpacked_size: u32,
    name: &str,
) -> Vec<u8> {
    let mut buf = Vec::new();

    // Base header (24 bytes).
    buf.extend_from_slice(b"BTDX");
    buf.extend_from_slice(&version.to_le_bytes());
    buf.extend_from_slice(b"GNRL");
    buf.extend_from_slice(&1u32.to_le_bytes()); // numFiles
    let name_table_offset_pos = buf.len();
    buf.extend_from_slice(&0u64.to_le_bytes()); // nameTableOffset, patched below

    // Version-dependent header extension.
    if version == 2 {
        buf.extend_from_slice(&0u64.to_le_bytes()); // 8 reserved bytes
    } else if version == 3 {
        buf.extend_from_slice(&0u64.to_le_bytes()); // 8 reserved bytes
        buf.extend_from_slice(&compression_method.unwrap_or(0).to_le_bytes());
    }

    // One F4GeneralInfo record (36 bytes).
    let record_start = buf.len();
    buf.extend_from_slice(&0u32.to_le_bytes()); // nameHash
    buf.extend_from_slice(b"txt\0"); // ext
    buf.extend_from_slice(&0u32.to_le_bytes()); // dirHash
    buf.extend_from_slice(&0u32.to_le_bytes()); // unk0C
    let offset_pos = buf.len();
    buf.extend_from_slice(&0u64.to_le_bytes()); // offset, patched below
    buf.extend_from_slice(&packed_size.to_le_bytes());
    buf.extend_from_slice(&unpacked_size.to_le_bytes());
    buf.extend_from_slice(&0xBAAD_F00Du32.to_le_bytes());
    assert_eq!(buf.len() - record_start, 36);

    // Payload.
    let payload_offset = buf.len() as u64;
    buf.write_all(packed).unwrap();

    // Name table.
    let name_table_offset = buf.len() as u64;
    buf.extend_from_slice(&(name.len() as u16).to_le_bytes());
    buf.extend_from_slice(name.as_bytes());

    // Patch offsets.
    buf[name_table_offset_pos..name_table_offset_pos + 8]
        .copy_from_slice(&name_table_offset.to_le_bytes());
    buf[offset_pos..offset_pos + 8].copy_from_slice(&payload_offset.to_le_bytes());

    buf
}

#[test]
fn v1_zlib_payload_decodes() {
    let plain = b"Hello from a v1 (zlib) GNRL entry, repeated for compressibility. ".repeat(10);
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&plain).unwrap();
    let packed = enc.finish().unwrap();

    let archive_bytes =
        build_gnrl_archive(1, None, &packed, packed.len() as u32, plain.len() as u32, "hello.txt");
    let mut archive = Ba2Archive::open(Cursor::new(archive_bytes)).unwrap();

    assert_eq!(archive.version(), 1);
    assert_eq!(archive.entries().len(), 1);
    assert_eq!(archive.entries()[0].name, "hello.txt");

    let out = archive.read_file(0).unwrap();
    assert_eq!(out, plain);
}

#[test]
fn v2_is_always_zlib_and_has_an_8_byte_header_extension() {
    // v2 (Starfield GNRL) has no compression_method field and is always
    // zlib. This also proves the reader correctly skips the 8-byte v2
    // header extension before reading file records (getting this offset
    // wrong would corrupt every field after it).
    let plain =
        b"Hello from a v2 (Starfield, zlib) GNRL entry, repeated for compressibility. ".repeat(10);
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&plain).unwrap();
    let packed = enc.finish().unwrap();

    let archive_bytes =
        build_gnrl_archive(2, None, &packed, packed.len() as u32, plain.len() as u32, "hello2.txt");
    let mut archive = Ba2Archive::open(Cursor::new(archive_bytes)).unwrap();

    assert_eq!(archive.version(), 2);
    assert_eq!(archive.entries()[0].name, "hello2.txt");
    let out = archive.read_file(0).unwrap();
    assert_eq!(out, plain);
}

#[test]
fn v3_compression_method_zero_means_zlib() {
    let plain = b"Hello from a v3 (Starfield) entry using compression_method=0 (zlib). "
        .repeat(10);
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&plain).unwrap();
    let packed = enc.finish().unwrap();

    let archive_bytes = build_gnrl_archive(
        3,
        Some(0),
        &packed,
        packed.len() as u32,
        plain.len() as u32,
        "hello3-zlib.txt",
    );
    let mut archive = Ba2Archive::open(Cursor::new(archive_bytes)).unwrap();
    assert_eq!(archive.version(), 3);
    let out = archive.read_file(0).unwrap();
    assert_eq!(out, plain);
}

#[test]
fn v3_compression_method_three_means_lz4_block() {
    // This is the field this crate previously ignored entirely (it
    // guessed LZ4 for every v3 archive from the version number alone).
    // A real v3 archive with compression_method == 0 would have been
    // silently misdecoded as LZ4 under the old logic; this test pins the
    // fix by using method == 3 explicitly, matching the ByroRedux-
    // documented "Starfield | BA2 BTDX v3 DX10 | zlib + LZ4 block" case.
    let plain = b"Hello from a v3 (Starfield) entry using compression_method=3 (LZ4 block). "
        .repeat(10);
    let packed = lz4_flex::block::compress(&plain);

    let archive_bytes = build_gnrl_archive(
        3,
        Some(3),
        &packed,
        packed.len() as u32,
        plain.len() as u32,
        "hello3-lz4.txt",
    );
    let mut archive = Ba2Archive::open(Cursor::new(archive_bytes)).unwrap();
    assert_eq!(archive.version(), 3);
    let out = archive.read_file(0).unwrap();
    assert_eq!(out, plain);

    // Cross-check: decoding this same payload as zlib (what the old,
    // version-only codec guess would still get right by accident for
    // v3-forced-LZ4, but what a *method==0* v3 archive misread as LZ4
    // would get wrong) must not silently succeed with the right bytes.
    let mut bad = flate2::read::ZlibDecoder::new(&packed[..]);
    let mut bad_out = Vec::new();
    use std::io::Read;
    let bad_result = bad.read_to_end(&mut bad_out);
    assert!(
        bad_result.is_err() || bad_out != plain,
        "zlib must not successfully decode an LZ4 payload the same way the reference bug did"
    );
}

#[test]
fn uncompressed_payload_is_read_verbatim() {
    let plain = b"stored with packedSize == 0, no codec applied".to_vec();
    let archive_bytes =
        build_gnrl_archive(1, None, &plain, 0, plain.len() as u32, "raw.bin");
    let mut archive = Ba2Archive::open(Cursor::new(archive_bytes)).unwrap();

    let out = archive.read_file(0).unwrap();
    assert_eq!(out, plain);
}
