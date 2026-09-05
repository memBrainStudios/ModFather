//! BA2 (`BTDX`/FO4) name-hash algorithm.
//!
//! Format: `{ file: crc32_no_invert(lowercase(stem)), extension: first 4
//! raw bytes of the (lowercase) extension packed little-endian, directory:
//! crc32_no_invert(lowercase(parent_path)) }`, where `crc32_no_invert` is
//! the standard CRC-32 (IEEE 802.3, poly 0xEDB88320 reflected) generator
//! **without** the usual `0xFFFFFFFF` pre/post inversion — confirmed by
//! reproducing several of the `ba2` crate's own published known-answer
//! test vectors (docs.rs/ba2, `fo4::hash_file`) with an independently
//! written table here; nothing beyond those published input/output pairs
//! was taken from that crate.

/// Standard CRC-32 (IEEE 802.3) reflected table, generated from the
/// polynomial 0xEDB88320. This is the same well-known table used by
/// zlib/PNG/gzip CRC32 (and, incidentally, this crate's own `crc32fast`
/// dependency) — the only difference from a normal CRC32 call is that BA2
/// hashing skips the pre/post `0xFFFFFFFF` inversion.
fn crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for (i, entry) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *entry = c;
    }
    table
}

/// CRC-32 with no initial/final inversion, matching the FO4/BA2 hash's
/// underlying primitive (see module docs).
fn crc32_no_invert(bytes: &[u8]) -> u32 {
    let table = crc32_table();
    let mut crc: u32 = 0;
    for &b in bytes {
        crc = crc.wrapping_shr(8) ^ table[((crc ^ u32::from(b)) & 0xFF) as usize];
    }
    crc
}

/// The three-part BA2 file hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ba2Hash {
    pub file: u32,
    pub extension: u32,
    pub directory: u32,
}

/// Split a backslash-normalized, already-lowercased path into
/// (parent, stem, extension-without-dot).
fn split_path(path: &str) -> (&str, &str, &str) {
    let stem_pos = path.rfind('\\');
    let parent = stem_pos.map(|p| &path[..p]).unwrap_or("");
    let ext_pos = path.rfind('.');
    let extension = ext_pos.map(|p| &path[p + 1..]).unwrap_or("");
    let first = stem_pos.map(|p| p + 1).unwrap_or(0);
    let last = ext_pos.unwrap_or(path.len());
    let stem = if first <= last { &path[first..last] } else { "" };
    (parent, stem, extension)
}

/// Compute the BA2 hash for a full VFS-relative path (e.g.
/// `Interface\Pipboy_StatsPage.swf`). Case and slash direction are
/// normalized internally.
pub fn hash_path(path: &str) -> Ba2Hash {
    let normalized = path.replace('/', "\\").to_lowercase();
    let (parent, stem, extension) = split_path(&normalized);

    let mut ext_packed: u32 = 0;
    for (i, b) in extension.as_bytes().iter().take(4).enumerate() {
        ext_packed |= (*b as u32) << (i * 8);
    }

    Ba2Hash {
        file: crc32_no_invert(stem.as_bytes()),
        extension: ext_packed,
        directory: crc32_no_invert(parent.as_bytes()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Known-answer vectors reproduced from the `ba2` crate's own published
    // unit tests (docs.rs/ba2, source: `fo4::hashing` module) -- only the
    // input path / expected (file, extension, directory) triples are used
    // here, not any of that crate's code.
    #[test]
    fn known_answer_vectors() {
        let cases: &[(&str, u32, u32, u32)] = &[
            (
                r"ShadersFX\Shaders011.fxp",
                0x883415D8,
                0x00707866,
                0xDFAE3D0F,
            ),
            (
                r"Interface\Pipboy_StatsPage.swf",
                0x2F26E4D0,
                0x00667773,
                0xD2FDF873,
            ),
            (
                r"scripts\MinRadiantOwnedBuildResourceScript.pex",
                0xA2DAD4FD,
                0x00786570,
                0x40724840,
            ),
            (
                r"Meshes\debris\roundrock2_dirt.nif",
                0x1E47A158,
                0x0066696E,
                0xF55EC6BA,
            ),
        ];

        for (path, file, extension, directory) in cases {
            let h = hash_path(path);
            assert_eq!(h.file, *file, "file hash mismatch for {path}");
            assert_eq!(h.extension, *extension, "extension hash mismatch for {path}");
            assert_eq!(h.directory, *directory, "directory hash mismatch for {path}");
        }
    }

    #[test]
    fn case_and_slash_normalization_do_not_change_the_hash() {
        let a = hash_path("Textures/Armor/Cuirass_d.DDS");
        let b = hash_path(r"textures\armor\cuirass_d.dds");
        assert_eq!(a, b);
    }
}
