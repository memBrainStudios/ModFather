//! Write -> read round-trip tests for the DX10 (texture-chunk) writer
//! ([`modfather_ba2::write_dx10`]), the gap flagged as follow-up work in
//! both `writer.rs`'s and `modfather-vestibule/src/packing.rs`'s module
//! doc comments (Textures BA2 archives were previously always packed as
//! GNRL, never real DX10).
//!
//! This crate's own [`modfather_ba2::reader::Ba2Archive::read_dx10_entries`]
//! is the target byte layout: `F4TexInfo` (24 bytes) + one `F4TexChunk`
//! (24 bytes) per texture, per the Wave-0 single-full-mip-range-chunk
//! scope documented on `write_dx10`. These tests confirm the writer's
//! output actually parses back through that exact reader path with the
//! metadata and payload bytes preserved.

use modfather_ba2::{write_dx10, Ba2Archive, EntryKind, TextureToPack, WriteOptions};
use std::io::Cursor;

fn pack_and_open(
    textures: &[TextureToPack],
    options: &WriteOptions,
) -> Ba2Archive<Cursor<Vec<u8>>> {
    let mut buf = Vec::new();
    write_dx10(Cursor::new(&mut buf), textures, options).expect("write_dx10 should succeed");
    Ba2Archive::open(Cursor::new(buf)).expect("archive should re-open")
}

#[test]
fn v1_zlib_dx10_round_trip_single_texture() {
    let textures = vec![TextureToPack {
        name: "Textures\\Armor\\Cuirass_d.dds".to_string(),
        data: b"pretend BC7 mip bytes, repeated for compressibility ".repeat(40),
        height: 512,
        width: 512,
        num_mips: 9,
        format: 98, // DXGI_FORMAT_BC7_UNORM
    }];
    let options = WriteOptions {
        version: 1,
        compress: true,
        force_lz4_v3: false,
    };

    let mut archive = pack_and_open(&textures, &options);
    assert_eq!(archive.version(), 1);
    let entries = archive.entries().to_vec();
    assert_eq!(entries.len(), 1);

    let expected_name = textures[0].name.replace('/', "\\").to_lowercase();
    assert_eq!(entries[0].name, expected_name);

    match &entries[0].kind {
        EntryKind::Texture {
            height,
            width,
            num_mips,
            format,
            chunks,
        } => {
            assert_eq!(*height, 512);
            assert_eq!(*width, 512);
            assert_eq!(*num_mips, 9);
            assert_eq!(*format, 98);
            assert_eq!(chunks.len(), 1, "Wave-0 writer emits exactly one chunk");
            assert_eq!(chunks[0].start_mip, 0);
            assert_eq!(chunks[0].end_mip, 8, "end_mip == num_mips - 1");
        }
        EntryKind::General { .. } => panic!("expected a Texture entry"),
    }

    let out = archive.read_chunk(0, 0).unwrap();
    assert_eq!(out, textures[0].data, "round-tripped mip bytes");
}

#[test]
fn multiple_textures_and_uncompressed_mode() {
    let textures = vec![
        TextureToPack {
            name: "textures\\cube_d.dds".to_string(),
            data: b"diffuse mip bytes, repeated for compressibility ".repeat(30),
            height: 256,
            width: 256,
            num_mips: 8,
            format: 71, // BC1_UNORM
        },
        TextureToPack {
            name: "textures\\cube_n.dds".to_string(),
            data: b"normal map mip bytes, repeated for compressibility ".repeat(30),
            height: 256,
            width: 256,
            num_mips: 8,
            format: 77, // BC3_UNORM
        },
    ];
    let options = WriteOptions {
        version: 1,
        compress: false,
        force_lz4_v3: false,
    };

    let mut archive = pack_and_open(&textures, &options);
    let entries = archive.entries().to_vec();
    assert_eq!(entries.len(), 2);

    for (idx, original) in textures.iter().enumerate() {
        let out = archive.read_chunk(idx, 0).unwrap();
        assert_eq!(out, original.data, "uncompressed round trip for {}", original.name);
    }
}

#[test]
fn v3_lz4_dx10_round_trip() {
    // v3's codec is a real per-archive `compression_method` field; DX10
    // shares the exact same header-extension/codec-selection logic as
    // GNRL (see `format::header_size_for_version`), so this exercises
    // that shared path through the DX10 writer specifically.
    let textures = vec![TextureToPack {
        name: "textures\\starfield_planet_d.dds".to_string(),
        data: b"starfield mip bytes, repeated for compressibility ".repeat(50),
        height: 1024,
        width: 1024,
        num_mips: 11,
        format: 98,
    }];
    let options = WriteOptions {
        version: 3,
        compress: true,
        force_lz4_v3: true,
    };

    let mut archive = pack_and_open(&textures, &options);
    assert_eq!(archive.version(), 3);
    let out = archive.read_chunk(0, 0).unwrap();
    assert_eq!(out, textures[0].data);
}

#[test]
fn single_mip_texture_has_end_mip_zero() {
    // num_mips == 1 (e.g. a UI icon with no mip chain) must not
    // underflow when computing end_mip = num_mips - 1.
    let textures = vec![TextureToPack {
        name: "textures\\interface\\icon.dds".to_string(),
        data: b"single-mip icon bytes".to_vec(),
        height: 32,
        width: 32,
        num_mips: 1,
        format: 71,
    }];
    let options = WriteOptions::default();

    let mut archive = pack_and_open(&textures, &options);
    match &archive.entries()[0].kind {
        EntryKind::Texture { chunks, .. } => {
            assert_eq!(chunks[0].start_mip, 0);
            assert_eq!(chunks[0].end_mip, 0);
        }
        EntryKind::General { .. } => panic!("expected a Texture entry"),
    }
    assert_eq!(archive.read_chunk(0, 0).unwrap(), textures[0].data);
}
