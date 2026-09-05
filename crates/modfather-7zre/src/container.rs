//! Assembles the one shared [`sevenzip_re::container::Registry`] this
//! project's docs already describe in prose ("7-Zip RE's container
//! registry", "additional container handlers alongside 7z") into an
//! actual, working value -- this is the only place that step happens.
//!
//! `sevenzip-re` defines the trait pair and ships the registry mechanism
//! itself, empty (see `sevenzip_re::container`'s doc comment for why it
//! never pre-registers anything). Each format crate implements the trait
//! pair for its own archive type (`sevenzip_re::container::SevenZipFormat`,
//! `modfather_bsa::container::{Tes3BsaFormat, Tes4BsaFormat}`,
//! `modfather_ba2::container::Ba2Format`). `modfather-7zre` is the crate
//! that depends on every one of them (see this crate's own top-level doc
//! comment for why `sevenzip-re` itself cannot be that crate, and why
//! `modfather-vestibule` must not be either), so it is the correct place
//! -- and the only correct place, without inverting the custody chain's
//! dependency direction -- to actually register all of them together.
//!
//! **This module used to live in `modfather-vestibule`.** Per the
//! project's architecture decision ("Vestibule is a client of 7-Zip
//! RE... at no time does Vestibule implement anything related to an
//! archive"), assembling this registry is archive-format logic and does
//! not belong in Vestibule; it moved here, to `modfather-7zre`, which sits
//! between the format crates and Vestibule in the custody chain
//! specifically so Vestibule can depend on one crate that already depends
//! on every format, without ever naming a format crate itself.
//!
//! When RAR support lands (license permitting), it becomes one more
//! `ContainerFormat` implementation registered by one more line in
//! [`build_registry`]; nothing else in this module, or in any of its
//! callers, needs to change -- that is the point of the pattern.

use sevenzip_re::container::Registry;

/// Build the one [`Registry`] with every currently-implemented container
/// format registered: 7z (via `sevenzip-re` itself), both BSA generations
/// (Morrowind's [`modfather_bsa::container::Tes3BsaFormat`] and Oblivion-
/// through-Skyrim's [`modfather_bsa::container::Tes4BsaFormat`]), and BA2.
///
/// Registration order does not currently matter (7z/BSA/BA2 magics never
/// overlap, and the two BSA generations' magics are disjoint from each
/// other too), but is listed in "custody chain" order for readability: the
/// standalone engine first, then each Bethesda extension, oldest BSA
/// generation first.
pub fn build_registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .register(Box::new(sevenzip_re::container::SevenZipFormat))
        .register(Box::new(modfather_bsa::container::Tes3BsaFormat))
        .register(Box::new(modfather_bsa::container::Tes4BsaFormat))
        .register(Box::new(modfather_ba2::container::Ba2Format));
    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn make_7z_bytes() -> Vec<u8> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.7z");
        let entries = vec![sevenzip_re::NewEntry {
            name: "hello.txt".to_string(),
            data: b"hello from the shared registry".to_vec(),
        }];
        sevenzip_re::create(&path, &entries, sevenzip_re::PackCodec::Lzma2).unwrap();
        std::fs::read(&path).unwrap()
    }

    fn make_bsa_bytes() -> Vec<u8> {
        let files = vec![modfather_bsa::FileToPack {
            folder: "meshes".to_string(),
            name: "sword.nif".to_string(),
            data: b"bsa payload via the shared registry".to_vec(),
        }];
        let options = modfather_bsa::WriteOptions {
            version: 105,
            compress: true,
        };
        let mut buf = Vec::new();
        modfather_bsa::write(Cursor::new(&mut buf), &files, &options).unwrap();
        buf
    }

    fn make_ba2_bytes() -> Vec<u8> {
        let files = vec![modfather_ba2::FileToPack {
            name: "meshes\\sword.nif".to_string(),
            data: b"ba2 payload via the shared registry".to_vec(),
        }];
        let options = modfather_ba2::WriteOptions {
            version: 1,
            compress: true,
            force_lz4_v3: false,
        };
        let mut buf = Vec::new();
        modfather_ba2::write(Cursor::new(&mut buf), &files, &options).unwrap();
        buf
    }

    /// The composed proof this module exists for: one [`Registry`],
    /// built once by [`build_registry`], correctly probes and dispatches
    /// real archives of different formats -- 7z, (TES4) BSA, and BA2 --
    /// to their own handler, purely from magic bytes, with no
    /// caller-side format switch anywhere. This is the "modular payload"
    /// architecture the user asked for, exercised end to end rather than
    /// per-crate in isolation.
    #[test]
    fn shared_registry_dispatches_7z_bsa_and_ba2_each_to_their_own_handler() {
        let registry = build_registry();
        assert_eq!(registry.len(), 4);

        let mut sevenzip_handle = registry.open(Box::new(Cursor::new(make_7z_bytes()))).unwrap();
        assert_eq!(sevenzip_handle.format_name(), "7z");
        assert_eq!(
            sevenzip_handle.read_file_at(0).unwrap(),
            b"hello from the shared registry"
        );

        let mut bsa_handle = registry.open(Box::new(Cursor::new(make_bsa_bytes()))).unwrap();
        assert_eq!(bsa_handle.format_name(), "bsa");
        assert_eq!(
            bsa_handle.read_file_at(0).unwrap(),
            b"bsa payload via the shared registry"
        );

        let mut ba2_handle = registry.open(Box::new(Cursor::new(make_ba2_bytes()))).unwrap();
        assert_eq!(ba2_handle.format_name(), "ba2");
        assert_eq!(
            ba2_handle.read_file_at(0).unwrap(),
            b"ba2 payload via the shared registry"
        );
    }

    /// RAR bytes (a real RAR5 magic, `Rar!\x1A\x07\x01\x00`) match no
    /// registered format -- proving the registry correctly refuses to
    /// silently mis-dispatch an unimplemented format, rather than e.g.
    /// falling through to whichever handler happens to be registered
    /// last. RAR itself stays unimplemented pending a license (per
    /// `docs/VESTIBULE.md`); this test only exercises the *registry's*
    /// behavior on unrecognized input, not RAR decoding.
    #[test]
    fn shared_registry_rejects_rar_bytes_since_no_rar_format_is_registered_yet() {
        let registry = build_registry();
        let rar5_magic: Vec<u8> = vec![0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00];
        let result = registry.open(Box::new(Cursor::new(rar5_magic)));
        assert!(result.is_err());
    }
}
