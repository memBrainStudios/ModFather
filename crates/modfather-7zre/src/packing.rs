//! Pack a MOD's packable loose files back into archives, per Bethesda
//! doctrine (`docs/VESTIBULE.md`, "Pull" step 3):
//!
//! - BSA: `{stem}.bsa`, `{stem} - Textures.bsa` — **never** ` - Main` on BSA.
//! - BA2: `{stem} - Main.ba2`, `{stem} - Textures.ba2`.
//!
//! This module owns the *split decision* (which loose files are "texture"
//! files vs. everything else) and the resulting archive naming; the actual
//! container bytes are produced by [`modfather_bsa::write`] /
//! [`modfather_ba2::write`] (Main) and [`modfather_ba2::write_dx10`]
//! (BA2 Textures).
//!
//! BA2 Textures archives are packed as real DX10 (not GNRL): each `.dds`
//! loose file's header is parsed by [`crate::dds`] (a minimal, non-pixel
//! DDS-metadata parser -- see that module's doc comment for its scope)
//! into a `modfather_ba2::TextureToPack`, which `write_dx10` packs as one
//! full-mip-range chunk per texture (Wave-0 scope; see that function's
//! doc comment in `modfather-ba2`). BSA Textures archives have no such
//! sub-format distinction -- BSA is a flat container regardless of file
//! type -- so [`pack_bsa_stem`] needs no equivalent DDS-aware step.

use std::collections::BTreeMap;

/// One packable loose file: a VFS-relative path (folder + name, slash or
/// backslash) and its bytes.
#[derive(Debug, Clone)]
pub struct LooseFile {
    pub path: String,
    pub data: Vec<u8>,
}

/// Whether a loose file's extension marks it as a "texture" file for the
/// Main/Textures split. Only `.dds` is a texture extension in the
/// Bethesda archive doctrine this module implements.
fn is_texture_extension(path: &str) -> bool {
    path.rsplit('.')
        .next()
        .map(|ext| ext.eq_ignore_ascii_case("dds"))
        .unwrap_or(false)
}

/// Split `files` into (main, textures) by extension.
pub fn classify(files: &[LooseFile]) -> (Vec<&LooseFile>, Vec<&LooseFile>) {
    let mut main = Vec::new();
    let mut textures = Vec::new();
    for f in files {
        if is_texture_extension(&f.path) {
            textures.push(f);
        } else {
            main.push(f);
        }
    }
    (main, textures)
}

fn split_folder_name(path: &str) -> (String, String) {
    let normalized = path.replace('/', "\\");
    match normalized.rfind('\\') {
        Some(idx) => (normalized[..idx].to_string(), normalized[idx + 1..].to_string()),
        None => (String::new(), normalized),
    }
}

/// Pack `files` into BSA archives named per Bethesda doctrine for `stem`.
/// Returns a map from archive file name to archive bytes. The main archive
/// (`{stem}.bsa`) is always present (even if empty of textures); the
/// Textures archive (`{stem} - Textures.bsa`) is present only if `files`
/// contains at least one `.dds` entry.
pub fn pack_bsa_stem(
    stem: &str,
    files: &[LooseFile],
    version: u32,
    compress: bool,
) -> modfather_bsa::Result<BTreeMap<String, Vec<u8>>> {
    let (main, textures) = classify(files);
    let mut out = BTreeMap::new();

    let main_bytes = pack_bsa_group(&main, version, compress)?;
    out.insert(format!("{stem}.bsa"), main_bytes);

    if !textures.is_empty() {
        let tex_bytes = pack_bsa_group(&textures, version, compress)?;
        out.insert(format!("{stem} - Textures.bsa"), tex_bytes);
    }

    Ok(out)
}

fn pack_bsa_group(
    files: &[&LooseFile],
    version: u32,
    compress: bool,
) -> modfather_bsa::Result<Vec<u8>> {
    let to_pack: Vec<modfather_bsa::FileToPack> = files
        .iter()
        .map(|f| {
            let (folder, name) = split_folder_name(&f.path);
            modfather_bsa::FileToPack {
                folder,
                name,
                data: f.data.clone(),
            }
        })
        .collect();

    let options = modfather_bsa::WriteOptions { version, compress };
    let mut buf = Vec::new();
    modfather_bsa::write(std::io::Cursor::new(&mut buf), &to_pack, &options)?;
    Ok(buf)
}

