//! Turns what a player did into tree experience, through the sources the trees declare.
use crate::tree::{BlockEvent, Of, Source, Trees};

/// What happened, as the hooks tell it.
pub enum Action<'a> {
    /// `player_stat_award`: a statistic type, its entry and the amount.
    Stat { stat: &'a str, of: &'a str, amount: f64 },
    /// `player_destroyed_block` or `entity_place`: the block, and a lookup of its state's
    /// property values.
    Block { on: BlockEvent, block: &'a str, state: &'a dyn Fn(&str) -> Option<String> },
}

/// Answers whether an id is in a tag (`has_tag("minecraft:stone", "minecraft:mineable/pickaxe")`).
/// The registry the tag belongs to follows from the action (blocks for `mined`, entity types for
/// `killed`, items for `crafted` and `used`).
pub trait Tags {
    fn has(&self, id: &str, tag: &str) -> bool;
}

fn matches(of: &Of, id: &str, tags: &dyn Tags) -> bool {
    match of {
        Of::Id(want) => want == id,
        Of::Tag(tag) => tags.has(id, tag),
        Of::Any => true,
    }
}

/// The experience `action` earns on each tree, by tree id. Every matching source adds up, so a
/// pack can give stone one point everywhere and ores eight more.
pub fn gains(trees: &Trees, action: &Action<'_>, tags: &dyn Tags) -> Vec<(String, f64)> {
    let mut out: Vec<(String, f64)> = Vec::new();
    for tree in &trees.0 {
        let xp: f64 = tree
            .sources
            .iter()
            .map(|source| match (source, action) {
                (Source::Stat { stat, of, xp }, Action::Stat { stat: s, of: o, amount })
                    if stat == s && matches(of, o, tags) =>
                {
                    xp * amount
                }
                (Source::Block { block, on, state, xp }, Action::Block { on: o, block: b, state: st })
                    if on == o && matches(block, b, tags) && state.iter().all(|(k, v)| st(k).as_ref() == Some(v)) =>
                {
                    *xp
                }
                _ => 0.0,
            })
            .sum();
        if xp > 0.0 && xp.is_finite() {
            out.push((tree.id.clone(), xp));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::load;
    use std::collections::BTreeMap;

    struct Ores;
    impl Tags for Ores {
        fn has(&self, id: &str, tag: &str) -> bool {
            (tag == "c:ores" && id.ends_with("_ore")) || (tag == "minecraft:crops" && id == "minecraft:wheat")
        }
    }

    fn trees() -> Trees {
        let file = r##"{ "title": "t", "sources": [
            { "stat": "minecraft:mined", "of": "minecraft:stone", "xp": 1 },
            { "stat": "minecraft:mined", "of": "#c:ores", "xp": 8 },
            { "stat": "minecraft:custom", "of": "minecraft:walk_one_cm", "xp": 0.001 },
            { "block": "#minecraft:crops", "state": { "age": "7" }, "xp": 2 } ] }"##;
        load([("skills:t".into(), file.as_bytes())], |_| true).0
    }

    #[test]
    fn stats_and_tags_add_up() {
        let t = trees();
        let mined = |of| gains(&t, &Action::Stat { stat: "minecraft:mined", of, amount: 1.0 }, &Ores);
        assert_eq!(mined("minecraft:stone"), [("skills:t".to_string(), 1.0)]);
        assert_eq!(mined("minecraft:iron_ore"), [("skills:t".to_string(), 8.0)]);
        assert!(mined("minecraft:dirt").is_empty());
        let walk = gains(&t, &Action::Stat { stat: "minecraft:custom", of: "minecraft:walk_one_cm", amount: 500.0 }, &Ores);
        assert!((walk[0].1 - 0.5).abs() < 1e-9);
    }

    #[test]
    fn crops_count_only_when_grown() {
        let t = trees();
        let at = |age: &str| {
            let state = BTreeMap::from([("age".to_string(), age.to_string())]);
            let prop = |k: &str| state.get(k).cloned();
            gains(&t, &Action::Block { on: BlockEvent::Broken, block: "minecraft:wheat", state: &prop }, &Ores)
        };
        assert!(at("3").is_empty());
        assert_eq!(at("7"), [("skills:t".to_string(), 2.0)]);
    }
}
