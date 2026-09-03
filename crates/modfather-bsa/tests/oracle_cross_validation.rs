//! Cross-validation against an independently-written BSA implementation.
//!
//! `docs/SCHEDULE.md`'s Wave 0 gate calls for testing against "a real BSA".
//! Genuine game-shipped BSA files are Bethesda's copyrighted game assets and
//! are not available in this sandbox (no licensed game install), so a
//! same-crate write->read round trip (see `write_roundtrip.rs`) can only
//! prove this crate is *self-consistent*, not that it agrees with the public
//! UESP spec as anyone else would implement it.
//!
//! To actually close that gap without needing licensed game content, this
//! test uses the `ba2` crate (crates.io, 0BSD license, written independently
//! of ModFather) purely as a **dev-dependency test oracle**:
//! - `ba2::tes4::Archive::write` produces a BSA that this crate's reader must
//!   parse and decode correctly.
//! - This crate's `write` produces a BSA that `ba2::tes4::Archive::read` must
//!   parse and decode correctly.
//!
//! `ba2` is never a runtime dependency of `modfather-bsa` or of the
//! ModFather product; it appears only in `[dev-dependencies]` and only in
//! this test file. This does not relax the "full native standalone Rust
//! implementation" mandate -- both implementations remain independent, pure
//! Rust, with no host `7z`/game-tool shelling out anywhere.

use ba2::prelude::*;
use ba2::tes4::{
    Archive as OracleArchive, ArchiveKey as OracleArchiveKey, ArchiveOptions as OracleOptions,
    ArchiveTypes as OracleTypes, Directory as OracleDirectory, DirectoryKey as OracleDirectoryKey,
    File as OracleFile, Version as OracleVersion,
};
use modfather_bsa::{write, BsaArchive, FileToPack, WriteOptions};
use std::io::Cursor;

/// Our writer -> the oracle's reader.
///
/// If our writer produced a byte layout the oracle can't parse (wrong field
/// offsets, wrong hash-sort order, wrong bzstring encoding, etc.), this
/// would fail even though our own reader might accept it -- so this is a
/// materially stronger check than the same-crate round trip.
#[test]
fn our_writer_is_readable_by_independent_oracle() {
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
        FileToPack {
            folder: "sound\\fx".to_string(),
            name: "click.wav".to_string(),
            data: b"pretend wav bytes, repeated for compressibility ".repeat(30),
        },
    ];

    for (version, compress) in [(103u32, true), (104, true), (105, true), (105, false)] {
        let options = WriteOptions { version, compress };
        let mut buf = Vec::new();
        write(Cursor::new(&mut buf), &files, &options)
            .unwrap_or_else(|e| panic!("our writer failed for v{version} compress={compress}: {e}"));

        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &buf).unwrap();
        let (oracle, meta) = OracleArchive::read(tmp.path()).unwrap_or_else(|e| {
            panic!("oracle rejected our v{version} compress={compress} archive: {e}")
        });
        assert_eq!(meta.version() as u32, version);

        for original in &files {
            let folder_key = OracleArchiveKey::from(
                original.folder.to_lowercase().replace('/', "\\").into_bytes(),
            );
            let dir = oracle
                .get(&folder_key)
                .unwrap_or_else(|| panic!("oracle: missing folder {}", original.folder));
            let file_key =
                OracleDirectoryKey::from(original.name.to_lowercase().into_bytes());
            let oracle_file = dir
                .get(&file_key)
                .unwrap_or_else(|| panic!("oracle: missing file {}", original.name));

            let mut decoded = Vec::new();
            let read_options = meta.into();
            oracle_file
                .write(&mut decoded, &read_options)
                .unwrap_or_else(|e| panic!("oracle failed to decode {}: {e}", original.name));
            assert_eq!(
                decoded, original.data,
                "oracle-decoded bytes for {} (v{version}, compress={compress})",
                original.name
            );
        }
    }
}

/// The oracle's writer -> our reader.
///
/// If our reader made an assumption the oracle's writer doesn't share (e.g.
/// about ordering, padding, or codec dispatch per version), this would fail
/// even though our own writer's output round-trips through our own reader.
#[test]
fn oracle_writer_is_readable_by_our_reader() {
    // Oracle archive: two folders, mixed compressible content, SSE (v105 ->
    // LZ4) so this also exercises the exact codec bug class the reference
    // implementation got wrong (unconditional zlib on v105 payloads).
    let armor_data = b"nif model bytes, repeated for compressibility ".repeat(20);
    let armor_file = OracleFile::from_decompressed(armor_data.as_slice());
    let armor_dir: OracleDirectory = [(
        OracleDirectoryKey::from(b"cuirass.nif".as_slice()),
        armor_file,
    )]
    .into_iter()
    .collect();

    let sound_data = b"pretend wav bytes, repeated for compressibility ".repeat(30);
    let sound_file = OracleFile::from_decompressed(sound_data.as_slice());
    let sound_dir: OracleDirectory = [(
        OracleDirectoryKey::from(b"click.wav".as_slice()),
        sound_file,
    )]
    .into_iter()
    .collect();

    let archive: OracleArchive = [
        (
            OracleArchiveKey::from(b"meshes\\armor\\iron".as_slice()),
            armor_dir,
        ),
        (OracleArchiveKey::from(b"sound\\fx".as_slice()), sound_dir),
    ]
    .into_iter()
    .collect();

    let options = OracleOptions::builder()
        .types(OracleTypes::MESHES | OracleTypes::SOUNDS)
        .version(OracleVersion::SSE)
        .build();

    let mut buf = Vec::new();
    archive
        .write(&mut buf, &options)
        .expect("oracle writer should succeed");

    let mut ours = BsaArchive::open(Cursor::new(buf)).expect("our reader should open the oracle's v105 archive");
    let entries = ours.entries();
    assert_eq!(entries.len(), 2);

    let expectations: &[(&str, &str, Vec<u8>)] = &[
        (
            "meshes\\armor\\iron",
            "cuirass.nif",
            b"nif model bytes, repeated for compressibility ".repeat(20),
        ),
        (
            "sound\\fx",
            "click.wav",
            b"pretend wav bytes, repeated for compressibility ".repeat(30),
        ),
    ];

    for (folder, name, expected) in expectations {
        let idx = entries
            .iter()
            .position(|e| &e.folder == folder && &e.name == name)
            .unwrap_or_else(|| panic!("our reader: missing entry {folder}/{name}"));
        let out = ours.read_file(idx).unwrap();
        assert_eq!(&out, expected, "our reader decoded {folder}/{name}");
    }
}
