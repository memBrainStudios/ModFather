//! Write -> read round-trip tests for the BSA writer, proving it produces
//! archives this crate's own reader can parse correctly (and, since the
//! reader implements the exact UESP byte layout, that the writer is
//! spec-conforming rather than just self-consistent).

use modfather_bsa::{write, BsaArchive, FileToPack, WriteOptions};
use std::io::Cursor;

fn pack_and_open(files: &[FileToPack], options: &WriteOptions) -> BsaArchive<Cursor<Vec<u8>>> {
    let mut buf = Vec::new();
    write(Cursor::new(&mut buf), files, options).expect("write should succeed");
    BsaArchive::open(Cursor::new(buf)).expect("archive should re-open")
}

#[test]
fn v105_compressed_round_trip_multiple_files_and_folders() {
    let files = vec![
        FileToPack {
            folder: "meshes\\armor\\iron".to_string(),
            name: "cuirass.nif".to_string(),
            data: b"nif model bytes, repeated for compressibility ".repeat(20),
        },
        FileToPack {
            folder: "meshes\\armor\\iron".to_string(),
            name: "gauntlets.nif".to_string(),
            data: b"another nif model, repeated for compressibility ".repeat(15),
        },
        FileToPack {
            folder: "textures\\armor\\iron".to_string(),
            name: "cuirass.dds".to_string(),
            data: b"dds texture bytes, repeated for compressibility ".repeat(25),
        },
    ];
    let options = WriteOptions {
        version: 105,
        compress: true,
    };

    let mut archive = pack_and_open(&files, &options);
    let entries = archive.entries();
    assert_eq!(entries.len(), 3);

    // All three files must be present (order is hash-sorted, not insertion
    // order) and each must decode back to its original bytes.
    for original in &files {
        let idx = entries
            .iter()
            .position(|e| {
                e.name == original.name.to_lowercase()
                    && e.folder == original.folder.to_lowercase()
            })
            .unwrap_or_else(|| panic!("missing entry for {}/{}", original.folder, original.name));
        let out = archive.read_file(idx).unwrap();
        assert_eq!(out, original.data, "round-tripped bytes for {}", original.name);
    }
}

#[test]
fn v104_zlib_round_trip() {
    let files = vec![FileToPack {
        folder: "sound\\fx".to_string(),
        name: "click.wav".to_string(),
        data: b"pretend wav bytes, repeated for compressibility ".repeat(30),
    }];
    let options = WriteOptions {
        version: 104,
        compress: true,
    };

    let mut archive = pack_and_open(&files, &options);
    assert_eq!(archive.entries().len(), 1);
    let out = archive.read_file(0).unwrap();
    assert_eq!(out, files[0].data);
}

#[test]
fn uncompressed_round_trip() {
    let files = vec![FileToPack {
        folder: "loose".to_string(),
        name: "readme.txt".to_string(),
        data: b"stored uncompressed".to_vec(),
    }];
    let options = WriteOptions {
        version: 105,
        compress: false,
    };

    let mut archive = pack_and_open(&files, &options);
    let out = archive.read_file(0).unwrap();
    assert_eq!(out, files[0].data);
}

#[test]
fn rejects_unsupported_version() {
    let files = vec![FileToPack {
        folder: "x".to_string(),
        name: "y.txt".to_string(),
        data: b"z".to_vec(),
    }];
    let options = WriteOptions {
        version: 999,
        compress: false,
    };
    let mut buf = Vec::new();
    let result = write(Cursor::new(&mut buf), &files, &options);
    assert!(result.is_err());
}
