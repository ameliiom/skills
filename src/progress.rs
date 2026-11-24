//! A player's progress: the only stored state. Levels, points and perks derive from it.
use crate::tree::{Curve, Tree};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// One tree's progress.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// Every bit of experience earned on the tree, ever.
    pub xp: f64,
    pub unlocked: BTreeSet<String>,
}

/// A player's progress on every tree, by tree id. Trees no pack defines any more stay here
/// untouched, so removing a pack and adding it back loses nothing.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    pub trees: BTreeMap<String, Track>,
}

/// Where experience stands inside a level, for the bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level {
    pub level: u32,
    /// Experience into this level, and what the next one needs (0 at the top).
    pub into: f64,
    pub next: f64,
}

impl Curve {
    /// The experience level `n + 1` needs on top of level `n`.
    pub fn need(&self, n: u32) -> f64 {
        (self.base + self.step * f64::from(n)).max(1.0)
    }

    /// The level `xp` reaches, capped at `top`.
    pub fn level(&self, xp: f64, top: u32) -> Level {
        let mut left = xp.max(0.0);
        let mut level = 0;
        while level < top {
            let need = self.need(level);
            if left < need {
                return Level { level, into: left, next: need };
            }
            left -= need;
            level += 1;
        }
        Level { level, into: 0.0, next: 0.0 }
    }
}

/// Why a node can't be unlocked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NoSuchNode,
    Unlocked,
    Needs,
    Points,
}

/// How a node shows on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeState {
    Unlocked,
    /// Its needs are met and there are points enough: one click unlocks it.
    Open,
    /// Its needs are met, the points aren't yet.
    Affordable,
    Locked,
}

/// One tree seen through one player's progress.
pub struct View<'a> {
    pub tree: &'a Tree,
    pub track: Option<&'a Track>,
}

impl<'a> View<'a> {
    pub fn new(tree: &'a Tree, progress: &'a Progress) -> Self {
        Self { tree, track: progress.trees.get(&tree.id) }
    }

    fn unlocked(&self, id: &str) -> bool {
        self.track.is_some_and(|t| t.unlocked.contains(id))
    }

    pub fn level(&self) -> Level {
        let xp = self.track.map_or(0.0, |t| t.xp);
        self.tree.curve.level(xp, self.tree.total_cost())
    }

    /// Points spent on nodes this tree still has.
    pub fn spent(&self) -> u32 {
        self.tree.nodes.iter().filter(|n| self.unlocked(&n.id)).map(|n| n.cost).sum()
    }

    /// One point per level, minus what is spent.
    pub fn points(&self) -> u32 {
        self.level().level.saturating_sub(self.spent())
    }

    pub fn check(&self, node: &str) -> Result<(), Refusal> {
        let n = self.tree.node(node).ok_or(Refusal::NoSuchNode)?;
        if self.unlocked(node) {
            return Err(Refusal::Unlocked);
        }
        if !n.needs.iter().all(|d| self.unlocked(d)) {
            return Err(Refusal::Needs);
        }
        if self.points() < n.cost {
            return Err(Refusal::Points);
        }
        Ok(())
    }

    pub fn state(&self, node: &str) -> NodeState {
        match self.check(node) {
            Ok(()) => NodeState::Open,
            Err(Refusal::Unlocked) => NodeState::Unlocked,
            Err(Refusal::Points) => NodeState::Affordable,
            Err(_) => NodeState::Locked,
        }
    }
}

impl Progress {
    /// Unlocks `node` on `tree` when [`View::check`] allows it.
    pub fn unlock(&mut self, tree: &Tree, node: &str) -> Result<(), Refusal> {
        View::new(tree, self).check(node)?;
        self.trees.entry(tree.id.clone()).or_default().unlocked.insert(node.into());
        Ok(())
    }

    /// Adds experience to `tree`; returns the levels gained (for the toast).
    pub fn earn(&mut self, tree: &Tree, xp: f64) -> u32 {
        if !(xp.is_finite() && xp > 0.0) {
            return 0;
        }
        let before = View::new(tree, self).level().level;
        self.trees.entry(tree.id.clone()).or_default().xp += xp;
        View::new(tree, self).level().level - before
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Node;

    fn tree() -> Tree {
        let node = |id: &str, cost, needs: &[&str]| Node {
            id: id.into(),
            icon: "minecraft:stone".into(),
            at: [0, 0],
            cost,
            needs: needs.iter().map(|s| s.to_string()).collect(),
            perks: vec![],
        };
        Tree {
            id: "skills:mining".into(),
            title: String::new(),
            icon: String::new(),
            background: String::new(),
            order: 0,
            curve: Curve { base: 10.0, step: 5.0 },
            sources: vec![],
            nodes: vec![node("a", 1, &[]), node("b", 2, &["a"])],
        }
    }

    #[test]
    fn levels_follow_the_curve_and_stop_at_the_total_cost() {
        let c = Curve { base: 10.0, step: 5.0 };
        assert_eq!(c.level(0.0, 3), Level { level: 0, into: 0.0, next: 10.0 });
        assert_eq!(c.level(24.0, 3), Level { level: 1, into: 14.0, next: 15.0 });
        assert_eq!(c.level(1e9, 3).level, 3);
    }

    #[test]
    fn unlocking_spends_points_and_respects_needs() {
        let t = tree();
        let mut p = Progress::default();
        assert_eq!(p.unlock(&t, "a"), Err(Refusal::Points));
        assert_eq!(p.earn(&t, 10.0), 1);
        assert_eq!(View::new(&t, &p).state("b"), NodeState::Locked);
        assert_eq!(p.unlock(&t, "b"), Err(Refusal::Needs));
        assert_eq!(p.unlock(&t, "a"), Ok(()));
        assert_eq!(p.unlock(&t, "a"), Err(Refusal::Unlocked));
        assert_eq!(View::new(&t, &p).state("b"), NodeState::Affordable);
        p.earn(&t, 1e6);
        assert_eq!(View::new(&t, &p).points(), 2);
        assert_eq!(p.unlock(&t, "b"), Ok(()));
        assert_eq!(View::new(&t, &p).points(), 0);
    }

    #[test]
    fn bad_experience_is_ignored() {
        let t = tree();
        let mut p = Progress::default();
        for xp in [f64::NAN, -5.0, 0.0, f64::INFINITY] {
            assert_eq!(p.earn(&t, xp), 0);
        }
        assert!(p.trees.is_empty());
    }

    /// Law: whatever is earned and clicked, points never go negative and spent never exceeds levels.
    #[test]
    fn points_never_overspend() {
        let t = tree();
        let mut p = Progress::default();
        for step in 0..200u32 {
            p.earn(&t, f64::from(step % 7));
            let _ = p.unlock(&t, if step % 2 == 0 { "a" } else { "b" });
            let v = View::new(&t, &p);
            assert!(v.spent() <= v.level().level);
        }
    }
}