/// Pack `files` into BA2 archives named per Bethesda doctrine for `stem`.
/// Returns a map from archive file name to archive bytes. The Main archive
/// (`{stem} - Main.ba2`) is always GNRL and always present; the Textures
/// archive (`{stem} - Textures.ba2`) is real DX10 (see this module's doc
/// comment) and is present only if `files` contains at least one `.dds`
/// entry.
///
/// # Errors
/// Returns [`modfather_ba2::Error::Malformed`] if any `.dds` texture file
/// fails to parse (bad/missing DDS magic, truncated header, or an
/// unrecognized legacy FourCC with no `DX10` extended header -- see
/// [`crate::dds::parse`]).
pub fn pack_ba2_stem(
    stem: &str,
    files: &[LooseFile],
    version: u32,
    compress: bool,
) -> modfather_ba2::Result<BTreeMap<String, Vec<u8>>> {
    let (main, textures) = classify(files);
    let mut out = BTreeMap::new();

    let main_bytes = pack_ba2_group(&main, version, compress)?;
    out.insert(format!("{stem} - Main.ba2"), main_bytes);

    if !textures.is_empty() {
        let tex_bytes = pack_ba2_textures_group(&textures, version, compress)?;
        out.insert(format!("{stem} - Textures.ba2"), tex_bytes);
    }

    Ok(out)
}

fn pack_ba2_group(
    files: &[&LooseFile],
    version: u32,
    compress: bool,
) -> modfather_ba2::Result<Vec<u8>> {
    let to_pack: Vec<modfather_ba2::FileToPack> = files
        .iter()
        .map(|f| modfather_ba2::FileToPack {
            name: f.path.replace('/', "\\"),
            data: f.data.clone(),
        })
        .collect();

    let options = modfather_ba2::WriteOptions {
        version,
        compress,
        force_lz4_v3: false,
    };
    let mut buf = Vec::new();
    modfather_ba2::write(std::io::Cursor::new(&mut buf), &to_pack, &options)?;
    Ok(buf)
}

