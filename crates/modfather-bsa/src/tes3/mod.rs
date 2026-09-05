//! Morrowind (TES III) BSA -- the oldest BSA format and, per the project's
//! own architecture principle, the "ground truth" this whole crate's
//! version-partitioning is built around: every later BSA generation
//! ([`crate::tes4`]) either would have built on this layout or, as actually
//! happened with Oblivion, replaced it outright. Because Oblivion chose
//! "replace" rather than "extend" (no folder records, no compression, no
//! version field, a completely different hash algorithm), this format gets
//! its own module and its own reader/writer/hash/format quartet rather than
//! a branch inside `tes4`'s version-aware dispatch -- see `tes3::format`'s
//! doc comment for the full case.
//!
//! Distinguishing which family a given `.bsa` file belongs to is a pure
//! magic-byte check: TES3's magic is the 4 little-endian bytes of
//! `0x0000_0100` ([`format::MAGIC`]); TES4-and-later's is the byte string
//! `b"BSA\0"` ([`crate::tes4::format::MAGIC`]). The two never collide, so
//! `crate::container` can safely register both
//! [`sevenzip_re::container::ContainerFormat`] strategies in the same
//! [`sevenzip_re::container::Registry`] and let header-probing pick the
//! right one -- no file extension or user hint required.

pub mod format;
pub mod hash;
pub mod reader;
pub mod writer;

pub use hash::hash_path;
pub use reader::{Tes3Archive, Tes3Entry};
pub use writer::{write, Tes3FileToPack};
