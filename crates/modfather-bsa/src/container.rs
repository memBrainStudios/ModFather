//! This crate's [`sevenzip_re::container::ContainerFormat`]/
//! [`sevenzip_re::container::ContainerHandle`] payloads -- the "modular
//! payload" half of the GoF Strategy + Factory registry defined in
//! `sevenzip-re::container`. Wraps [`crate::tes4::BsaArchive`] and
//! [`crate::tes3::Tes3Archive`] without changing either; existing callers
//! of those types' `open`/`entries`/`read_file` directly are unaffected.
//!
//! Two strategies, one per BSA generation ([`Tes4BsaFormat`] and
//! [`Tes3BsaFormat`]), both named `"bsa"` in `format_name()` -- callers
//! never need to know or guess which generation a `.bsa` file is; the
//! registry's magic-byte probe (`b"BSA\0"` vs. `0x0000_0100`) picks the
//! right one transparently, exactly like this project's architecture rule
//! requires ("the header information in the file provides that
//! characteristic").
//!
//! `modfather-bsa` implements this trait pair itself (rather than
//! `sevenzip-re` doing it) so that `sevenzip-re` never has to depend on a
//! Bethesda-specific crate -- see `sevenzip_re::container`'s own doc
//! comment for the full rationale. Nothing here registers itself
//! automatically: a consumer constructs a
//! [`sevenzip_re::container::Registry`] and registers both:
//! ```ignore
//! registry
//!     .register(Box::new(modfather_bsa::container::Tes4BsaFormat))
//!     .register(Box::new(modfather_bsa::container::Tes3BsaFormat));
//! ```

use crate::tes3::format::MAGIC as TES3_MAGIC;
use crate::tes3::reader::Tes3Archive;
use crate::tes4::format::MAGIC as TES4_MAGIC;
use crate::tes4::reader::BsaArchive;
use sevenzip_re::container::{
    ContainerEntry, ContainerError, ContainerFormat, ContainerHandle, ContainerResult, ReadSeek,
};

/// The TES4-and-later BSA (v103-105) [`ContainerFormat`] strategy.
pub struct Tes4BsaFormat;

impl ContainerFormat for Tes4BsaFormat {
    fn format_name(&self) -> &'static str {
        "bsa"
    }

    fn probe_len(&self) -> usize {
        TES4_MAGIC.len()
    }

    fn probe(&self, header: &[u8]) -> bool {
        header == TES4_MAGIC.as_slice()
    }

    fn open(&self, reader: Box<dyn ReadSeek>) -> ContainerResult<Box<dyn ContainerHandle>> {
        let archive =
            BsaArchive::open(reader).map_err(|e| ContainerError::Format(format!("bsa: {e}")))?;
        Ok(Box::new(Tes4BsaHandle(archive)))
    }
}

struct Tes4BsaHandle(BsaArchive<Box<dyn ReadSeek>>);

impl ContainerHandle for Tes4BsaHandle {
    fn format_name(&self) -> &'static str {
        "bsa"
    }

    fn entries(&self) -> Vec<ContainerEntry> {
        self.0
            .entries()
            .into_iter()
            .map(|e| {
                let name = if e.folder.is_empty() {
                    e.name
                } else {
                    format!("{}\\{}", e.folder, e.name)
                };
                ContainerEntry {
                    name,
                    // BSA does not expose a size ahead of decoding (it
                    // depends on whether this entry inverts the archive's
                    // default compression flag) -- `BsaEntry::size` is
                    // itself always 0 for the same reason (see
                    // `tes4/reader.rs`'s "resolved lazily" comment).
                    size: 0,
                    is_dir: false,
                    // BSA does not carry a per-entry CRC in the container
                    // format itself.
                    crc: None,
                }
            })
            .collect()
    }

    fn read_file_at(&mut self, idx: usize) -> ContainerResult<Vec<u8>> {
        self.0
            .read_file(idx)
            .map_err(|e| ContainerError::Format(format!("bsa: {e}")))
    }
}

/// The Morrowind (TES III) BSA [`ContainerFormat`] strategy. Registered
/// alongside [`Tes4BsaFormat`] under the same `"bsa"` name -- the registry
/// tells them apart purely by [`crate::tes3::format::MAGIC`] vs.
/// [`crate::tes4::format::MAGIC`], which never collide.
pub struct Tes3BsaFormat;

