//! `modfather-vestibule` — Wave 0 floor: VFS roots, last-wins layering with
//! explicit picks, compare tooltips.
//!
//! Depends on the `sevenzip-re` standalone package plus the Bethesda
//! archive extension crates (`modfather-bsa`, `modfather-ba2`); a consumer
//! who only needs general-purpose 7z support is not forced to pull in
//! Bethesda format code (that dependency direction is enforced by this
//! crate depending *down* on the extensions, never the other way around).
//!
//! Status: Wave 0 scaffold. Real VFS-root and last-wins-layering logic is
//! tracked as immediate follow-up work (see `docs/SCHEDULE.md`, Wave 0
//! gate).

/// Re-exported so downstream crates can name the standalone engine and the
/// Bethesda extensions through a single `modfather-vestibule` dependency
/// during early Wave 0 work, without pinning to their internal module paths.
pub use modfather_ba2 as ba2;
pub use modfather_bsa as bsa;
pub use sevenzip_re as sevenzip;
