//! LOOT — generalized sorter stub (`docs/VESTIBULE.md`, `docs/SCHEDULE.md`
//! Wave 0 floor).
//!
//! Sorts a set of plugins (ESM/ESL/ESP — "ESM" means all three throughout
//! ModFather's docs) into a load order using, in decreasing priority:
//!
//! 1. **User-created rules** — explicit `before`/`after` constraints. These
//!    are the highest priority and may reorder plugins relative to the
//!    masterlist or categories.
//! 2. **The traditional masterlist** — a fixed priority ordering of known
//!    plugin names (`docs/VESTIBULE.md`: "traditional masterlist").
//! 3. **Nexus categories** — the user may use or modify them
//!    (`docs/VESTIBULE.md`); plugins with no masterlist entry fall back to
//!    a category priority, then to their original input order.
//! 4. **Stable input order** as the final fallback, so sorting never
//!    reshuffles plugins the rules above have no opinion about.
//!
//! This is deliberately a **thin rule engine** for Wave 0, per
//! `docs/SCHEDULE.md` ("even if the rule engine is thin at first"): the
//! masterlist is a flat priority list and user rules are simple pairwise
//! ordering constraints, not the full LOOT metadata language (conditions,
//! plugin groups, "requires"/"incompatible" messages, etc.). Growing
//! toward that full language is tracked as follow-up work once real
//! masterlist fixtures are available.
//!
//! Like [`crate::layering`], this module is deliberately independent of
//! archive bytes: it only reasons about plugin names, categories, and
//! ordering constraints, matching Vestibule's `MOD`/`MGE` state-container
//! split.

use std::collections::{HashMap, HashSet};

/// One plugin to be sorted, identified by file name. Comparisons are
/// case-insensitive, matching Bethesda's own case-insensitive file lookup.
#[derive(Debug, Clone)]
pub struct Plugin {
    pub name: String,
    /// Nexus category name, if the user has kept or assigned one for this
    /// plugin. `None` means "uncategorized".
    pub category: Option<String>,
}

/// A user-created ordering rule: `before` must load strictly before
/// `after`. Comparison is case-insensitive by plugin name.
#[derive(Debug, Clone)]
pub struct UserRule {
    pub before: String,
    pub after: String,
}

/// The traditional masterlist: known plugin names in a fixed priority
/// order (earlier entries load earlier). A plugin present in the
/// masterlist always sorts before any plugin that is not, matching real
/// LOOT's masterlist-then-everything-else precedence; plugins absent from
/// the masterlist fall through to category priority.
#[derive(Debug, Clone, Default)]
pub struct Masterlist {
    order: Vec<String>,
}

impl Masterlist {
    pub fn new(order: impl IntoIterator<Item = String>) -> Self {
        Masterlist {
            order: order.into_iter().collect(),
        }
    }

    fn rank(&self, name: &str) -> Option<usize> {
        self.order.iter().position(|n| n.eq_ignore_ascii_case(name))
    }
}

/// Nexus category priorities: a lower number sorts earlier among plugins
/// that have no masterlist entry. Categories not present here (including
/// `None`, "uncategorized") default to priority `0`. The user may use or
/// modify this map at will (`docs/VESTIBULE.md`).
#[derive(Debug, Clone, Default)]
pub struct CategoryPriorities {
    priorities: HashMap<String, i32>,
}

impl CategoryPriorities {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, category: impl Into<String>, priority: i32) {
        self.priorities.insert(category.into(), priority);
    }

    fn priority_for(&self, category: Option<&str>) -> i32 {
        match category {
            Some(c) => *self.priorities.get(c).unwrap_or(&0),
            None => 0,
        }
    }
}

/// Errors returned when the requested ordering cannot be satisfied.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SortError {
    /// Two or more user rules form a cycle (e.g. A before B, B before A);
    /// the named plugin is one member of that cycle.
    #[error("cyclic user rule involving plugin: {0}")]
    Cycle(String),

    /// A user rule names a plugin that is not in the input set.
    #[error("user rule references unknown plugin: {0}")]
    UnknownPlugin(String),
}

