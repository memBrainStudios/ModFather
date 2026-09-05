//! `sevenzip-re` — clean-room, standalone Rust implementation of the 7z
//! container format and its core codecs.
//!
//! This crate is one half of "7-Zip RE" (see `docs/VESTIBULE.md` in the
//! ModFather repository). It contains:
//!
//! - A full parser/writer for the 7z container (`SignatureHeader`,
//!   `PackInfo`, `Folder`/`UnpackInfo`, `SubStreamsInfo`, `FilesInfo`,
//!   including compressed (`kEncodedHeader`) headers).
//! - Native codecs: **Copy**, **LZMA**, **LZMA2** (via the pure-Rust
//!   `lzma-rs` crate). No shelling out to a system `7z`/`7za` binary.
//!
//! It contains **zero Bethesda-specific code**. BSA and BA2 are Bethesda's
//! own archive formats, not 7z; they live in separate extension crates
//! (`modfather-bsa`, `modfather-ba2`) that register as additional
//! [`container::ContainerFormat`] handlers alongside 7z (see the
//! [`container`] module) rather than being folded into this crate.
//!
//! RAR is a placeholder pending a license and is not implemented here.
//! When a license is secured, RAR slots into the same [`container`]
//! registry mechanism as one more [`container::ContainerFormat`] --
//! nothing about this crate's public API needs to change for that.

pub mod archive;
pub mod codec;
pub mod container;
pub mod error;
pub mod format;
pub mod header;
pub mod varint;

pub use archive::{create, Archive, Entry, NewEntry};
pub use codec::PackCodec;
pub use container::{ContainerEntry, ContainerError, ContainerFormat, ContainerHandle, ContainerResult, Registry};
pub use error::{Error, Result};
