//! The trees as data packs declare them (`data/<ns>/skills/<tree>.json`), merged and checked.
//!
//! A file either defines a tree (it has a `title`) or adds to one (`extends`). [`load`] merges
//! every file into [`Trees`], skipping each invalid entry with a [`Problem`] naming its file.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// How experience turns into levels: level `n` needs `base + step * n` more.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Curve {
    pub base: f64,
    pub step: f64,
}

impl Default for Curve {
    fn default() -> Self {
        Self { base: 50.0, step: 25.0 }
    }
}

/// How a perk's amount applies, as the game's `AttributeModifier.Operation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    AddValue,
    AddMultipliedBase,
    AddMultipliedTotal,
}

/// What a node gives. Tagged so new kinds add later without changing files.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Perk {
    Attribute { attribute: String, amount: f64, operation: Operation },
}

/// What a source matches: an id (`minecraft:stone`), a tag (`#minecraft:logs`) or anything (`*`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum Of {
    Id(String),
    Tag(String),
    Any,
}

impl From<String> for Of {
    fn from(s: String) -> Self {
        match s.strip_prefix('#') {
            Some(tag) => Of::Tag(tag.into()),
            None if s == "*" => Of::Any,
            None => Of::Id(s),
        }
    }
}

impl From<Of> for String {
    fn from(of: Of) -> Self {
        match of {
            Of::Id(id) => id,
            Of::Tag(tag) => format!("#{tag}"),
            Of::Any => "*".into(),
        }
    }
}

/// Which block event a block source hears.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockEvent {
    #[default]
    Broken,
    Placed,
}

/// An action that earns a tree experience.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Source {
    /// A statistic award: `stat` is its type (`minecraft:mined`), `of` its entry. Each award
    /// earns `xp` times its amount (`walk_one_cm` counts centimetres, so its `xp` is small).
    Stat { stat: String, of: Of, xp: f64 },
    /// A block a player broke (or placed, with `"on": "placed"`), only when its state has every
    /// listed property value.
    Block {
        block: Of,
        #[serde(default)]
        on: BlockEvent,
        #[serde(default)]
        state: BTreeMap<String, String>,
        xp: f64,
    },
}

/// One node of a tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// Unique in its tree (`haste_1`).
    pub id: String,
    /// A translation key for its name; the id shows when it has none.
    #[serde(default)]
    pub title: Option<String>,
    /// The item drawn in its frame.
    pub icon: String,
    /// Its cell on the tree's grid, `[column, row]`, from the reading start.
    pub at: [i32; 2],
    #[serde(default = "one")]
    pub cost: u32,
    /// Every node it needs unlocked first.
    #[serde(default)]
    pub needs: Vec<String>,
    pub perks: Vec<Perk>,
}

fn one() -> u32 {
    1
}

/// One tree, merged from its file and every `extends` file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tree {
    /// `skills:mining`, from the file's path.
    pub id: String,
    /// A translation key.
    pub title: String,
    pub icon: String,
    /// The block texture tiled behind the nodes (`minecraft:block/stone`).
    pub background: String,
    /// Tabs sort by `order`, then id.
    pub order: i32,
    pub curve: Curve,
    pub sources: Vec<Source>,
    pub nodes: Vec<Node>,
}

impl Tree {
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Points every node costs together: the tree's levels stop there.
    pub fn total_cost(&self) -> u32 {
        self.nodes.iter().map(|n| n.cost).sum()
    }
}

/// A tree file as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    title: Option<String>,
    extends: Option<String>,
    icon: Option<String>,
    background: Option<String>,
    #[serde(default)]
    order: i32,
    #[serde(default)]
    curve: Curve,
    #[serde(default)]
    sources: Vec<Source>,
    #[serde(default)]
    nodes: Vec<Node>,
}

/// Something skipped while loading, for one log line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub file: String,
    pub what: String,
}

/// Every loaded tree, in tab order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Trees(pub Vec<Tree>);

impl Trees {
    pub fn get(&self, id: &str) -> Option<&Tree> {
        self.0.iter().find(|t| t.id == id)
    }
}

