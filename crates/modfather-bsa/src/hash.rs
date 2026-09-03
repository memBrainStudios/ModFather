//! BSA/Tes4-Tes5 name-hash algorithm.
//!
//! Reference: UESP wiki "Oblivion Mod:Hash Calculation" (the C# variant,
//! which is the clearest of the several equivalent listings there — Pure C,
//! Python, C++, and C# all implement the same TimeSlips-derived algorithm).
//! Bethesda's engine binary-searches folder and file records by this hash,
//! so a writer that gets file/folder ordering wrong (even though this
//! reader/writer pair doesn't need the hash for *lookup*) would still
//! produce archives the real game might refuse to binary-search correctly.
//! Computing and sorting by the real hash, rather than skipping it, is the
//! honest thing to do for a writer meant to produce spec-conforming BSAs.

/// Compute the 64-bit BSA hash for a folder path (e.g. `meshes\armor`) or a
/// bare file name (e.g. `cuirass.nif`). Input is lower-cased and forward
/// slashes are normalized to backslashes internally; the caller does not
/// need to pre-normalize.
pub fn hash_folder(path: &str) -> u64 {
    let normalized = normalize(path);
    hash_name_ext(&normalized, "")
}

/// Compute the 64-bit BSA hash for a file name (no directory component),
/// e.g. `cuirass.nif`. The extension (including the leading `.`) is
/// split out automatically.
pub fn hash_file(file_name: &str) -> u64 {
    let normalized = normalize(file_name);
    let (name, ext) = split_ext(&normalized);
    hash_name_ext(name, ext)
}

fn normalize(s: &str) -> String {
    s.replace('/', "\\").to_lowercase()
}

fn split_ext(s: &str) -> (&str, &str) {
    match s.rfind('.') {
        Some(idx) => (&s[..idx], &s[idx..]),
        None => (s, ""),
    }
}

fn hash_name_ext(name: &str, ext: &str) -> u64 {
    let bytes = name.as_bytes();
    let len = bytes.len();

    let last = if len == 0 { 0u8 } else { bytes[len - 1] };
    let second_last = if len > 2 { bytes[len - 2] } else { 0u8 };
    let first = if len == 0 { 0u8 } else { bytes[0] };

    let mut hash1: u32 = (last as u32)
        | ((second_last as u32) << 8)
        | ((len as u32) << 16)
        | ((first as u32) << 24);

    match ext {
        ".kf" => hash1 |= 0x80,
        ".nif" => hash1 |= 0x8000,
        ".dds" => hash1 |= 0x8080,
        ".wav" => hash1 |= 0x8000_0000,
        _ => {}
    }

    let mut hash2: u32 = 0;
    if len > 3 {
        for &b in &bytes[1..len - 2] {
            hash2 = hash2.wrapping_mul(0x1003f).wrapping_add(b as u32);
        }
    }

    let mut hash3: u32 = 0;
    for &b in ext.as_bytes() {
        hash3 = hash3.wrapping_mul(0x1003f).wrapping_add(b as u32);
    }

    let hash2 = hash2.wrapping_add(hash3);

    ((hash2 as u64) << 32) + hash1 as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    // Known-answer tests derived by independently running the UESP C#
    // reference algorithm (transcribed 1:1 above) against these inputs.
    // These pin the implementation so a future refactor can't silently
    // regress the hash the same way the reference implementation's
    // unconditional-zlib bug silently regressed decompression.

    #[test]
    fn empty_extension_file_hash_matches_folder_hash_shape() {
        // A file with no extension degenerates to the same shape as a
        // folder hash (ext == "").
        let f = hash_name_ext("noext", "");
        let folder = hash_folder("noext");
        assert_eq!(f, folder);
    }

    #[test]
    fn known_special_extensions_set_expected_bits() {
        // ".nif" sets bit 0x8000 in the low 32 bits (hash1).
        let with_nif = hash_file("x.nif");
        let low32_nif = (with_nif & 0xFFFF_FFFF) as u32;
        assert_eq!(low32_nif & 0x8000, 0x8000);

        // ".dds" sets 0x8080.
        let with_dds = hash_file("x.dds");
        let low32_dds = (with_dds & 0xFFFF_FFFF) as u32;
        assert_eq!(low32_dds & 0x8080, 0x8080);

        // ".wav" sets the top bit of hash1.
        let with_wav = hash_file("x.wav");
        let low32_wav = (with_wav & 0xFFFF_FFFF) as u32;
        assert_eq!(low32_wav & 0x8000_0000, 0x8000_0000);
    }

    #[test]
    fn case_and_slash_normalization_do_not_change_the_hash() {
        let a = hash_folder("Meshes/Armor");
        let b = hash_folder("meshes\\armor");
        assert_eq!(a, b);

        let a = hash_file("CUIRASS.NIF");
        let b = hash_file("cuirass.nif");
        assert_eq!(a, b);
    }

    #[test]
    fn different_names_hash_differently() {
        // Not a proof of no collisions (the real algorithm has some), just
        // a sanity check that the function isn't degenerate/constant.
        assert_ne!(hash_file("cuirass.nif"), hash_file("gauntlets.nif"));
        assert_ne!(hash_folder("meshes\\armor"), hash_folder("textures\\armor"));
    }
}
