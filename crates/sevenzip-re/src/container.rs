//! Modular container-format registry: a GoF **Strategy** (one
//! [`ContainerFormat`] + [`ContainerHandle`] pair per archive format) plus
//! **Factory** ([`Registry`] probes magic bytes and dispatches to the
//! matching strategy) pattern.
//!
//! This exists to make real the "container registry" this project's own
//! docs already describe in prose (`README.md`: "BSA ... and BA2 ... plug
//! into 7-Zip RE's container registry"; `docs/VESTIBULE.md`: "registered
//! as additional container handlers alongside 7z") and its own stated
//! Principle ("Traits in the center, editions at the edge (SOLID)."),
//! neither of which had an actual trait or registry behind them before
//! this module -- [`crate::archive::Archive`] was, and remains, a
//! concrete, 7z-only type; it does not itself change.
//!
//! `sevenzip-re` deliberately ships only:
//! 1. The trait pair ([`ContainerFormat`], [`ContainerHandle`]) every
//!    format implements once (its "modular payload").
//! 2. [`Registry`], the probe-and-dispatch mechanism, starting **empty**.
//! 3. [`SevenZipFormat`]/[`SevenZipHandle`], the 7z format's own payload,
//!    since `Archive` already lives here.
//!
//! It never registers BSA/BA2 (or, once licensed, RAR) itself: that would
//! require `sevenzip-re` to depend on the Bethesda extension crates,
//! inverting the one-way custody chain (`7-Zip RE -> Vestibule -> Crucible
//! -> ModFather`, `docs/SCHEDULE.md`). Each extension crate instead
//! implements this same trait pair for its own archive type (see
//! `modfather_bsa::container`, `modfather_ba2::container`), and
//! `modfather-vestibule` -- the crate that already depends on all of
//! them -- is where one shared [`Registry`] gets every format registered
//! into it (see `modfather_vestibule::container::build_registry`). A
//! future RAR crate slots in the same way once a license is secured:
//! implement the trait pair, register it, and every existing caller of
//! `Registry::open` picks it up with zero code changes on their end --
//! that is the whole point of the pattern.

use std::io::{Read, Seek, SeekFrom};

/// Blanket-implemented for any type that is both [`Read`] and [`Seek`], so
/// container handlers can be written once against a single trait object
/// (`Box<dyn ReadSeek>`) instead of every format needing its own reader
/// generic threaded through the registry API.
pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

// `Box<dyn ReadSeek>` itself satisfies `Read + Seek` via `std`'s own
// blanket `impl<R: Read + ?Sized> Read for Box<R>` / `impl<S: Seek +
// ?Sized> Seek for Box<S>`, so it can be threaded through
// `Archive<R: Read + Seek>` and friends without every format's `open()`
// signature needing its own reader generic.

/// One archive entry as seen through the registry -- deliberately smaller
/// than any single format's own entry type (e.g. [`crate::archive::Entry`],
/// `modfather_bsa::BsaEntry`, `modfather_ba2::Ba2Entry`), which callers
/// that need format-specific fields should keep using directly; this is
/// only the common subset a format-agnostic caller (like a VFS listing)
/// needs.
#[derive(Debug, Clone)]
pub struct ContainerEntry {
    /// Full path within the archive (folder + name already joined, using
    /// `\` as the separator to match Bethesda archive conventions).
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
    /// Per-entry CRC, when the format exposes one. `None` if the format
    /// does not track a per-entry CRC (e.g. BSA, BA2) or the entry is a
    /// directory placeholder.
    pub crc: Option<u32>,
}

/// Errors from probing/opening/reading through the registry. Wraps each
/// format's own concrete error as a string (via `Display`/`ToString`)
/// rather than depending on every format crate's error type here, which
/// would defeat the point of `sevenzip-re` not depending on the Bethesda
/// extension crates.
#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
    /// No registered [`ContainerFormat`] recognized the input's magic
    /// bytes.
    #[error("no registered container format recognized this input")]
    NoMatchingFormat,
    /// Underlying I/O failure while peeking header bytes or reading.
    #[error("io error: {0}")]
    Io(String),
    /// The matched format's own `open`/`read_file_at` failed.
    #[error("{0}")]
    Format(String),
}