/// Pack `.dds` files into a real DX10 BA2 archive, per this module's doc
/// comment. Each file's DDS header is parsed by [`crate::dds`] to fill in
/// `F4TexInfo`'s metadata fields.
fn pack_ba2_textures_group(
    files: &[&LooseFile],
    version: u32,
    compress: bool,
) -> modfather_ba2::Result<Vec<u8>> {
    let to_pack: Vec<modfather_ba2::TextureToPack> = files
        .iter()
        .map(|f| {
            crate::dds::to_texture_to_pack(f).map_err(|e| {
                modfather_ba2::Error::Malformed(format!(
                    "failed to parse DDS header for {:?}: {e}",
                    f.path
                ))
            })
        })
        .collect::<modfather_ba2::Result<Vec<_>>>()?;

    let options = modfather_ba2::WriteOptions {
        version,
        compress,
        force_lz4_v3: false,
    };
    let mut buf = Vec::new();
    modfather_ba2::write_dx10(std::io::Cursor::new(&mut buf), &to_pack, &options)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loose(path: &str, data: &[u8]) -> LooseFile {
        LooseFile {
            path: path.to_string(),
            data: data.to_vec(),
        }
    }

    /// Build a minimal legal legacy-FourCC (`DXT5`) `.dds` file wrapping
    /// `mip_data`, for tests that need a `.dds` loose file that will
    /// actually pass [`crate::dds::parse`] -- see that module's own unit
    /// tests for the full header-field layout this mirrors.
    fn fake_dds(mip_data: &[u8]) -> Vec<u8> {
        let mut buf = vec![0u8; 128];
        buf[0..4].copy_from_slice(b"DDS ");
        buf[12..16].copy_from_slice(&64u32.to_le_bytes()); // height
        buf[16..20].copy_from_slice(&64u32.to_le_bytes()); // width
        buf[28..32].copy_from_slice(&1u32.to_le_bytes()); // mip map count
        buf[84..88].copy_from_slice(b"DXT5");
        buf.extend_from_slice(mip_data);
        buf
    }

    #[test]
    fn classify_splits_dds_from_everything_else() {
        let files = vec![
            loose("meshes\\cube.nif", b"nif"),
            loose("textures\\cube_d.dds", b"dds"),
            loose("textures\\cube_n.DDS", b"dds-upper-ext"),
            loose("sound\\click.wav", b"wav"),
        ];
        let (main, textures) = classify(&files);
        assert_eq!(main.len(), 2);
        assert_eq!(textures.len(), 2);
        assert!(main.iter().all(|f| !f.path.to_lowercase().ends_with(".dds")));
        assert!(textures.iter().all(|f| f.path.to_lowercase().ends_with(".dds")));
    }

    #[test]
    fn bsa_naming_never_uses_main_suffix() {
        let files = vec![
            loose("meshes\\cube.nif", b"nif bytes repeated for compressibility ".repeat(10).as_slice()),
            loose("textures\\cube_d.dds", b"dds bytes repeated for compressibility ".repeat(10).as_slice()),
        ];
        let archives = pack_bsa_stem("MyMod", &files, 105, true).unwrap();

        assert!(archives.contains_key("MyMod.bsa"));
        assert!(archives.contains_key("MyMod - Textures.bsa"));
        assert!(!archives.keys().any(|k| k.contains(" - Main")));
    }

    #[test]
    fn bsa_textures_archive_absent_when_no_dds_files() {
        let files = vec![loose(
            "meshes\\cube.nif",
            b"nif bytes repeated for compressibility ".repeat(10).as_slice(),
        )];
        let archives = pack_bsa_stem("MyMod", &files, 105, true).unwrap();

        assert_eq!(archives.len(), 1);
        assert!(archives.contains_key("MyMod.bsa"));
    }

    #[test]
    fn ba2_naming_uses_main_and_textures_suffixes() {
        let files = vec![
            loose(
                "Scripts\\Source\\Foo.psc",
                b"Scriptname Foo extends Quest ".repeat(10).as_slice(),
            ),
            loose(
                "Textures\\Foo_d.dds",
                &fake_dds(b"dds mip bytes repeated for compressibility ".repeat(10).as_slice()),
            ),
        ];
        let archives = pack_ba2_stem("MyMod", &files, 1, true).unwrap();

        assert!(archives.contains_key("MyMod - Main.ba2"));
        assert!(archives.contains_key("MyMod - Textures.ba2"));
    }

    #[test]
    fn ba2_textures_archive_absent_when_no_dds_files() {
        let files = vec![loose(
            "Interface\\HUDMenu.swf",
            b"swf bytes repeated for compressibility ".repeat(10).as_slice(),
        )];
        let archives = pack_ba2_stem("MyMod", &files, 1, true).unwrap();

        assert_eq!(archives.len(), 1);
        assert!(archives.contains_key("MyMod - Main.ba2"));
    }

    #[test]
    fn packed_bsa_archives_round_trip_through_the_reader() {
        let files = vec![
            loose(
                "meshes\\cube.nif",
                b"nif bytes repeated for compressibility ".repeat(10).as_slice(),
            ),
            loose(
                "textures\\cube_d.dds",
                b"dds bytes repeated for compressibility ".repeat(10).as_slice(),
            ),
        ];
        let archives = pack_bsa_stem("MyMod", &files, 105, true).unwrap();

        let main_bytes = archives.get("MyMod.bsa").unwrap().clone();
        let mut main_archive =
            modfather_bsa::BsaArchive::open(std::io::Cursor::new(main_bytes)).unwrap();
        assert_eq!(main_archive.entries().len(), 1);
        assert_eq!(main_archive.read_file(0).unwrap(), files[0].data);

        let tex_bytes = archives.get("MyMod - Textures.bsa").unwrap().clone();
        let mut tex_archive =
            modfather_bsa::BsaArchive::open(std::io::Cursor::new(tex_bytes)).unwrap();
        assert_eq!(tex_archive.entries().len(), 1);
        assert_eq!(tex_archive.read_file(0).unwrap(), files[1].data);
    }

    #[test]
    fn packed_ba2_archives_round_trip_through_the_reader() {
        let tex_mip_data = b"dds mip bytes repeated for compressibility ".repeat(10);
        let files = vec![
            loose(
                "Interface\\HUDMenu.swf",
                b"swf bytes repeated for compressibility ".repeat(10).as_slice(),
            ),
            loose("Textures\\Foo_d.dds", &fake_dds(&tex_mip_data)),
        ];
        let archives = pack_ba2_stem("MyMod", &files, 1, true).unwrap();

        let main_bytes = archives.get("MyMod - Main.ba2").unwrap().clone();
        let mut main_archive =
            modfather_ba2::Ba2Archive::open(std::io::Cursor::new(main_bytes)).unwrap();
        assert_eq!(main_archive.entries().len(), 1);
        assert_eq!(main_archive.read_file(0).unwrap(), files[0].data);

        // The Textures BA2 is real DX10 now (see this module's doc
        // comment): its single entry is a `Texture`, not a `General`,
        // and its bytes are read back via `read_chunk` (chunk 0, the
        // one full-mip-range chunk `write_dx10` emits) -- and are the
        // *stripped* mip bytes, not the original `.dds` file bytes
        // (the DDS header parsed off by `dds::to_texture_to_pack` is
        // never packed into the archive; see `crate::dds`'s doc comment).
        let tex_bytes = archives.get("MyMod - Textures.ba2").unwrap().clone();
        let mut tex_archive =
            modfather_ba2::Ba2Archive::open(std::io::Cursor::new(tex_bytes)).unwrap();
        assert_eq!(tex_archive.entries().len(), 1);
        match &tex_archive.entries()[0].kind {
            modfather_ba2::EntryKind::Texture { height, width, .. } => {
                assert_eq!(*height, 64);
                assert_eq!(*width, 64);
            }
            modfather_ba2::EntryKind::General { .. } => panic!("expected a DX10 Texture entry"),
        }
        assert_eq!(tex_archive.read_chunk(0, 0).unwrap(), tex_mip_data);
    }
}
