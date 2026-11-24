//! The attribute modifiers and tags a player should have: derived from progress, never stored.
//!
//! The server reconciles a player to [`wanted`] on join, respawn, unlock and reload: it adds or
//! replaces each wanted modifier and removes any `skills:` modifier no longer wanted.
use crate::progress::Progress;
use crate::tree::{Operation, Perk, Trees};
use std::collections::BTreeSet;

/// One modifier to have on one attribute.
#[derive(Clone, Debug, PartialEq)]
pub struct Wanted {
    pub attribute: String,
    /// `skills:<tree namespace>/<tree path>/<node>/<n>`: unique, and found again on reload.
    pub id: String,
    pub amount: f64,
    pub operation: Operation,
}

/// Every modifier `progress` earns on the loaded trees.
pub fn wanted(trees: &Trees, progress: &Progress) -> Vec<Wanted> {
    let mut out = Vec::new();
    for tree in &trees.0 {
        let Some(track) = progress.trees.get(&tree.id) else { continue };
        for node in tree.nodes.iter().filter(|n| track.unlocked.contains(&n.id)) {
            for (n, perk) in node.perks.iter().enumerate() {
                let Perk::Attribute { attribute, amount, operation } = perk else { continue };
                out.push(Wanted {
                    attribute: attribute.clone(),
                    id: format!("{}:{}/{}/{n}", crate::MOD_ID, tree.id.replace(':', "/"), node.id),
                    amount: *amount,
                    operation: *operation,
                });
            }
        }
    }
    out
}

/// The player tags `progress` earns, and every tag any loaded node can give (the ones Skills
/// may add or remove; a tag nothing gives is never touched).
pub fn tags(trees: &Trees, progress: &Progress) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut wanted = BTreeSet::new();
    let mut known = BTreeSet::new();
    for tree in &trees.0 {
        let unlocked = progress.trees.get(&tree.id).map(|t| &t.unlocked);
        for node in &tree.nodes {
            for perk in &node.perks {
                let Perk::Tag { tag } = perk else { continue };
                known.insert(tag.clone());
                if unlocked.is_some_and(|u| u.contains(&node.id)) {
                    wanted.insert(tag.clone());
                }
            }
        }
    }
    (wanted, known)
}

/// Whether a modifier id is one this mod owns (and so may remove).
pub fn ours(id: &str) -> bool {
    id.starts_with(&format!("{}:", crate::MOD_ID))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::load;

    #[test]
    fn only_unlocked_nodes_give_modifiers_with_stable_ids() {
        let file = r#"{ "title": "t", "nodes": [
            { "id": "a", "icon": "x:y", "at": [0, 0], "perks": [
                { "attribute": "minecraft:max_health", "amount": 2, "operation": "add_value" }] },
            { "id": "b", "icon": "x:y", "at": [1, 0], "perks": [
                { "attribute": "minecraft:luck", "amount": 1, "operation": "add_value" }] } ] }"#;
        let (trees, _) = load([("skills:farming".into(), file.as_bytes())], |_| true);
        let mut p = Progress::default();
        p.trees.entry("skills:farming".into()).or_default().unlocked.insert("a".into());
        let w = wanted(&trees, &p);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].id, "skills:skills/farming/a/0");
        assert!(ours(&w[0].id) && !ours("minecraft:base"));
    }

    #[test]
    fn tag_perks_follow_unlocked_nodes_and_known_tags_cover_every_node() {
        let file = r#"{ "title": "t", "nodes": [
            { "id": "a", "icon": "x:y", "at": [0, 0], "perks": [{ "tag": "storage.find" }] },
            { "id": "b", "icon": "x:y", "at": [1, 0], "perks": [{ "tag": "storage.network.64" },
                { "attribute": "minecraft:luck", "amount": 1, "operation": "add_value" }] } ] }"#;
        let (trees, problems) = load([("storage:organization".into(), file.as_bytes())], |_| true);
        assert!(problems.is_empty(), "{problems:?}");
        let mut p = Progress::default();
        p.trees.entry("storage:organization".into()).or_default().unlocked.insert("a".into());
        let (given, known) = tags(&trees, &p);
        assert_eq!(given.into_iter().collect::<Vec<_>>(), vec!["storage.find".to_string()]);
        assert_eq!(known.len(), 2);
        // A tag perk gives no attribute modifier.
        assert!(wanted(&trees, &p).is_empty());
    }
}
