//! Error type for the BSA extension crate.

use std::io;

/// Errors returned by `modfather-bsa`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Underlying I/O failure.
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    /// The 4-byte magic did not read `BSA\0`.
    #[error("not a BSA archive: bad signature")]
    BadSignature,

    /// Archive declares a version this crate does not understand.
    ///
    /// Only v103 (Oblivion / Fallout 3 / New Vegas), v104 (Skyrim LE /
    /// Fallout 4 pre-Next-Gen), and v105 (Skyrim SE/AE) are implemented.
    /// TES3 (Morrowind) BSA is a materially different, older format and is
    /// explicitly out of scope until the user decides otherwise (see
    /// `docs/SCHEDULE.md`'s Wave 0 "TES3 (Morrowind) BSA scope" note for
    /// the two options awaiting that decision).
    #[error("unsupported BSA version: {0}")]
    UnsupportedVersion(u32),

    /// The header or a sub-structure was truncated or malformed.
    #[error("malformed BSA: {0}")]
    Malformed(String),

    /// zlib (v103/v104) decompression failure.
    #[error("zlib decompress error: {0}")]
    Zlib(String),

    /// LZ4 (v105) decompression failure.
    #[error("lz4 decompress error: {0}")]
    Lz4(String),

    /// Requested file/folder does not exist in the archive.
    #[error("no such entry: {0}")]
    NoSuchEntry(String),
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;
