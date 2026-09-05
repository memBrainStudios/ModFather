//! BSA v103-105 (Oblivion through Skyrim SE/AE) -- the "TES4-and-later"
//! BSA family: versioned, compressible (zlib/LZ4), folder+file hash-sorted
//! layout. See [`crate::tes3`] for the older, structurally distinct
//! Morrowind format this family replaced.
//!
//! Named `tes4` because Oblivion (TES IV) introduced this layout; Fallout
//! 3/New Vegas (v103), Skyrim LE/Fallout 4 pre-Next-Gen (v104), and Skyrim
//! SE/AE (v105) are all the same family, differing only in the codec used
//! for compressed payloads and a few padding fields -- not a format
//! rewrite the way Morrowind -> Oblivion was.

pub mod format;
pub mod hash;
pub mod reader;
pub mod writer;

pub use reader::{BsaArchive, BsaEntry};
pub use writer::{write, FileToPack, WriteOptions};
