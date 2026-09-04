//! `modfather-vestibule` — Wave 0 floor: VFS roots, last-wins layering with
//! explicit picks, compare tooltips.
//!
//! Depends on the `sevenzip-re` standalone package plus the Bethesda
//! archive extension crates (`modfather-bsa`, `modfather-ba2`); a consumer
//! who only needs general-purpose 7z support is not forced to pull in
//! Bethesda format code (that dependency direction is enforced by this
//! crate depending *down* on the extensions, never the other way around).
//!
//! Status: Wave 0. [`layering::resolve`] implements last-wins layering plus
//! explicit 1:1 picks (the "Bash" behavior in `docs/VESTIBULE.md`) and is
//! unit-tested, including the Wave 0 gate's exact requirement: "a manual
//! conflict pick overrides last-wins on one path" without affecting any
//! other path. [`vfs::VfsRoots`] models the three real on-disk roots
//! (install, save/config, Data). [`packing`] implements the Main/Textures
//! archive-naming split from `docs/VESTIBULE.md`'s Pull step 3
//! (`{stem}.bsa`/`{stem} - Textures.bsa`, `{stem} - Main.ba2`/
//! `{stem} - Textures.ba2`) on top of `modfather-bsa`'s and
//! `modfather-ba2`'s writers. [`loot`] implements the generalized-sorter
//! stub (masterlist + Nexus categories + user rules) named in
//! `docs/VESTIBULE.md`. Real pull-pipeline wiring (`download-repo` ->
//! `VCS`) is tracked as follow-up work.

pub mod layering;
pub mod loot;
pub mod packing;
pub mod vfs;

/// Re-exported so downstream crates can name the standalone engine and the
/// Bethesda extensions through a single `modfather-vestibule` dependency
/// during early Wave 0 work, without pinning to their internal module paths.
pub use modfather_ba2 as ba2;
pub use modfather_bsa as bsa;
pub use sevenzip_re as sevenzip;