/// Sort `plugins` into a load order using `masterlist`, `categories`, and
/// `user_rules`, per this module's doc comment. The result is a `Vec` of
/// plugin names in final load order (earliest first).
pub fn sort(
    plugins: &[Plugin],
    masterlist: &Masterlist,
    categories: &CategoryPriorities,
    user_rules: &[UserRule],
) -> Result<Vec<String>, SortError> {
    let mut index_by_name: HashMap<String, usize> = HashMap::new();
    for (i, p) in plugins.iter().enumerate() {
        index_by_name.insert(p.name.to_lowercase(), i);
    }

    for rule in user_rules {
        if !index_by_name.contains_key(&rule.before.to_lowercase()) {
            return Err(SortError::UnknownPlugin(rule.before.clone()));
        }
        if !index_by_name.contains_key(&rule.after.to_lowercase()) {
            return Err(SortError::UnknownPlugin(rule.after.clone()));
        }
    }

    // Base order: masterlist rank first (plugins in the masterlist always
    // precede plugins that are not), then category priority, then the
    // original input position as a stable tiebreaker.
    let mut order: Vec<usize> = (0..plugins.len()).collect();
    order.sort_by(|&a, &b| {
        let pa = &plugins[a];
        let pb = &plugins[b];
        match (masterlist.rank(&pa.name), masterlist.rank(&pb.name)) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => {
                let ca = categories.priority_for(pa.category.as_deref());
                let cb = categories.priority_for(pb.category.as_deref());
                ca.cmp(&cb).then(a.cmp(&b))
            }
        }
    });

    // User rules are applied as a topological adjustment on top of the
    // base order: build a `before -> after` DAG, then repeatedly emit the
    // earliest-in-base-order plugin with no remaining unsatisfied
    // predecessor. This is a stable topo sort, so rules only move the
    // plugins they actually constrain — everything else keeps its
    // masterlist/category/input-order position.
    let mut adjacency: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut indegree: HashMap<usize, usize> = order.iter().map(|&i| (i, 0)).collect();
    for rule in user_rules {
        let before = index_by_name[&rule.before.to_lowercase()];
        let after = index_by_name[&rule.after.to_lowercase()];
        adjacency.entry(before).or_default().push(after);
        *indegree.entry(after).or_insert(0) += 1;
    }

    let position: HashMap<usize, usize> = order.iter().enumerate().map(|(pos, &i)| (i, pos)).collect();
    let mut remaining: HashSet<usize> = order.iter().copied().collect();
    let mut result = Vec::with_capacity(plugins.len());

    while !remaining.is_empty() {
        let next = remaining
            .iter()
            .filter(|i| indegree.get(i).copied().unwrap_or(0) == 0)
            .min_by_key(|i| position[i])
            .copied();

        match next {
            Some(i) => {
                remaining.remove(&i);
                result.push(i);
                if let Some(neighbors) = adjacency.get(&i) {
                    for &n in neighbors {
                        if let Some(d) = indegree.get_mut(&n) {
                            *d -= 1;
                        }
                    }
                }
            }
            None => {
                // No remaining node has indegree 0: every remaining
                // plugin is part of (or blocked by) a cycle. Report one
                // of them by the lowest base-order position for a
                // deterministic error.
                let culprit = *remaining.iter().min_by_key(|i| position[i]).unwrap();
                return Err(SortError::Cycle(plugins[culprit].name.clone()));
            }
        }
    }

    Ok(result.into_iter().map(|i| plugins[i].name.clone()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin(name: &str, category: Option<&str>) -> Plugin {
        Plugin {
            name: name.to_string(),
            category: category.map(|c| c.to_string()),
        }
    }

    #[test]
    fn masterlist_plugins_sort_before_unlisted_plugins() {
        let plugins = vec![plugin("Unlisted.esp", None), plugin("Skyrim.esm", None)];
        let masterlist = Masterlist::new(["Skyrim.esm".to_string()]);
        let categories = CategoryPriorities::new();

        let sorted = sort(&plugins, &masterlist, &categories, &[]).unwrap();
        assert_eq!(sorted, vec!["Skyrim.esm", "Unlisted.esp"]);
    }

    #[test]
    fn masterlist_order_is_respected_among_listed_plugins() {
        let plugins = vec![
            plugin("Update.esm", None),
            plugin("Skyrim.esm", None),
            plugin("Dawnguard.esm", None),
        ];
        let masterlist = Masterlist::new([
            "Skyrim.esm".to_string(),
            "Update.esm".to_string(),
            "Dawnguard.esm".to_string(),
        ]);
        let categories = CategoryPriorities::new();

        let sorted = sort(&plugins, &masterlist, &categories, &[]).unwrap();
        assert_eq!(sorted, vec!["Skyrim.esm", "Update.esm", "Dawnguard.esm"]);
    }

    #[test]
    fn unlisted_plugins_fall_back_to_category_priority() {
        let plugins = vec![
            plugin("PatchC.esp", Some("Patches")),
            plugin("GameplayA.esp", Some("Gameplay")),
            plugin("PatchB.esp", Some("Patches")),
        ];
        let masterlist = Masterlist::default();
        let mut categories = CategoryPriorities::new();
        categories.set("Gameplay", 0);
        categories.set("Patches", 10);

        let sorted = sort(&plugins, &masterlist, &categories, &[]).unwrap();
        // Gameplay category sorts first; within a category, original
        // input order is the stable tiebreaker (PatchC before PatchB).
        assert_eq!(sorted, vec!["GameplayA.esp", "PatchC.esp", "PatchB.esp"]);
    }

    #[test]
    fn uncategorized_plugins_use_default_priority_zero() {
        let plugins = vec![
            plugin("Low.esp", Some("Low")),
            plugin("NoCategory.esp", None),
        ];
        let masterlist = Masterlist::default();
        let mut categories = CategoryPriorities::new();
        categories.set("Low", 5);

        let sorted = sort(&plugins, &masterlist, &categories, &[]).unwrap();
        // Uncategorized defaults to priority 0, sorting before "Low" (5).
        assert_eq!(sorted, vec!["NoCategory.esp", "Low.esp"]);
    }

    #[test]
    fn user_rule_overrides_masterlist_order_for_the_pair_only() {
        let plugins = vec![
            plugin("A.esp", None),
            plugin("B.esp", None),
            plugin("C.esp", None),
        ];
        let masterlist = Masterlist::new(["A.esp".to_string(), "B.esp".to_string(), "C.esp".to_string()]);
        let categories = CategoryPriorities::new();
        let rules = vec![UserRule {
            before: "C.esp".to_string(),
            after: "A.esp".to_string(),
        }];

        let sorted = sort(&plugins, &masterlist, &categories, &rules).unwrap();
        // C must now come before A; B has no constraint touching it and
        // keeps its masterlist-relative position between the two others
        // wherever the topo sort naturally places it, but the key
        // invariant is: C precedes A.
        let pos_a = sorted.iter().position(|n| n == "A.esp").unwrap();
        let pos_c = sorted.iter().position(|n| n == "C.esp").unwrap();
        assert!(pos_c < pos_a, "C.esp must load before A.esp per the user rule");
    }

    #[test]
    fn user_rule_does_not_affect_unrelated_plugins() {
        let plugins = vec![
            plugin("A.esp", None),
            plugin("B.esp", None),
            plugin("C.esp", None),
            plugin("D.esp", None),
        ];
        let masterlist = Masterlist::default();
        let categories = CategoryPriorities::new();
        // Only a rule about C/D; A and B have no masterlist/category
        // opinion either, so their relative order (input order) should
        // be fully preserved.
        let rules = vec![UserRule {
            before: "D.esp".to_string(),
            after: "C.esp".to_string(),
        }];

        let sorted = sort(&plugins, &masterlist, &categories, &rules).unwrap();
        let pos_a = sorted.iter().position(|n| n == "A.esp").unwrap();
        let pos_b = sorted.iter().position(|n| n == "B.esp").unwrap();
        assert!(pos_a < pos_b, "A.esp and B.esp must keep their relative input order");
    }

    #[test]
    fn cyclic_user_rules_are_rejected() {
        let plugins = vec![plugin("A.esp", None), plugin("B.esp", None)];
        let masterlist = Masterlist::default();
        let categories = CategoryPriorities::new();
        let rules = vec![
            UserRule {
                before: "A.esp".to_string(),
                after: "B.esp".to_string(),
            },
            UserRule {
                before: "B.esp".to_string(),
                after: "A.esp".to_string(),
            },
        ];

        let err = sort(&plugins, &masterlist, &categories, &rules).unwrap_err();
        assert!(matches!(err, SortError::Cycle(_)));
    }

    #[test]
    fn user_rule_naming_an_unknown_plugin_is_an_error() {
        let plugins = vec![plugin("A.esp", None)];
        let masterlist = Masterlist::default();
        let categories = CategoryPriorities::new();
        let rules = vec![UserRule {
            before: "A.esp".to_string(),
            after: "GhostPlugin.esp".to_string(),
        }];

        let err = sort(&plugins, &masterlist, &categories, &rules).unwrap_err();
        assert_eq!(err, SortError::UnknownPlugin("GhostPlugin.esp".to_string()));
    }

    #[test]
    fn plugin_name_comparisons_are_case_insensitive() {
        let plugins = vec![plugin("Skyrim.ESM", None), plugin("Other.esp", None)];
        let masterlist = Masterlist::new(["skyrim.esm".to_string()]);
        let categories = CategoryPriorities::new();
        let rules = vec![UserRule {
            before: "OTHER.ESP".to_string(),
            after: "skyrim.esm".to_string(),
        }];

        let sorted = sort(&plugins, &masterlist, &categories, &rules).unwrap();
        assert_eq!(sorted, vec!["Other.esp", "Skyrim.ESM"]);
    }

    #[test]
    fn empty_input_sorts_to_empty_output() {
        let plugins: Vec<Plugin> = vec![];
        let masterlist = Masterlist::default();
        let categories = CategoryPriorities::new();

        let sorted = sort(&plugins, &masterlist, &categories, &[]).unwrap();
        assert!(sorted.is_empty());
    }
}
