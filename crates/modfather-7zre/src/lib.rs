//! `modfather-7zre` — the Vestibule-facing integration layer for 7-Zip RE.
//!
//! Per the project's architecture decision: "7-Zip RE is responsible for
//! all archives related to this project... Vestibule is a client of
//! 7-Zip RE... at no time does Vestibule implement anything related to an
//! archive." `sevenzip-re` itself must stay free of Bethesda-specific code
//! (so a consumer who only wants general-purpose 7z support is never
//! forced to pull in BSA/BA2), which means `sevenzip-re` cannot be the
//! crate that assembles a registry containing `modfather-bsa`/
//! `modfather-ba2` handlers, or owns Bethesda pack-back naming. This crate
//! is that missing layer: it depends on `sevenzip-re` plus every Bethesda
//! extension crate, and owns everything that requires knowing about more
//! than one of them at once. `modfather-vestibule` depends on this crate
//! and never depends on `sevenzip-re`/`modfather-bsa`/`modfather-ba2`
//! directly, which is what keeps Vestibule genuinely free of
//! archive-format logic rather than just organizationally adjacent to it.
//!
//! One-way custody chain (unchanged, now with this crate inserted):
//! `sevenzip-re` + Bethesda extensions -> **`modfather-7zre`** ->
//! `modfather-vestibule` -> Crucible -> ModFather.
//!
//! Modules:
//! - [`container`]: assembles the one shared
//!   [`sevenzip_re::container::Registry`] with every implemented format
//!   registered (7z, both BSA generations, BA2), magic-byte-dispatched.
//! - [`packing`]: packs a MOD's loose files back into BSA/BA2 archives
//!   using Bethesda's Main/Textures naming split.
//! - [`dds`]: minimal DDS header parser feeding BA2's DX10 texture packer
//!   (deliberately not a full DDS/DXGI implementation — see its own doc
//!   comment).
//!
//! What this crate is *not*: it is not a live, mountable filesystem view
//! of an archive's contents ("read and write like the archive is just
//! another folder" from the architecture decision). That capability is
//! still to be designed on top of [`container::Registry`]'s existing
//! list/extract API and is tracked as follow-up work, not implemented
//! here yet.

pub mod container;
pub mod dds;
pub mod packing;
