//! Composed Wave 0 gate: exercises `docs/SCHEDULE.md`'s Wave 0 gate
//! sentence as one connected flow through the crate that actually owns
//! it (Vestibule), rather than as separate clauses proven in isolation by
//! different crates.
//!
//! The gate sentence is:
//!
//! > round-trip a real 7z archive through the standalone package alone
//! > (no BSA/BA2 dependency); separately, list + extract a real BSA and
//! > a real BA2 through the extension crates; pack a stem back to
//! > `{stem}.bsa` / `{stem} - Textures.bsa` and `{stem} - Main.ba2` /
//! > `{stem} - Textures.ba2`; a manual conflict pick overrides last-wins
//! > on one path.
//!
//! Each crate already has its own dedicated tests for its own clause
//! in isolation:
//! - `sevenzip-re/tests/roundtrip.rs` (the 7z round trip, both
//!   directions against the real system binary).
//! - `modfather-bsa/tests/oracle_cross_validation.rs` and
//!   `modfather-ba2/tests/oracle_cross_validation.rs` (list/extract a
//!   real-format BSA/BA2 against an independent oracle implementation).
//! - `modfather-vestibule/src/packing.rs` unit tests (pack-back naming).
//! - `modfather-vestibule/src/layering.rs` unit tests (manual pick
//!   overrides last-wins on one path).
//!
//! What none of those prove on their own is that the pieces **compose**:
//! that a stem packed by [`modfather_vestibule::packing`] is a real,
//! spec-conforming archive its own crate's reader can list and extract
//! (the "list + extract ... through the extension crates" half of the
//! gate, applied to *our own* pack-back output, not just a synthetic
//! fixture), and that the resulting per-MOD contribution set is exactly
//! what [`modfather_vestibule::layering`] resolves with a manual pick
//! overriding last-wins on one path and nothing else. This test wires
//! that whole chain together the way Vestibule -- the crate with custody
//! of files per `docs/SCHEDULE.md` -- actually would.

use modfather_vestibule::layering::{resolve, LayerSource, VfsPath};
use modfather_vestibule::packing::{pack_ba2_stem, pack_bsa_stem, LooseFile};
use std::collections::HashMap;

fn loose(path: &str, data: &[u8]) -> LooseFile {
    LooseFile {
        path: path.to_string(),
        data: data.to_vec(),
    }
}

/// Pack a MOD's loose files to BSA, then list + extract every resulting
/// archive back out through `modfather-bsa`'s own reader, returning the
/// flat set of VFS-relative paths this MOD contributes (folder/name
/// recombined, matching how a real VFS layer would see them).
fn pack_and_relist_bsa(stem: &str, files: &[LooseFile]) -> (Vec<String>, HashMap<String, Vec<u8>>) {
    let archives = pack_bsa_stem(stem, files, 105, true).expect("BSA pack-back must succeed");

    let mut vfs_paths = Vec::new();
    let mut contents: HashMap<String, Vec<u8>> = HashMap::new();

    for bytes in archives.values() {
        let mut archive = modfather_bsa::BsaArchive::open(std::io::Cursor::new(bytes.clone()))
            .expect("our own pack-back output must be openable by our own reader");
        for (idx, entry) in archive.entries().into_iter().enumerate() {
            let full = if entry.folder.is_empty() {
                entry.name.clone()
            } else {
                format!("{}\\{}", entry.folder, entry.name)
            };
            let data = archive
                .read_file(idx)
                .expect("every listed entry must extract cleanly");
            vfs_paths.push(full.clone());
            contents.insert(full, data);
        }
    }

    (vfs_paths, contents)
}

/// Same as [`pack_and_relist_bsa`] but for BA2, whose entries already
/// carry the full backslash-joined path in `name` (no separate
/// folder/name split at the reader level).
fn pack_and_relist_ba2(stem: &str, files: &[LooseFile]) -> (Vec<String>, HashMap<String, Vec<u8>>) {
    let archives = pack_ba2_stem(stem, files, 1, true).expect("BA2 pack-back must succeed");

    let mut vfs_paths = Vec::new();
    let mut contents: HashMap<String, Vec<u8>> = HashMap::new();

    for bytes in archives.values() {
        let mut archive = modfather_ba2::Ba2Archive::open(std::io::Cursor::new(bytes.clone()))
            .expect("our own pack-back output must be openable by our own reader");
        let names: Vec<String> = archive.entries().iter().map(|e| e.name.clone()).collect();
        for (idx, name) in names.into_iter().enumerate() {
            let data = archive
                .read_file(idx)
                .expect("every listed entry must extract cleanly");
            vfs_paths.push(name.clone());
            contents.insert(name, data);
        }
    }

    (vfs_paths, contents)
}

