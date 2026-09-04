//! This crate's [`sevenzip_re::container::ContainerFormat`]/
//! [`sevenzip_re::container::ContainerHandle`] payload -- see
//! `modfather_bsa::container`'s doc comment for the full rationale
//! (shared by every extension crate implementing this same trait pair).
//! Wraps [`crate::Ba2Archive`] without changing it; existing callers of
//! `Ba2Archive::open`/`entries`/`read_file`/`read_chunk` directly are
//! unaffected.

use crate::format::MAGIC;
use crate::reader::{Ba2Archive, EntryKind};
use sevenzip_re::container::{
    ContainerEntry, ContainerError, ContainerFormat, ContainerHandle, ContainerResult, ReadSeek,
};

/// The BA2 (GNRL/DX10) [`ContainerFormat`] strategy.
pub struct Ba2Format;

impl ContainerFormat for Ba2Format {
    fn format_name(&self) -> &'static str {
        "ba2"
    }

    fn probe_len(&self) -> usize {
        MAGIC.len()
    }

    fn probe(&self, header: &[u8]) -> bool {
        header == MAGIC.as_slice()
    }

    fn open(&self, reader: Box<dyn ReadSeek>) -> ContainerResult<Box<dyn ContainerHandle>> {
        let archive =
            Ba2Archive::open(reader).map_err(|e| ContainerError::Format(format!("ba2: {e}")))?;
        Ok(Box::new(Ba2Handle(archive)))
    }
}

struct Ba2Handle(Ba2Archive<Box<dyn ReadSeek>>);

impl ContainerHandle for Ba2Handle {
    fn format_name(&self) -> &'static str {
        "ba2"
    }

    fn entries(&self) -> Vec<ContainerEntry> {
        self.0
            .entries()
            .iter()
            .map(|e| {
                let size = match &e.kind {
                    EntryKind::General { unpacked_size, .. } => *unpacked_size as u64,
                    EntryKind::Texture { chunks, .. } => {
                        chunks.iter().map(|c| c.unpacked_size as u64).sum()
                    }
                };
                ContainerEntry {
                    name: e.name.clone(),
                    size,
                    is_dir: false,
                    // BA2 does not carry a per-entry CRC in the container
                    // format itself.
                    crc: None,
                }
            })
            .collect()
    }

    fn read_file_at(&mut self, idx: usize) -> ContainerResult<Vec<u8>> {
        let is_texture = matches!(
            self.0
                .entries()
                .get(idx)
                .ok_or_else(|| ContainerError::Format(format!("ba2: no such entry: index {idx}")))?
                .kind,
            EntryKind::Texture { .. }
        );

        if !is_texture {
            return self
                .0
                .read_file(idx)
                .map_err(|e| ContainerError::Format(format!("ba2: {e}")));
        }

        // Texture entries have no single "whole file" read on
        // `Ba2Archive` itself (mips are independently streamable via
        // `read_chunk`, by design -- see `reader.rs`'s `TexChunk` doc
        // comment): concatenate every chunk in order so a format-agnostic
        // registry caller still gets one coherent byte stream per entry,
        // matching what a full (non-streaming) extraction would produce.
        let num_chunks = match &self.0.entries()[idx].kind {
            EntryKind::Texture { chunks, .. } => chunks.len(),
            EntryKind::General { .. } => unreachable!("checked above"),
        };
        let mut out = Vec::new();
        for chunk_index in 0..num_chunks {
            let bytes = self
                .0
                .read_chunk(idx, chunk_index)
                .map_err(|e| ContainerError::Format(format!("ba2: {e}")))?;
            out.extend_from_slice(&bytes);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::{write, write_dx10, FileToPack, TextureToPack, WriteOptions};
    use sevenzip_re::container::Registry;
    use std::io::Cursor;

    fn options() -> WriteOptions {
        WriteOptions {
            version: 1,
            compress: true,
            force_lz4_v3: false,
        }
    }

    fn make_gnrl_bytes() -> Vec<u8> {
        let files = vec![FileToPack {
            name: "meshes\\armor\\helmet.nif".to_string(),
            data: b"fake nif bytes for the registry test".to_vec(),
        }];
        let mut buf = Vec::new();
        write(Cursor::new(&mut buf), &files, &options()).unwrap();
        buf
    }

    fn make_dx10_bytes() -> Vec<u8> {
        let textures = vec![TextureToPack {
            name: "textures\\armor\\helmet.dds".to_string(),
            height: 64,
            width: 64,
            num_mips: 1,
            format: 71,
            data: b"fake dds mip bytes for the registry test".to_vec(),
        }];
        let mut buf = Vec::new();
        write_dx10(Cursor::new(&mut buf), &textures, &options()).unwrap();
        buf
    }

    #[test]
    fn registry_with_ba2_format_opens_a_real_gnrl_archive() {
        let mut registry = Registry::new();
        registry.register(Box::new(Ba2Format));

        let bytes = make_gnrl_bytes();
        let mut handle = registry
            .open(Box::new(Cursor::new(bytes)))
            .expect("registry must dispatch to Ba2Format");

        assert_eq!(handle.format_name(), "ba2");
        let entries = handle.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "meshes\\armor\\helmet.nif");

        let content = handle.read_file_at(0).unwrap();
        assert_eq!(content, b"fake nif bytes for the registry test");
    }

    #[test]
    fn registry_with_ba2_format_opens_a_real_dx10_archive_and_concatenates_chunks() {
        let mut registry = Registry::new();
        registry.register(Box::new(Ba2Format));

        let bytes = make_dx10_bytes();
        let mut handle = registry
            .open(Box::new(Cursor::new(bytes)))
            .expect("registry must dispatch to Ba2Format");

        let entries = handle.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "textures\\armor\\helmet.dds");
        assert_eq!(
            entries[0].size,
            b"fake dds mip bytes for the registry test".len() as u64
        );

        let content = handle.read_file_at(0).unwrap();
        assert_eq!(content, b"fake dds mip bytes for the registry test");
    }

    #[test]
    fn registry_with_bsa_and_ba2_formats_dispatches_each_to_its_own_handler() {
        let mut registry = Registry::new();
        registry.register(Box::new(modfather_bsa::container::BsaFormat));
        registry.register(Box::new(Ba2Format));

        let ba2_bytes = make_gnrl_bytes();
        let handle = registry.open(Box::new(Cursor::new(ba2_bytes))).unwrap();
        assert_eq!(handle.format_name(), "ba2");
    }
}