impl ContainerFormat for Tes3BsaFormat {
    fn format_name(&self) -> &'static str {
        "bsa"
    }

    fn probe_len(&self) -> usize {
        TES3_MAGIC.len()
    }

    fn probe(&self, header: &[u8]) -> bool {
        header == TES3_MAGIC.as_slice()
    }

    fn open(&self, reader: Box<dyn ReadSeek>) -> ContainerResult<Box<dyn ContainerHandle>> {
        let archive = Tes3Archive::open(reader)
            .map_err(|e| ContainerError::Format(format!("bsa (tes3): {e}")))?;
        Ok(Box::new(Tes3BsaHandle(archive)))
    }
}

struct Tes3BsaHandle(Tes3Archive<Box<dyn ReadSeek>>);

impl ContainerHandle for Tes3BsaHandle {
    fn format_name(&self) -> &'static str {
        "bsa"
    }

    fn entries(&self) -> Vec<ContainerEntry> {
        self.0
            .entries()
            .into_iter()
            .map(|e| ContainerEntry {
                name: e.path,
                size: e.size,
                is_dir: false,
                // Morrowind BSA carries no per-entry CRC either.
                crc: None,
            })
            .collect()
    }

    fn read_file_at(&mut self, idx: usize) -> ContainerResult<Vec<u8>> {
        self.0
            .read_file(idx)
            .map_err(|e| ContainerError::Format(format!("bsa (tes3): {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tes3::writer::{write as write_tes3, Tes3FileToPack};
    use crate::tes4::writer::{write as write_tes4, FileToPack, WriteOptions};
    use sevenzip_re::container::Registry;
    use std::io::Cursor;

    fn make_tes4_bsa_bytes() -> Vec<u8> {
        let files = vec![FileToPack {
            folder: "textures\\armor".to_string(),
            name: "helmet.dds".to_string(),
            data: b"fake dds bytes for the registry test".to_vec(),
        }];
        let options = WriteOptions {
            version: 105,
            compress: true,
        };
        let mut buf = Vec::new();
        write_tes4(Cursor::new(&mut buf), &files, &options).unwrap();
        buf
    }

    fn make_tes3_bsa_bytes() -> Vec<u8> {
        let files = vec![Tes3FileToPack {
            path: "textures\\armor\\helmet.dds".to_string(),
            data: b"fake dds bytes for the tes3 registry test".to_vec(),
        }];
        let mut buf = Vec::new();
        write_tes3(&mut buf, &files).unwrap();
        buf
    }

    #[test]
    fn registry_with_tes4_bsa_format_opens_a_real_bsa_archive() {
        let mut registry = Registry::new();
        registry.register(Box::new(Tes4BsaFormat));

        let bytes = make_tes4_bsa_bytes();
        let mut handle = registry
            .open(Box::new(Cursor::new(bytes)))
            .expect("registry must dispatch to Tes4BsaFormat");

        assert_eq!(handle.format_name(), "bsa");
        let entries = handle.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "textures\\armor\\helmet.dds");

        let content = handle.read_file_at(0).unwrap();
        assert_eq!(content, b"fake dds bytes for the registry test");
    }

    #[test]
    fn registry_with_tes3_bsa_format_opens_a_real_bsa_archive() {
        let mut registry = Registry::new();
        registry.register(Box::new(Tes3BsaFormat));

        let bytes = make_tes3_bsa_bytes();
        let mut handle = registry
            .open(Box::new(Cursor::new(bytes)))
            .expect("registry must dispatch to Tes3BsaFormat");

        assert_eq!(handle.format_name(), "bsa");
        let entries = handle.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "textures\\armor\\helmet.dds");

        let content = handle.read_file_at(0).unwrap();
        assert_eq!(content, b"fake dds bytes for the tes3 registry test");
    }

    #[test]
    fn registry_with_both_bsa_formats_dispatches_by_magic() {
        let mut registry = Registry::new();
        registry
            .register(Box::new(Tes4BsaFormat))
            .register(Box::new(Tes3BsaFormat));

        let tes4_bytes = make_tes4_bsa_bytes();
        let tes4_handle = registry
            .open(Box::new(Cursor::new(tes4_bytes)))
            .expect("registry must open a tes4 archive when both formats are registered");
        assert_eq!(tes4_handle.format_name(), "bsa");

        let tes3_bytes = make_tes3_bsa_bytes();
        let tes3_handle = registry
            .open(Box::new(Cursor::new(tes3_bytes)))
            .expect("registry must open a tes3 archive when both formats are registered");
        assert_eq!(tes3_handle.format_name(), "bsa");
    }

    #[test]
    fn registry_with_only_7z_rejects_a_bsa_archive() {
        let mut registry = Registry::new();
        registry.register(Box::new(sevenzip_re::container::SevenZipFormat));

        let bytes = make_tes4_bsa_bytes();
        let result = registry.open(Box::new(Cursor::new(bytes)));
        assert!(result.is_err());
    }
}