pub type ContainerResult<T> = std::result::Result<T, ContainerError>;

/// An opened archive, accessed only through this trait by
/// format-agnostic callers (the GoF Strategy object itself, already bound
/// to one concrete format by [`ContainerFormat::open`]).
pub trait ContainerHandle {
    /// Short format tag (`"7z"`, `"bsa"`, `"ba2"`, ...), for logging/UI.
    fn format_name(&self) -> &'static str;
    /// List every entry in the archive.
    fn entries(&self) -> Vec<ContainerEntry>;
    /// Read and decode one entry's bytes by its index into [`Self::entries`].
    fn read_file_at(&mut self, idx: usize) -> ContainerResult<Vec<u8>>;
}

/// One pluggable container format: a GoF Strategy interface (what makes
/// this format recognizable and openable) doubling as a Factory method
/// (`open` manufactures the concrete [`ContainerHandle`]). Implement this
/// once per archive format and [`Registry::register`] it; callers of
/// [`Registry::open`] never match on format themselves again.
pub trait ContainerFormat: Send + Sync {
    /// Short format tag, matching the handle it opens.
    fn format_name(&self) -> &'static str;
    /// How many leading bytes of the stream [`Self::probe`] needs to see.
    fn probe_len(&self) -> usize;
    /// Does `header` (exactly [`Self::probe_len`] bytes peeked from the
    /// start of the stream, not consumed from any live reader) match this
    /// format's magic?
    fn probe(&self, header: &[u8]) -> bool;
    /// Open `reader` (already rewound to the start of the stream by the
    /// caller/[`Registry`]) as this format.
    fn open(&self, reader: Box<dyn ReadSeek>) -> ContainerResult<Box<dyn ContainerHandle>>;
}

/// The registry itself (GoF Factory): holds every registered
/// [`ContainerFormat`] and dispatches [`Registry::open`] to whichever
/// one's [`ContainerFormat::probe`] recognizes the input -- the mechanism
/// this project's docs call "7-Zip RE's container registry" /
/// "additional container handlers alongside 7z". Starts empty; see this
/// module's own doc comment for why `sevenzip-re` never pre-registers
/// BSA/BA2 itself.
#[derive(Default)]
pub struct Registry {
    formats: Vec<Box<dyn ContainerFormat>>,
}

impl Registry {
    /// A registry with no formats registered yet.
    pub fn new() -> Self {
        Registry {
            formats: Vec::new(),
        }
    }

    /// Register one more format. Registration order breaks ties if two
    /// formats' magics could ever overlap (none of 7z/BSA/BA2's do).
    pub fn register(&mut self, format: Box<dyn ContainerFormat>) -> &mut Self {
        self.formats.push(format);
        self
    }

    /// How many formats are currently registered.
    pub fn len(&self) -> usize {
        self.formats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.formats.is_empty()
    }

    /// The largest [`ContainerFormat::probe_len`] across every registered
    /// format -- how many header bytes [`Self::open`] needs to peek.
    fn max_probe_len(&self) -> usize {
        self.formats.iter().map(|f| f.probe_len()).max().unwrap_or(0)
    }

    /// Peek up to [`Self::max_probe_len`] bytes from the start of
    /// `reader`, rewind it back to the start (so the matched format's own
    /// `open` sees an untouched stream), probe every registered format in
    /// registration order, and open with the first match.
    pub fn open(&self, mut reader: Box<dyn ReadSeek>) -> ContainerResult<Box<dyn ContainerHandle>> {
        let probe_len = self.max_probe_len();
        let mut header = vec![0u8; probe_len];
        let mut filled = 0usize;
        while filled < probe_len {
            match reader.read(&mut header[filled..]) {
                Ok(0) => break, // shorter than probe_len: fine, just won't match anything needing more.
                Ok(n) => filled += n,
                Err(e) => return Err(ContainerError::Io(e.to_string())),
            }
        }
        header.truncate(filled);

        reader
            .seek(SeekFrom::Start(0))
            .map_err(|e| ContainerError::Io(e.to_string()))?;

        for format in &self.formats {
            let need = format.probe_len();
            if header.len() >= need && format.probe(&header[..need]) {
                return format.open(reader);
            }
        }
        Err(ContainerError::NoMatchingFormat)
    }
}