/// Merges the files (`(file id, bytes)`, in resource order) into trees. `known_attribute` says
/// whether the game registers an attribute id.
pub fn load<'a>(
    files: impl IntoIterator<Item = (String, &'a [u8])>,
    known_attribute: impl Fn(&str) -> bool,
) -> (Trees, Vec<Problem>) {
    let mut problems = Vec::new();
    let mut trees: Vec<Tree> = Vec::new();
    let mut additions: Vec<(String, File)> = Vec::new();
    for (id, bytes) in files {
        let file: File = match serde_json::from_slice(bytes) {
            Ok(f) => f,
            Err(e) => {
                problems.push(Problem { file: id, what: e.to_string() });
                continue;
            }
        };
        match (&file.title, &file.extends) {
            (Some(title), None) => trees.push(Tree {
                title: title.clone(),
                icon: file.icon.clone().unwrap_or_else(|| "minecraft:book".into()),
                background: file.background.clone().unwrap_or_else(|| "minecraft:block/stone".into()),
                order: file.order,
                curve: file.curve,
                sources: file.sources,
                nodes: file.nodes,
                id,
            }),
            (None, Some(_)) => additions.push((id, file)),
            _ => problems.push(Problem { file: id, what: "needs exactly one of title or extends".into() }),
        }
    }
    for (id, file) in additions {
        let target = file.extends.unwrap_or_default();
        match trees.iter_mut().find(|t| t.id == target) {
            Some(tree) => {
                tree.sources.extend(file.sources);
                tree.nodes.extend(file.nodes);
            }
            None => problems.push(Problem { file: id, what: format!("extends unknown tree {target}") }),
        }
    }
    for tree in &mut trees {
        check(tree, &known_attribute, &mut problems);
    }
    trees.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
    (Trees(trees), problems)
}

