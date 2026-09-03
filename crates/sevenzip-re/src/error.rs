//! Error type for the standalone 7z engine.

use std::io;

/// Errors returned by `sevenzip-re`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Underlying I/O failure (opening/reading/writing the archive file).
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    /// The 6-byte magic signature did not match `7z\xBC\xAF\x27\x1C`.
    #[error("not a 7z archive: bad signature")]
    BadSignature,

    /// `StartHeaderCRC` did not match the CRC of the 20-byte `StartHeader`.
    #[error("corrupt archive: start header CRC mismatch")]
    StartHeaderCrcMismatch,

    /// `NextHeaderCRC` did not match the CRC of the decoded next-header bytes.
    #[error("corrupt archive: next header CRC mismatch")]
    NextHeaderCrcMismatch,

    /// A stream's CRC (pack stream or unpack sub-stream) did not match its declared digest.
    #[error("corrupt archive: stream CRC mismatch")]
    StreamCrcMismatch,

    /// The header (or a sub-structure) was truncated or malformed.
    #[error("malformed header: {0}")]
    Malformed(String),

    /// A coder (codec) referenced in a `Folder` is not implemented by this crate.
    ///
    /// `sevenzip-re` implements Copy, LZMA, and LZMA2. Everything else
    /// (including RAR, which is a placeholder pending license, and BSA/BA2,
    /// which are separate Bethesda extension crates) is out of scope here.
    #[error("unsupported codec: {0}")]
    UnsupportedCodec(String),

    /// LZMA/LZMA2 decode failure from the underlying `lzma-rs` decoder.
    #[error("lzma error: {0}")]
    Lzma(String),

    /// The requested entry (by name or index) does not exist in the archive.
    #[error("no such entry: {0}")]
    NoSuchEntry(String),
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

impl From<lzma_rs::error::Error> for Error {
    fn from(e: lzma_rs::error::Error) -> Self {
        Error::Lzma(e.to_string())
    }
}
