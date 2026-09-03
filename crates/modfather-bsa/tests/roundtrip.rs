//! Synthetic-fixture tests for `modfather-bsa`'s version-aware payload
//! codec dispatch (zlib for v103/v104, LZ4 for v105) plus the
//! uncompressed path and the v105-only folder-record padding fields.

use modfather_bsa::BsaArchive;
use std::io::{Cursor, Write};

/// Build a minimal single-folder, single-file BSA archive by hand,
/// matching the header/folder-record/file-record/name-table layout
/// documented in `crates/modfather-bsa/src/format.rs` and the UESP spec.
fn build_bsa_archive(
    version: u32,
    folder: &str,
    file_name: &str,
    is_compressed: bool,
    payload_on_disk: &[u8],
) -> Vec<u8> {
    use modfather_bsa::format::archive_flags;

    let mut buf = Vec::new();
    buf.extend_from_slice(b"BSA\0");
    buf.extend_from_slice(&version.to_le_bytes());
    buf.extend_from_slice(&36u32.to_le_bytes()); // offset
    let mut flags = archive_flags::INCLUDE_DIR_NAMES | archive_flags::INCLUDE_FILE_NAMES;
    if is_compressed {
        flags |= archive_flags::COMPRESSED_ARCHIVE;
    }
    buf.extend_from_slice(&flags.to_le_bytes());
    buf.extend_from_slice(&1u32.to_le_bytes()); // folderCount
    buf.extend_from_slice(&1u32.to_le_bytes()); // fileCount
    buf.extend_from_slice(&((folder.len() + 1) as u32).to_le_bytes()); // totalFolderNameLength
    buf.extend_from_slice(&((file_name.len() + 1) as u32).to_le_bytes()); // totalFileNameLength
    buf.extend_from_slice(&0u16.to_le_bytes()); // fileFlags
    buf.extend_from_slice(&0u16.to_le_bytes()); // padding
    assert_eq!(buf.len(), 36);

    // Folder record.
    buf.extend_from_slice(&0u64.to_le_bytes()); // nameHash (unused by reader)
    buf.extend_from_slice(&1u32.to_le_bytes()); // count
    if version >= 105 {
        buf.extend_from_slice(&0u32.to_le_bytes()); // pad
    }
    let folder_offset_pos = buf.len();
    buf.extend_from_slice(&0u32.to_le_bytes()); // offset, unused by this reader
    if version >= 105 {
        buf.extend_from_slice(&0u32.to_le_bytes()); // pad
    }
    let _ = folder_offset_pos;

    // File-record block: folder name (bzstring) + one file record.
    let folder_bztring_len = folder.len() + 1; // includes trailing NUL, per bzstring
    buf.push(folder_bztring_len as u8);
    buf.extend_from_slice(folder.as_bytes());
    buf.push(0); // NUL terminator

    buf.extend_from_slice(&0u64.to_le_bytes()); // nameHash

    // Compose the on-disk data block first so we can compute placement.
    let data_block: Vec<u8> = payload_on_disk.to_vec();

    let size_field_pos = buf.len();
    buf.extend_from_slice(&0u32.to_le_bytes()); // size, patched below
    let offset_field_pos = buf.len();
    buf.extend_from_slice(&0u32.to_le_bytes()); // offset, patched below

    // File-names block.
    buf.extend_from_slice(file_name.as_bytes());
    buf.push(0);

    // Data block starts here.
    let data_offset = buf.len() as u32;
    buf.write_all(&data_block).unwrap();

    buf[size_field_pos..size_field_pos + 4]
        .copy_from_slice(&(data_block.len() as u32).to_le_bytes());
    buf[offset_field_pos..offset_field_pos + 4].copy_from_slice(&data_offset.to_le_bytes());

    buf
}

#[test]
fn v104_zlib_payload_decodes() {
    let plain = b"Hello from a v104 (zlib) BSA entry, repeated for compressibility. ".repeat(10);
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&plain).unwrap();
    let compressed = enc.finish().unwrap();

    let mut on_disk = Vec::new();
    on_disk.extend_from_slice(&(plain.len() as u32).to_le_bytes());
    on_disk.extend_from_slice(&compressed);

    let archive_bytes = build_bsa_archive(104, "meshes\\test", "cube.nif", true, &on_disk);
    let mut archive = BsaArchive::open(Cursor::new(archive_bytes)).unwrap();

    let entries = archive.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].folder, "meshes\\test");
    assert_eq!(entries[0].name, "cube.nif");

    let out = archive.read_file(0).unwrap();
    assert_eq!(out, plain);
}

#[test]
fn v105_lz4_payload_decodes() {
    // v105 (Skyrim SE/AE) uses the LZ4 **frame** format, not raw LZ4 blocks
    // -- confirmed against an independent oracle (see
    // `src/reader.rs::decode_lz4`'s doc comment for the cross-check
    // details). This fixture must therefore be built with `FrameEncoder`,
    // matching what the reader now expects.
    let plain = b"Hello from a v105 (Skyrim SE/AE, LZ4) BSA entry, repeated. ".repeat(10);
    let compressed = {
        use lz4_flex::frame::FrameEncoder;
        let mut enc = FrameEncoder::new(Vec::new());
        enc.write_all(&plain).unwrap();
        enc.finish().unwrap()
    };

    let mut on_disk = Vec::new();
    on_disk.extend_from_slice(&(plain.len() as u32).to_le_bytes());
    on_disk.extend_from_slice(&compressed);

    let archive_bytes = build_bsa_archive(105, "textures\\test", "brick.dds", true, &on_disk);
    let mut archive = BsaArchive::open(Cursor::new(archive_bytes)).unwrap();

    let out = archive.read_file(0).unwrap();
    assert_eq!(out, plain);
}

#[test]
fn v105_lz4_payload_would_fail_as_zlib() {
    // This is the exact regression class the reference implementation had:
    // it used `ZlibDecoder` unconditionally, which either errors out or
    // (worse) silently produces wrong bytes on a real v105 (LZ4) archive.
    let plain = b"regression guard content".repeat(20);
    let compressed = {
        use lz4_flex::frame::FrameEncoder;
        let mut enc = FrameEncoder::new(Vec::new());
        enc.write_all(&plain).unwrap();
        enc.finish().unwrap()
    };

    use std::io::Read;
    let mut bad = flate2::read::ZlibDecoder::new(&compressed[..]);
    let mut bad_out = Vec::new();
    let result = bad.read_to_end(&mut bad_out);
    assert!(
        result.is_err() || bad_out != plain,
        "zlib must not successfully decode a real LZ4 payload the same way the reference bug did"
    );
}

#[test]
fn uncompressed_payload_is_read_verbatim() {
    let plain = b"stored uncompressed, archive default flag off".to_vec();
    let archive_bytes = build_bsa_archive(104, "loose", "readme.txt", false, &plain);
    let mut archive = BsaArchive::open(Cursor::new(archive_bytes)).unwrap();

    let out = archive.read_file(0).unwrap();
    assert_eq!(out, plain);
}
