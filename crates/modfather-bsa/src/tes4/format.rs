//! BSA v103-105 container layout constants.
//!
//! Reference: UESP wiki "Skyrim Mod:Archive File Format" (BSA), cross-checked
//! against Oblivion/FO3/FNV (v103) and Skyrim LE/FO4 (v104) archives.

/// 4-byte magic at the start of every BSA file.
pub const MAGIC: [u8; 4] = [b'B', b'S', b'A', 0];

/// Fixed header size (through the end of the `fileFlags`/padding fields).
pub const HEADER_SIZE: u64 = 36;

/// `archiveFlags` bits.
pub mod archive_flags {
    /// Folder names are stored in the folder-records block.
    pub const INCLUDE_DIR_NAMES: u32 = 1 << 0;
    /// File names are stored in the file-names block.
    pub const INCLUDE_FILE_NAMES: u32 = 1 << 1;
    /// Archive's *default* compression state for every file (files can
    /// invert this individually via the 0x4000_0000 size bit).
    pub const COMPRESSED_ARCHIVE: u32 = 1 << 2;
    /// Xbox 360 archive: big-endian numeric fields throughout.
    pub const XBOX_360_ARCHIVE: u32 = 1 << 6;
    /// Per-file compressed-size-with-name-prefix flag (embed file name
    /// before the data of each compressed file record).
    pub const EMBED_FILE_NAMES: u32 = 1 << 8;
}

/// Bit that, when set on a file record's `size` field, inverts that file's
/// compression relative to the archive's `COMPRESSED_ARCHIVE` default.
pub const FILE_SIZE_COMPRESSION_INVERT_BIT: u32 = 1 << 30;

/// Mask to strip the inversion bit and get the real (possibly-compressed)
/// on-disk size.
pub const FILE_SIZE_MASK: u32 = !FILE_SIZE_COMPRESSION_INVERT_BIT;
