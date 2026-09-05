//! Error type for the BSA extension crate.

use std::io;

/// Errors returned by `modfather-bsa`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Underlying I/O failure.
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    /// The 4-byte magic did not match a known BSA family.
    ///
    /// Shared across both format families this crate implements: Morrowind
    /// ([`crate::tes3`], magic `0x0000_0100`) and Oblivion-through-Skyrim
    /// ([`crate::tes4`], magic `b"BSA\0"`). The two magics are disjoint, so
    /// a single probe safely tells them apart -- see `crate::container` for
    /// the [`sevenzip_re::container::Registry`] dispatch that relies on
    /// this.
    #[error("not a BSA archive: bad signature")]
    BadSignature,

    /// Archive declares a version this crate does not understand.
    ///
    /// Only applies to the [`crate::tes4`] family: v103 (Oblivion / Fallout
    /// 3 / New Vegas), v104 (Skyrim LE / Fallout 4 pre-Next-Gen), and v105
    /// (Skyrim SE/AE) are implemented. [`crate::tes3`] (Morrowind) BSA has
    /// no version field at all -- its header's second `u32` is a byte
    /// offset (`hashOffset`), not a version -- so this variant never
    /// applies to it; a corrupt/foreign TES3 file that fails to parse
    /// surfaces as [`Error::Malformed`] instead.
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

    /// [`crate::tes3`] writer only: two distinct paths hashed to the same
    /// 64-bit value. UESP's "Morrowind Mod:BSA File Format" notes real
    /// Morrowind tooling treats this as a hard error rather than silently
    /// overwriting one entry -- this crate's writer matches that behavior
    /// instead of producing an archive with a lost file.
    #[error("hash collision between {0:?} and {1:?} (hash {2:#018x})")]
    HashCollision(String, String, u64),
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;
