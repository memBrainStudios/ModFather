//! Write -> read round-trip tests for the BA2 (GNRL) writer, proving it
//! produces archives this crate's own reader can parse correctly.
//!
//! v2/v3 cases here also exercise the Starfield header-extension fix
//! (see `format::header_size_for_version`'s doc comment): v2 is always
//! zlib, and v3's codec is a real per-archive `compression_method` field
//! (`WriteOptions::force_lz4_v3`) rather than implied by version number.

use modfather_ba2::{write, Ba2Archive, EntryKind, FileToPack, WriteOptions};
use std::io::Cursor;

fn pack_and_open(files: &[FileToPack], options: &WriteOptions) -> Ba2Archive<Cursor<Vec<u8>>> {
    let mut buf = Vec::new();
    write(Cursor::new(&mut buf), files, options).expect("write should succeed");
    Ba2Archive::open(Cursor::new(buf)).expect("archive should re-open")
}

#[test]
fn v1_zlib_round_trip_multiple_files() {
    let files = vec![
        FileToPack {
            name: "Interface\\HUDMenu.swf".to_string(),
            data: b"swf bytes, repeated for compressibility ".repeat(20),
        },
        FileToPack {
            name: "Scripts\\Source\\SomeScript.psc".to_string(),
            data: b"Scriptname SomeScript extends Quest ".repeat(15),
        },
    ];
    let options = WriteOptions {
        version: 1,
        compress: true,
        force_lz4_v3: false,
    };

    let mut archive = pack_and_open(&files, &options);
    assert_eq!(archive.version(), 1);
    let entries = archive.entries().to_vec();
    assert_eq!(entries.len(), 2);

    for (idx, original) in files.iter().enumerate() {
        let expected_name = original.name.replace('/', "\\").to_lowercase();
        assert_eq!(entries[idx].name, expected_name);
        match &entries[idx].kind {
            EntryKind::General { .. } => {}
            EntryKind::Texture { .. } => panic!("expected a General entry"),
        }
        let out = archive.read_file(idx).unwrap();
        assert_eq!(out, original.data, "round-tripped bytes for {}", original.name);
    }
}

#[test]
fn v2_is_always_zlib_round_trip() {
    // v2 (Starfield GNRL) has no per-archive compression_method field --
    // it is always zlib. This also exercises the 8-byte v2 header
    // extension this crate now reads/writes (see module docs).
    let files = vec![FileToPack {
        name: "Sound/FX/click.wav".to_string(),
        data: b"pretend wav bytes, repeated for compressibility ".repeat(30),
    }];
    let options = WriteOptions {
        version: 2,
        compress: true,
        force_lz4_v3: false, // ignored for v2, included for clarity
    };

    let mut archive = pack_and_open(&files, &options);
    assert_eq!(archive.version(), 2);
    let out = archive.read_file(0).unwrap();
    assert_eq!(out, files[0].data);
}

#[test]
fn v3_default_zlib_round_trip() {
    // v3 with force_lz4_v3 = false must write compression_method = 0
    // (zlib) in its 12-byte header extension and round-trip correctly.
    let files = vec![FileToPack {
        name: "Textures/rocks/boulder01_d.dds".to_string(),
        data: b"pretend dds bytes, repeated for compressibility ".repeat(40),
    }];
    let options = WriteOptions {
        version: 3,
        compress: true,
        force_lz4_v3: false,
    };

    let mut archive = pack_and_open(&files, &options);
    assert_eq!(archive.version(), 3);
    let out = archive.read_file(0).unwrap();
    assert_eq!(out, files[0].data);
}

#[test]
fn v3_forced_lz4_round_trip() {
    // v3 with force_lz4_v3 = true must write compression_method = 3
    // (LZ4 block) and this crate's own reader must decode it correctly.
    let files = vec![FileToPack {
        name: "Textures/rocks/boulder01_n.dds".to_string(),
        data: b"pretend normal-map dds bytes, repeated for compressibility ".repeat(40),
    }];
    let options = WriteOptions {
        version: 3,
        compress: true,
        force_lz4_v3: true,
    };

    let mut archive = pack_and_open(&files, &options);
    assert_eq!(archive.version(), 3);
    let out = archive.read_file(0).unwrap();
    assert_eq!(out, files[0].data);
}

#[test]
fn uncompressed_round_trip() {
    let files = vec![FileToPack {
        name: "readme.txt".to_string(),
        data: b"stored uncompressed, packedSize == 0".to_vec(),
    }];
    let options = WriteOptions {
        version: 1,
        compress: false,
        force_lz4_v3: false,
    };

    let mut archive = pack_and_open(&files, &options);
    let out = archive.read_file(0).unwrap();
    assert_eq!(out, files[0].data);
}
