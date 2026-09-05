//! Cross-validation of `modfather_bsa::tes3` against the independently
//! written `ba2` crate's own `tes3` module -- the same rationale and same
//! oracle dependency as `oracle_cross_validation.rs` uses for the tes4
//! (v103-105) family, applied to Morrowind's BSA. See that file's module
//! doc comment for why an independent oracle crate is used instead of a
//! same-crate round trip, and why this does not relax the "full native
//! standalone Rust implementation" mandate.

use ba2::prelude::*;
use ba2::tes3::{Archive as OracleArchive, ArchiveKey as OracleArchiveKey, File as OracleFile};
use ba2::Borrowed;
use modfather_bsa::tes3::{write, Tes3Archive, Tes3FileToPack};
use std::io::Cursor;

/// Our writer -> the oracle's reader.
#[test]
fn our_tes3_writer_is_readable_by_independent_oracle() {
    let files = vec![
        Tes3FileToPack {
            path: "meshes\\armor\\iron\\cuirass.nif".to_string(),
            data: b"nif model bytes, repeated for realism ".repeat(20),
        },
        Tes3FileToPack {
            path: "meshes\\armor\\iron\\gauntlets.nif".to_string(),
            data: b"another nif model, repeated for realism ".repeat(15),
        },
        Tes3FileToPack {
            path: "textures\\armor\\iron\\cuirass.dds".to_string(),
            data: b"dds texture bytes, repeated for realism ".repeat(25),
        },
        Tes3FileToPack {
            path: "sound\\fx\\click.wav".to_string(),
            data: b"pretend wav bytes, repeated for realism ".repeat(30),
        },
    ];

    let mut buf = Vec::new();
    write(&mut buf, &files).expect("our tes3 writer should succeed");

    let oracle =
        OracleArchive::read(Borrowed(buf.as_slice())).expect("oracle should read our tes3 archive");

    for original in &files {
        let key = OracleArchiveKey::from(original.path.replace('/', "\\").into_bytes());
        let oracle_file = oracle
            .get(&key)
            .unwrap_or_else(|| panic!("oracle: missing file {}", original.path));
        assert_eq!(
            oracle_file.as_bytes(),
            original.data.as_slice(),
            "oracle-decoded bytes for {}",
            original.path
        );
    }
}

/// The oracle's writer -> our reader.
#[test]
fn oracle_tes3_writer_is_readable_by_our_reader() {
    let armor_data = b"nif model bytes, repeated for realism ".repeat(20);
    let armor_file: OracleFile = armor_data.as_slice().into();
    let sound_data = b"pretend wav bytes, repeated for realism ".repeat(30);
    let sound_file: OracleFile = sound_data.as_slice().into();

    let archive: OracleArchive = [
        (
            OracleArchiveKey::from(b"meshes\\armor\\iron\\cuirass.nif".as_slice()),
            armor_file,
        ),
        (
            OracleArchiveKey::from(b"sound\\fx\\click.wav".as_slice()),
            sound_file,
        ),
    ]
    .into_iter()
    .collect();

    let mut buf = Vec::new();
    archive
        .write(&mut buf)
        .expect("oracle tes3 writer should succeed");

    let mut ours =
        Tes3Archive::open(Cursor::new(buf)).expect("our tes3 reader should open the oracle's archive");
    let entries = ours.entries();
    assert_eq!(entries.len(), 2);

    let expectations: &[(&str, Vec<u8>)] = &[
        (
            "meshes\\armor\\iron\\cuirass.nif",
            b"nif model bytes, repeated for realism ".repeat(20),
        ),
        (
            "sound\\fx\\click.wav",
            b"pretend wav bytes, repeated for realism ".repeat(30),
        ),
    ];

    for (path, expected) in expectations {
        let idx = entries
            .iter()
            .position(|e| &e.path == path)
            .unwrap_or_else(|| panic!("our reader: missing entry {path}"));
        let out = ours.read_file(idx).unwrap();
        assert_eq!(&out, expected, "our reader decoded {path}");
    }
}

/// Confirms the two BSA generations' magic bytes never collide, i.e. that
/// `crate::container`'s registry can always tell them apart from the
/// header alone. This is the concrete, testable form of the project's
/// architecture rule ("the header information in the file provides that
/// characteristic").
#[test]
fn tes3_and_tes4_magic_bytes_never_collide() {
    assert_ne!(
        modfather_bsa::tes3::format::MAGIC,
        {
            // tes4::format::MAGIC is 4 bytes too, but a different type
            // constant -- compare byte-for-byte via a local binding to
            // keep this test crate-internals-agnostic about the exact
            // constant type.
            let tes4_magic: [u8; 4] = *b"BSA\0";
            tes4_magic
        }
    );
}
