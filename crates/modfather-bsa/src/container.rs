//! This crate's [`sevenzip_re::container::ContainerFormat`]/
//! [`sevenzip_re::container::ContainerHandle`] payload -- the "modular
//! payload" half of the GoF Strategy + Factory registry defined in
//! `sevenzip-re::container`. Wraps [`crate::BsaArchive`] without changing
//! it; existing callers of `BsaArchive::open`/`entries`/`read_file`
//! directly are unaffected.
//!
//! `modfather-bsa` implements this trait pair itself (rather than
//! `sevenzip-re` doing it) so that `sevenzip-re` never has to depend on a
//! Bethesda-specific crate -- see `sevenzip_re::container`'s own doc
//! comment for the full rationale. Nothing here registers itself
//! automatically: a consumer (typically `modfather-vestibule`, the crate
//! that already depends on every format crate) constructs a
//! [`sevenzip_re::container::Registry`] and calls
//! `registry.register(Box::new(modfather_bsa::container::BsaFormat))`.

use crate::format::MAGIC;
use crate::reader::BsaArchive;
use sevenzip_re::container::{
    ContainerEntry, ContainerError, ContainerFormat, ContainerHandle, ContainerResult, ReadSeek,
};

/// The BSA (v103-105) [`ContainerFormat`] strategy.
pub struct BsaFormat;

impl ContainerFormat for BsaFormat {
    fn format_name(&self) -> &'static str {
        "bsa"
    }

    fn probe_len(&self) -> usize {
        MAGIC.len()
    }

    fn probe(&self, header: &[u8]) -> bool {
        header == MAGIC.as_slice()
    }

    fn open(&self, reader: Box<dyn ReadSeek>) -> ContainerResult<Box<dyn ContainerHandle>> {
        let archive =
            BsaArchive::open(reader).map_err(|e| ContainerError::Format(format!("bsa: {e}")))?;
        Ok(Box::new(BsaHandle(archive)))
    }
}

struct BsaHandle(BsaArchive<Box<dyn ReadSeek>>);

impl ContainerHandle for BsaHandle {
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
                    // `reader.rs`'s "resolved lazily" comment).
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::{write, FileToPack, WriteOptions};
    use sevenzip_re::container::Registry;
    use std::io::Cursor;

    fn make_bsa_bytes() -> Vec<u8> {
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
        write(Cursor::new(&mut buf), &files, &options).unwrap();
        buf
    }

    #[test]
    fn registry_with_bsa_format_opens_a_real_bsa_archive() {
        let mut registry = Registry::new();
        registry.register(Box::new(BsaFormat));

        let bytes = make_bsa_bytes();
        let mut handle = registry
            .open(Box::new(Cursor::new(bytes)))
            .expect("registry must dispatch to BsaFormat");

        assert_eq!(handle.format_name(), "bsa");
        let entries = handle.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "textures\\armor\\helmet.dds");

        let content = handle.read_file_at(0).unwrap();
        assert_eq!(content, b"fake dds bytes for the registry test");
    }

    #[test]
    fn registry_with_only_7z_rejects_a_bsa_archive() {
        let mut registry = Registry::new();
        registry.register(Box::new(sevenzip_re::container::SevenZipFormat));

        let bytes = make_bsa_bytes();
        let result = registry.open(Box::new(Cursor::new(bytes)));
        assert!(result.is_err());
    }
}
