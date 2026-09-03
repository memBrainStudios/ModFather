//! Synthetic-fixture tests for the GNRL reader's version-aware codec
//! dispatch (zlib for v1, LZ4 for v2/v3) plus the uncompressed
//! (`packedSize == 0`) path.

use modfather_ba2::Ba2Archive;
use std::io::{Cursor, Write};

/// Build a minimal single-file GNRL archive by hand, matching the
/// `F4BSAHeader` + `F4GeneralInfo` + name-table layout documented in
/// `crates/modfather-ba2/src/format.rs`.
fn build_gnrl_archive(version: u32, packed: &[u8], packed_size: u32, unpacked_size: u32, name: &str) -> Vec<u8> {
    let mut buf = Vec::new();

    // Header (24 bytes).
    buf.extend_from_slice(b"BTDX");
    buf.extend_from_slice(&version.to_le_bytes());
    buf.extend_from_slice(b"GNRL");
    buf.extend_from_slice(&1u32.to_le_bytes()); // numFiles
    let name_table_offset_pos = buf.len();
    buf.extend_from_slice(&0u64.to_le_bytes()); // nameTableOffset, patched below

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

    let archive_bytes = build_gnrl_archive(1, &packed, packed.len() as u32, plain.len() as u32, "hello.txt");
    let mut archive = Ba2Archive::open(Cursor::new(archive_bytes)).unwrap();

    assert_eq!(archive.version(), 1);
    assert_eq!(archive.entries().len(), 1);
    assert_eq!(archive.entries()[0].name, "hello.txt");

    let out = archive.read_file(0).unwrap();
    assert_eq!(out, plain);
}

#[test]
fn v2_lz4_payload_decodes() {
    let plain = b"Hello from a v2 (LZ4, Starfield-era) GNRL entry, repeated for compressibility. ".repeat(10);
    let packed = lz4_flex::block::compress(&plain);

    let archive_bytes = build_gnrl_archive(2, &packed, packed.len() as u32, plain.len() as u32, "hello2.txt");
    let mut archive = Ba2Archive::open(Cursor::new(archive_bytes)).unwrap();

    assert_eq!(archive.version(), 2);
    let out = archive.read_file(0).unwrap();
    assert_eq!(out, plain);

    // Cross-check: decoding this same archive's payload as zlib (the
    // reference implementation's unconditional behavior) must NOT produce
    // the right bytes -- this is the regression this crate fixes.
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
    let archive_bytes = build_gnrl_archive(1, &plain, 0, plain.len() as u32, "raw.bin");
    let mut archive = Ba2Archive::open(Cursor::new(archive_bytes)).unwrap();

    let out = archive.read_file(0).unwrap();
    assert_eq!(out, plain);
}
