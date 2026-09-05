//! Morrowind (TES III) BSA name-hash algorithm -- a different, older
//! algorithm from [`crate::tes4::hash`]'s Tes4/Tes5 hash, computed over a
//! file's full relative path (folder + name together) rather than name
//! and folder separately: Morrowind BSA has no folder records at all
//! (see [`crate::tes3::format`]'s module doc comment), so there is
//! nothing to hash separately.
//!
//! Reference: UESP wiki "Morrowind Mod:BSA File Format"'s C# listing
//! (transcribed 1:1 below, `u32`-wrapping arithmetic made explicit).
//! Independently cross-checked against the `ba2` crate's own
//! `tes3::hash_file_in_place` implementation and its published
//! known-answer vectors (see this module's tests) -- both agree exactly,
//! which is strong evidence this transcription is correct rather than
//! merely self-consistent.

/// Compute the 64-bit Morrowind BSA hash for a full relative path (e.g.
/// `meshes\armor\cuirass.nif`). Case-insensitive and slash-normalizing:
/// the caller does not need to pre-normalize.
pub fn hash_path(path: &str) -> u64 {
    let normalized = path.replace('/', "\\").to_lowercase();
    let bytes = normalized.as_bytes();
    let len = bytes.len();
    let midpoint = len / 2;

    let mut lo: u32 = 0;
    for (i, &b) in bytes.iter().enumerate().take(midpoint) {
        lo ^= (b as u32) << ((i % 4) * 8);
    }

    let mut hi: u32 = 0;
    for (i, &b) in bytes.iter().enumerate().skip(midpoint) {
        let shift = ((i - midpoint) % 4) * 8;
        let temp = (b as u32) << shift;
        let rot = temp & 0x1F;
        hi = (hi ^ temp).rotate_right(rot);
    }

    ((lo as u64) << 32) | (hi as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Known-answer vectors published by the independently-written `ba2`
    // crate's own `tes3::hashing` unit tests (`validate_hashing`) -- using
    // a *different* independent implementation's own test data (rather
    // than only re-deriving from the same UESP listing this module
    // transcribes) is a materially stronger check than a same-source
    // round trip.
    #[test]
    fn matches_ba2_crates_published_known_answer_vectors() {
        assert_eq!(
            hash_path("meshes/c/artifact_bloodring_01.nif"),
            0x1C3C1149920D5F0C
        );
        assert_eq!(
            hash_path("meshes/x/ex_stronghold_pylon00.nif"),
            0x20250749ACCCD202
        );
        assert_eq!(hash_path("meshes/r/xsteam_centurions.kf"), 0x6E5C0F3125072EA6);
        assert_eq!(hash_path("textures/tx_rock_cave_mu_01.dds"), 0x58060C2FA3D8F759);
        assert_eq!(hash_path("meshes/f/furn_ashl_chime_02.nif"), 0x7C3B2F3ABFFC8611);
        assert_eq!(hash_path("textures/tx_rope_woven.dds"), 0x5865632F0C052C64);
        assert_eq!(hash_path("icons/a/tx_templar_skirt.dds"), 0x46512A0B60EDA673);
        assert_eq!(hash_path("icons/m/misc_prongs00.dds"), 0x51715677BBA837D3);
        assert_eq!(
            hash_path("meshes/i/in_c_stair_plain_tall_02.nif"),
            0x2A324956BF89B1C9
        );
        assert_eq!(hash_path("meshes/r/xkwama worker.nif"), 0x6D446E352C3F5A1E);
    }

    #[test]
    fn forward_slashes_and_backslashes_hash_the_same() {
        assert_eq!(hash_path("foo/bar/baz"), hash_path("foo\\bar\\baz"));
    }

    #[test]
    fn hashing_is_case_insensitive() {
        assert_eq!(hash_path("FOO/BAR/BAZ"), hash_path("foo/bar/baz"));
    }

    #[test]
    fn different_paths_hash_differently() {
        assert_ne!(hash_path("meshes\\a.nif"), hash_path("meshes\\b.nif"));
    }
}
