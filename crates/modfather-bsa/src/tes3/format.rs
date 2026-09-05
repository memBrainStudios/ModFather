//! Morrowind (TES III) BSA container layout constants.
//!
//! This is a **structurally distinct, older format** from the v103-105
//! family in [`crate::tes4`] -- not a variant reachable by branching
//! inside that reader/writer. It predates folders-as-records, per-file
//! compression, and even a real version field: the "magic number" every
//! source below calls out (`0x00000100`) is Morrowind's own hard-coded
//! sentinel, not a version that ever incremented again -- Oblivion (TES4)
//! replaced the whole layout rather than extending this one, which is
//! exactly why this crate keeps it in its own module instead of folding
//! it into `tes4`'s version-aware dispatch.
//!
//! References (independently cross-checked against each other, plus the
//! `ba2` crate's own `tes3` module and its published known-answer hash
//! vectors -- see `tes3::hash`'s tests):
//! - UESP wiki, "Morrowind Mod:BSA File Format" (the six-section layout,
//!   the hash algorithm's C++/C# listings).
//! - `niftools/nifskope`'s `lib/fsengine/bsa.{h,cpp}` (`MWBSAHeader`,
//!   `MWBSAFileSizeOffset`, and the real reader code computing
//!   `dataOffset = 12 + HashOffset + FileCount * 8`), BSD-licensed, used
//!   here purely as a format oracle, not vendored code.

/// Morrowind's magic **number**, not a byte string like TES4's `BSA\0` --
/// the literal little-endian bytes of `0x0000_0100`.
pub const MAGIC: [u8; 4] = [0x00, 0x01, 0x00, 0x00];

/// Fixed header size: magic(4) + hashOffset(4) + fileCount(4).
pub const HEADER_SIZE: u64 = 12;

/// Size of one (size, offset) file-size/offset table entry.
pub const FILE_SIZE_OFFSET_ENTRY_SIZE: u64 = 8;

/// Size of one name-offset table entry.
pub const NAME_OFFSET_ENTRY_SIZE: u64 = 4;

/// Size of one filename-hash table entry (two `u32`s: lo, hi).
pub const HASH_ENTRY_SIZE: u64 = 8;