/// The full composed gate: two "MODs" (one packed to BSA, one to BA2),
/// each contributing a shared path plus a MOD-unique path; layer them,
/// confirm last-wins picks the later MOD for the shared path, then
/// confirm an explicit pick flips *only* that one path back to the
/// earlier MOD while the MOD-unique paths are untouched.
#[test]
fn packed_bsa_and_ba2_archives_compose_through_layering_with_a_manual_pick() {
    // ModA: BSA-packed, contributes the shared path (older content) plus
    // a BSA-only unique path.
    let mod_a_files = vec![
        loose(
            "meshes\\shared.nif",
            b"ModA version of shared.nif, repeated for compressibility "
                .repeat(10)
                .as_slice(),
        ),
        loose(
            "meshes\\a_only.nif",
            b"ModA-only content, repeated for compressibility "
                .repeat(10)
                .as_slice(),
        ),
    ];
    let (mod_a_paths, mod_a_contents) = pack_and_relist_bsa("ModA", &mod_a_files);
    assert_eq!(mod_a_paths.len(), 2, "ModA's BSA pack-back must round-trip both files");

    // ModB: BA2-packed, contributes the *same* shared path (newer
    // content) plus a BA2-only unique path. Real BSA/BA2 use different
    // separators internally (BSA splits folder/name, BA2 keeps one
    // backslash-joined name) but both normalize to the same VfsPath key
    // once they reach the layering layer, exactly as
    // `docs/VESTIBULE.md`'s "last-wins layering" describes across
    // heterogeneous archive sources.
    let mod_b_files = vec![
        loose(
            "meshes\\shared.nif",
            b"ModB version of shared.nif, repeated for compressibility "
                .repeat(10)
                .as_slice(),
        ),
        loose(
            "meshes\\b_only.nif",
            b"ModB-only content, repeated for compressibility "
                .repeat(10)
                .as_slice(),
        ),
    ];
    let (mod_b_paths, mod_b_contents) = pack_and_relist_ba2("ModB", &mod_b_files);
    assert_eq!(mod_b_paths.len(), 2, "ModB's BA2 pack-back must round-trip both files");

    // Sanity: both mods really do contribute the *same* normalized VFS
    // path (case/slash differences aside), with genuinely different
    // bytes -- otherwise this wouldn't be exercising a real conflict.
    let shared_key = "meshes/shared.nif";
    assert!(mod_a_contents.contains_key("meshes\\shared.nif"));
    assert!(mod_b_contents.contains_key("meshes\\shared.nif"));
    assert_ne!(
        mod_a_contents["meshes\\shared.nif"],
        mod_b_contents["meshes\\shared.nif"],
        "the two MODs must genuinely disagree on shared.nif's bytes for this to be a real conflict"
    );

    let sources = vec![
        LayerSource {
            mod_name: "ModA".to_string(),
            paths: mod_a_paths.iter().map(|p| VfsPath::new(p)).collect(),
        },
        LayerSource {
            mod_name: "ModB".to_string(),
            paths: mod_b_paths.iter().map(|p| VfsPath::new(p)).collect(),
        },
    ];

    // Pass 1: no explicit picks -- last-wins means ModB (later in layer
    // order) wins the shared path.
    let no_picks: HashMap<VfsPath, String> = HashMap::new();
    let resolved = resolve(&sources, &no_picks);

    let shared = resolved
        .iter()
        .find(|r| r.path.as_str() == shared_key)
        .expect("shared.nif must appear in the resolved layering set");
    assert_eq!(shared.winner, "ModB");
    assert!(!shared.is_explicit_pick);
    assert_eq!(shared.contributors, vec!["ModA", "ModB"]);

    let a_only = resolved
        .iter()
        .find(|r| r.path.as_str() == "meshes/a_only.nif")
        .expect("ModA's unique file must still be present");
    assert_eq!(a_only.winner, "ModA");
    let b_only = resolved
        .iter()
        .find(|r| r.path.as_str() == "meshes/b_only.nif")
        .expect("ModB's unique file must still be present");
    assert_eq!(b_only.winner, "ModB");

    // Pass 2: an explicit manual pick for the shared path only, per the
    // gate's exact wording ("a manual conflict pick overrides last-wins
    // on one path"). ModA's real packed-and-reread bytes are the ones
    // that actually surface once resolved -- this is the composed
    // guarantee this test adds on top of `layering`'s own unit tests
    // (which only ever reason about plugin names, never real packed
    // archive bytes).
    let mut picks: HashMap<VfsPath, String> = HashMap::new();
    picks.insert(VfsPath::new(shared_key), "ModA".to_string());
    let resolved_with_pick = resolve(&sources, &picks);

    let shared_after_pick = resolved_with_pick
        .iter()
        .find(|r| r.path.as_str() == shared_key)
        .unwrap();
    assert_eq!(shared_after_pick.winner, "ModA");
    assert!(shared_after_pick.is_explicit_pick);
    assert_eq!(
        mod_a_contents["meshes\\shared.nif"],
        b"ModA version of shared.nif, repeated for compressibility ".repeat(10),
        "the winning bytes after the manual pick must be ModA's real packed-and-reread content"
    );

    // The pick must not leak onto any other path: both MOD-unique files
    // keep their only possible winner, exactly as before.
    let a_only_after_pick = resolved_with_pick
        .iter()
        .find(|r| r.path.as_str() == "meshes/a_only.nif")
        .unwrap();
    assert_eq!(a_only_after_pick.winner, "ModA");
    assert!(!a_only_after_pick.is_explicit_pick);
    let b_only_after_pick = resolved_with_pick
        .iter()
        .find(|r| r.path.as_str() == "meshes/b_only.nif")
        .unwrap();
    assert_eq!(b_only_after_pick.winner, "ModB");
    assert!(!b_only_after_pick.is_explicit_pick);
}
