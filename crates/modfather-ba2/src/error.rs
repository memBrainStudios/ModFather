//! Error type for the BA2 extension crate.

use std::io;

/// Errors returned by `modfather-ba2`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Underlying I/O failure.
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    /// The magic did not read `BTDX`.
    #[error("not a BA2 archive: bad signature")]
    BadSignature,

    /// Archive type tag was neither `GNRL` nor `DX10`.
    #[error("unsupported BA2 type tag: {0}")]
    UnsupportedType(String),

    /// The header or a sub-structure was truncated or malformed.
    #[error("malformed BA2: {0}")]
    Malformed(String),

    /// Requested file does not exist in the archive.
    #[error("no such entry: {0}")]
    NoSuchEntry(String),
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;
