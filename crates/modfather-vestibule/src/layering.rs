//! Last-wins layering with explicit 1:1 picks, and compare tooltips.
//!
//! This is "Bash" from `docs/VESTIBULE.md`: among Unlocked MODs, the last
//! one (by load order) to contribute a given VFS path wins by default; a
//! player-selected explicit pick for that exact path overrides the
//! automatic last-wins outcome. This module is deliberately independent of
//! any archive format — it only reasons about MOD order and the set of
//! relative VFS paths each MOD contributes, matching Vestibule's `MOD`/
//! `MGE` state-container split (archive bytes never enter this layer).

use std::collections::HashMap;

/// A normalized, slash-separated relative VFS path (e.g. `meshes/x.nif`).
/// Case is preserved for display but comparisons are case-insensitive,
/// matching Bethesda's own case-insensitive file lookup.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VfsPath(String);

impl VfsPath {
    pub fn new(raw: &str) -> Self {
        VfsPath(raw.replace('\\', "/"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn key(&self) -> String {
        self.0.to_lowercase()
    }
}

/// One Unlocked MOD's contribution to the layering pass: its name (for
/// display/picks) and the set of VFS paths it provides.
#[derive(Debug, Clone)]
pub struct LayerSource {
    pub mod_name: String,
    pub paths: Vec<VfsPath>,
}

/// Who provides a given path, and everyone else contending for it — the
/// data a "compare" tooltip needs to show without re-walking every MOD.
#[derive(Debug, Clone)]
pub struct Resolution {
    pub path: VfsPath,
    /// The MOD whose content is actually used for this path.
    pub winner: String,
    /// True if `winner` was forced by an explicit player pick rather than
    /// falling out of last-wins order.
    pub is_explicit_pick: bool,
    /// Every MOD (in layering order) that also contributes this path,
    /// including the winner — this is exactly what a compare tooltip shows.
    pub contributors: Vec<String>,
}

/// Resolve every path across `sources` (in layering order — later entries
/// win ties by default) into a single [`Resolution`] per path, honoring
/// any `explicit_picks` (`path -> mod_name`) as an override.
///
/// `explicit_picks` entries for a path with no contributor are ignored
/// (nothing to pick from); `sources` order is the MGE's Unlocked-MOD order,
/// not alphabetical.
pub fn resolve(sources: &[LayerSource], explicit_picks: &HashMap<VfsPath, String>) -> Vec<Resolution> {
    // path key -> (display path, contributors in layering order)
    let mut by_path: HashMap<String, (VfsPath, Vec<String>)> = HashMap::new();

    for source in sources {
        for path in &source.paths {
            let entry = by_path
                .entry(path.key())
                .or_insert_with(|| (path.clone(), Vec::new()));
            entry.1.push(source.mod_name.clone());
        }
    }

    let mut picks_by_key: HashMap<String, String> = HashMap::new();
    for (path, mod_name) in explicit_picks {
        picks_by_key.insert(path.key(), mod_name.clone());
    }

    let mut out: Vec<Resolution> = by_path
        .into_values()
        .map(|(path, contributors)| {
            let pick = picks_by_key.get(&path.key());
            let (winner, is_explicit_pick) = match pick {
                Some(picked) if contributors.iter().any(|c| c == picked) => {
                    (picked.clone(), true)
                }
                _ => (
                    contributors
                        .last()
                        .cloned()
                        .expect("a path always has at least one contributor"),
                    false,
                ),
            };
            Resolution {
                path,
                winner,
                is_explicit_pick,
                contributors,
            }
        })
        .collect();

    out.sort_by(|a, b| a.path.as_str().cmp(b.path.as_str()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(name: &str, paths: &[&str]) -> LayerSource {
        LayerSource {
            mod_name: name.to_string(),
            paths: paths.iter().map(|p| VfsPath::new(p)).collect(),
        }
    }

    #[test]
    fn last_wins_by_default() {
        let sources = vec![
            src("ModA", &["meshes/x.nif", "meshes/only_a.nif"]),
            src("ModB", &["meshes/x.nif"]),
            src("ModC", &["meshes/x.nif"]),
        ];
        let picks = HashMap::new();
        let resolved = resolve(&sources, &picks);

        let x = resolved
            .iter()
            .find(|r| r.path.as_str() == "meshes/x.nif")
            .unwrap();
        assert_eq!(x.winner, "ModC"); // last contributor in layering order
        assert!(!x.is_explicit_pick);
        assert_eq!(x.contributors, vec!["ModA", "ModB", "ModC"]);

        let only_a = resolved
            .iter()
            .find(|r| r.path.as_str() == "meshes/only_a.nif")
            .unwrap();
        assert_eq!(only_a.winner, "ModA");
        assert_eq!(only_a.contributors, vec!["ModA"]);
    }

    #[test]
    fn explicit_pick_overrides_last_wins_on_that_path_only() {
        let sources = vec![
            src("ModA", &["meshes/x.nif", "meshes/y.nif"]),
            src("ModB", &["meshes/x.nif", "meshes/y.nif"]),
            src("ModC", &["meshes/x.nif", "meshes/y.nif"]),
        ];
        let mut picks = HashMap::new();
        picks.insert(VfsPath::new("meshes/x.nif"), "ModA".to_string());
        let resolved = resolve(&sources, &picks);

        let x = resolved
            .iter()
            .find(|r| r.path.as_str() == "meshes/x.nif")
            .unwrap();
        assert_eq!(x.winner, "ModA");
        assert!(x.is_explicit_pick);

        // The pick must not leak onto the other path: y.nif still resolves
        // by last-wins (ModC), exactly as the Wave 0 gate requires ("a
        // manual conflict pick overrides last-wins on one path").
        let y = resolved
            .iter()
            .find(|r| r.path.as_str() == "meshes/y.nif")
            .unwrap();
        assert_eq!(y.winner, "ModC");
        assert!(!y.is_explicit_pick);
    }

    #[test]
    fn pick_for_a_mod_that_does_not_contribute_is_ignored() {
        let sources = vec![src("ModA", &["meshes/x.nif"]), src("ModB", &["meshes/x.nif"])];
        let mut picks = HashMap::new();
        picks.insert(VfsPath::new("meshes/x.nif"), "ModZ".to_string()); // never contributed
        let resolved = resolve(&sources, &picks);

        let x = &resolved[0];
        assert_eq!(x.winner, "ModB"); // falls back to last-wins
        assert!(!x.is_explicit_pick);
    }

    #[test]
    fn path_comparison_is_case_insensitive() {
        let sources = vec![
            src("ModA", &["Meshes/X.NIF"]),
            src("ModB", &["meshes/x.nif"]),
        ];
        let picks = HashMap::new();
        let resolved = resolve(&sources, &picks);

        assert_eq!(resolved.len(), 1, "the two paths must collapse to one entry");
        assert_eq!(resolved[0].winner, "ModB");
        assert_eq!(resolved[0].contributors, vec!["ModA", "ModB"]);
    }

    #[test]
    fn backslash_and_forward_slash_paths_are_equivalent() {
        let sources = vec![src("ModA", &["meshes\\x.nif"]), src("ModB", &["meshes/x.nif"])];
        let picks = HashMap::new();
        let resolved = resolve(&sources, &picks);

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].path.as_str(), "meshes/x.nif");
    }
}