/// Drops duplicate nodes, unknown attributes, unknown needs and needs that loop.
fn check(tree: &mut Tree, known_attribute: &impl Fn(&str) -> bool, problems: &mut Vec<Problem>) {
    let mut seen = BTreeSet::new();
    let mut flag = |what: String| problems.push(Problem { file: tree.id.clone(), what });
    tree.nodes.retain(|n| {
        let bad_perk = n.perks.iter().find_map(|p| match p {
            Perk::Attribute { attribute, .. } if !known_attribute(attribute) => Some(attribute.clone()),
            _ => None,
        });
        if let Some(attribute) = bad_perk {
            flag(format!("node {}: unknown attribute {attribute}", n.id));
            false
        } else if !seen.insert(n.id.clone()) {
            flag(format!("duplicate node {}", n.id));
            false
        } else {
            true
        }
    });
    // Unknown needs and loops: keep only nodes whose needs all resolve to kept nodes, repeatedly.
    loop {
        let ids: BTreeSet<String> = tree.nodes.iter().map(|n| n.id.clone()).collect();
        let (kept, lost): (Vec<Node>, Vec<Node>) = std::mem::take(&mut tree.nodes)
            .into_iter()
            .partition(|n| n.needs.iter().all(|need| ids.contains(need)));
        tree.nodes = kept;
        if lost.is_empty() {
            break;
        }
        for n in lost {
            flag(format!("node {} needs a missing node", n.id));
        }
    }
    let mut order = Vec::new();
    let mut placed = BTreeSet::new();
    while order.len() < tree.nodes.len() {
        let next = tree
            .nodes
            .iter()
            .position(|n| !placed.contains(&n.id) && n.needs.iter().all(|d| placed.contains(d)));
        match next {
            Some(i) => {
                placed.insert(tree.nodes[i].id.clone());
                order.push(i);
            }
            None => break,
        }
    }
    if order.len() < tree.nodes.len() {
        for n in tree.nodes.iter().filter(|n| !placed.contains(&n.id)) {
            flag(format!("node {} needs a loop", n.id));
        }
        tree.nodes.retain(|n| placed.contains(&n.id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINING: &str = r##"{
        "title": "skills.tree.mining", "icon": "minecraft:iron_pickaxe",
        "sources": [{ "stat": "minecraft:mined", "of": "#c:ores", "xp": 8 }],
        "nodes": [
            { "id": "a", "icon": "minecraft:stone", "at": [0, 0],
              "perks": [{ "attribute": "minecraft:block_break_speed", "amount": 0.1, "operation": "add_multiplied_base" }] },
            { "id": "b", "icon": "minecraft:stone", "at": [1, 0], "cost": 2, "needs": ["a"], "perks": [] }
        ]
    }"##;

    fn known(a: &str) -> bool {
        a.starts_with("minecraft:")
    }

    #[test]
    fn a_tree_and_an_addition_merge() {
        let extra = r#"{ "extends": "skills:mining", "nodes": [
            { "id": "c", "icon": "minecraft:stone", "at": [2, 0], "needs": ["b"], "perks": [] }] }"#;
        let (trees, problems) = load(
            [("skills:mining".into(), MINING.as_bytes()), ("acme:more".into(), extra.as_bytes())],
            known,
        );
        assert!(problems.is_empty(), "{problems:?}");
        let tree = trees.get("skills:mining").unwrap();
        assert_eq!(tree.nodes.len(), 3);
        assert_eq!(tree.total_cost(), 4);
        assert_eq!(tree.sources[0], Source::Stat { stat: "minecraft:mined".into(), of: Of::Tag("c:ores".into()), xp: 8.0 });
    }

    #[test]
    fn bad_nodes_are_skipped_and_named() {
        let bad = r#"{ "extends": "skills:mining", "nodes": [
            { "id": "a", "icon": "x:y", "at": [0, 1], "perks": [] },
            { "id": "x", "icon": "x:y", "at": [0, 2], "perks": [{ "attribute": "acme:nope", "amount": 1, "operation": "add_value" }] },
            { "id": "p", "icon": "x:y", "at": [0, 3], "needs": ["q"], "perks": [] },
            { "id": "q", "icon": "x:y", "at": [0, 4], "needs": ["p"], "perks": [] },
            { "id": "lost", "icon": "x:y", "at": [0, 5], "needs": ["ghost"], "perks": [] }] }"#;
        let (trees, problems) =
            load([("skills:mining".into(), MINING.as_bytes()), ("acme:bad".into(), bad.as_bytes())], known);
        let ids: Vec<_> = trees.get("skills:mining").unwrap().nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);
        assert!(problems.iter().any(|p| p.what.contains("duplicate node a")));
        assert!(problems.iter().any(|p| p.what.contains("acme:nope")));
    }

    #[test]
    fn unparsable_files_and_orphans_are_reported() {
        let (trees, problems) = load(
            [("a:broken".into(), b"{".as_slice()), ("a:orphan".into(), br#"{"extends":"a:none"}"#.as_slice())],
            known,
        );
        assert!(trees.0.is_empty());
        assert_eq!(problems.len(), 2);
    }
}

/// The trees this mod ships load cleanly: every file parses, every node is kept.
#[cfg(test)]
mod shipped {
    #[test]
    fn default_trees_load_without_problems() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/skills/skills");
        let files: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .map(|p| (format!("skills:{}", p.file_stem().unwrap().to_string_lossy()), std::fs::read(&p).unwrap()))
            .collect();
        let (trees, problems) = super::load(files.iter().map(|(i, b)| (i.clone(), b.as_slice())), |_| true);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(trees.0.len(), 5);
        let en: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("../../../assets/skills/lang/en_us.json")).unwrap()).unwrap();
        let ar: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("../../../assets/skills/lang/ar_sa.json")).unwrap()).unwrap();
        for t in &trees.0 {
            assert!((8..=40).contains(&t.total_cost()), "{} costs {}", t.id, t.total_cost());
            let keys = std::iter::once(t.title.clone()).chain(t.nodes.iter().filter_map(|n| n.title.clone()));
            for key in keys {
                assert!(en.get(&key).is_some() && ar.get(&key).is_some(), "untranslated {key}");
            }
        }
    }
}