/// The 7z format's own [`ContainerFormat`] payload. Lives here (rather
/// than in a separate module) since [`crate::archive::Archive`] already
/// does.
pub struct SevenZipFormat;

impl ContainerFormat for SevenZipFormat {
    fn format_name(&self) -> &'static str {
        "7z"
    }

    fn probe_len(&self) -> usize {
        crate::format::SIGNATURE.len()
    }

    fn probe(&self, header: &[u8]) -> bool {
        header == crate::format::SIGNATURE.as_slice()
    }

    fn open(&self, reader: Box<dyn ReadSeek>) -> ContainerResult<Box<dyn ContainerHandle>> {
        let archive = crate::archive::Archive::from_reader(reader)
            .map_err(|e| ContainerError::Format(format!("7z: {e}")))?;
        Ok(Box::new(SevenZipHandle(archive)))
    }
}

struct SevenZipHandle(crate::archive::Archive<Box<dyn ReadSeek>>);

impl ContainerHandle for SevenZipHandle {
    fn format_name(&self) -> &'static str {
        "7z"
    }

    fn entries(&self) -> Vec<ContainerEntry> {
        self.0
            .entries()
            .into_iter()
            .map(|e| ContainerEntry {
                name: e.name,
                size: e.size,
                is_dir: e.is_dir,
                crc: e.crc,
            })
            .collect()
    }

    fn read_file_at(&mut self, idx: usize) -> ContainerResult<Vec<u8>> {
        self.0
            .read_file_at(idx)
            .map_err(|e| ContainerError::Format(format!("7z: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{create, NewEntry};
    use crate::codec::PackCodec;
    use std::io::Cursor;

    fn make_7z_bytes() -> Vec<u8> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.7z");
        let entries = vec![
            NewEntry {
                name: "hello.txt".to_string(),
                data: b"Hello, registry!".to_vec(),
            },
            NewEntry {
                name: "dir/nested.txt".to_string(),
                data: b"Nested, via the registry.".to_vec(),
            },
        ];
        create(&path, &entries, PackCodec::Lzma2).unwrap();
        std::fs::read(&path).unwrap()
    }

    #[test]
    fn registry_with_only_7z_opens_a_real_7z_archive() {
        let mut registry = Registry::new();
        registry.register(Box::new(SevenZipFormat));

        let bytes = make_7z_bytes();
        let mut handle = registry
            .open(Box::new(Cursor::new(bytes)))
            .expect("registry must dispatch to SevenZipFormat");

        assert_eq!(handle.format_name(), "7z");
        let entries = handle.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "hello.txt");

        let content = handle.read_file_at(0).unwrap();
        assert_eq!(content, b"Hello, registry!");
    }

    #[test]
    fn registry_rejects_input_no_registered_format_recognizes() {
        let mut registry = Registry::new();
        registry.register(Box::new(SevenZipFormat));

        let bytes = b"not a 7z archive at all".to_vec();
        let result = registry.open(Box::new(Cursor::new(bytes)));

        assert!(matches!(result, Err(ContainerError::NoMatchingFormat)));
    }

    #[test]
    fn empty_registry_always_rejects() {
        let registry = Registry::new();
        assert!(registry.is_empty());

        let bytes = make_7z_bytes();
        let result = registry.open(Box::new(Cursor::new(bytes)));
        assert!(matches!(result, Err(ContainerError::NoMatchingFormat)));
    }
}
