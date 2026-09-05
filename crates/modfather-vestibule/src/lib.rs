//! `modfather-vestibule` — Wave 0 floor: VFS roots, last-wins layering with
//! explicit picks, compare tooltips.
//!
//! **Vestibule is a client of 7-Zip RE. At no time does Vestibule
//! implement anything related to an archive.** Per the project's
//! architecture decision, all archive-format logic (the container
//! registry, BSA/BA2 pack-back naming, DDS metadata parsing for BA2
//! texture packing) has moved to `modfather-7zre`, the crate that sits
//! between the format crates (`sevenzip-re`, `modfather-bsa`,
//! `modfather-ba2`) and Vestibule in the one-way custody chain
//! (`7-Zip RE + extensions -> modfather-7zre -> Vestibule -> Crucible ->
//! ModFather`). This crate never depends on `sevenzip-re`,
//! `modfather-bsa`, or `modfather-ba2` directly in its own source; where
//! archive bytes are needed, it goes through `modfather-7zre`.
//!
//! Status: Wave 0. [`layering::resolve`] implements last-wins layering plus
//! explicit 1:1 picks (the "Bash" behavior in `docs/VESTIBULE.md`) and is
//! unit-tested, including the Wave 0 gate's exact requirement: "a manual
//! conflict pick overrides last-wins on one path" without affecting any
//! other path. [`vfs::VfsRoots`] models the three real on-disk roots
//! (install, save/config, Data). [`loot`] implements the generalized-sorter
//! stub (masterlist + Nexus categories + user rules) named in
//! `docs/VESTIBULE.md`. Real pull-pipeline wiring (`download-repo` ->
//! `VCS`) is tracked as follow-up work.
//!
//! MODs and MGEs (folder containers with `mod.ini`/`mge.ini`, per the
//! project's architecture decision -- "MODs are no longer considered an
//! archive") are Vestibule's proper domain but are not yet implemented;
//! tracked as follow-up work.
//!
//! `tests/wave0_gate.rs` proves this crate's [`layering`] composes with
//! `modfather-7zre`'s packing end to end (pack via `modfather-7zre`, feed
//! the resulting archives' contents into `layering::resolve`). That test
//! reads packed archives back with `modfather-bsa`'s/`modfather-ba2`'s own
//! readers purely to verify the packed bytes are real, spec-conforming
//! archives -- those crates are dev-dependencies only, never depended on
//! by this crate's own (non-test) code.

pub mod layering;
pub mod loot;
pub mod vfs;
