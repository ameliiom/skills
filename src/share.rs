//! Experience shared with friends nearby: the earner keeps all of theirs, and every other player
//! in the same dimension within the radius earns a fraction of it on the same tree.

/// A player as sharing sees them, by any id (their UUID on the server).
pub struct Near<'a, K> {
    pub id: K,
    pub dimension: &'a str,
    pub at: [f64; 3],
}

/// The share rules, from the server settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rules {
    pub radius: f64,
    pub fraction: f64,
}

impl Default for Rules {
    fn default() -> Self {
        Self { radius: 32.0, fraction: 0.5 }
    }
}

/// Who earns what from `earner` gaining `xp`: the earner first with all of it, then each friend
/// in range with their share.
pub fn split<K: Copy + PartialEq>(earner: &Near<'_, K>, xp: f64, others: &[Near<'_, K>], rules: Rules) -> Vec<(K, f64)> {
    let mut out = vec![(earner.id, xp)];
    let share = xp * rules.fraction.clamp(0.0, 1.0);
    if share <= 0.0 || rules.radius <= 0.0 {
        return out;
    }
    for o in others {
        if o.id != earner.id && o.dimension == earner.dimension && crate::kit::near::within(o.at, earner.at, rules.radius) {
            out.push((o.id, share));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friends_in_range_and_dimension_share() {
        let me = Near { id: "a", dimension: "minecraft:overworld", at: [0.0, 64.0, 0.0] };
        let others = [
            Near { id: "b", dimension: "minecraft:overworld", at: [20.0, 64.0, 20.0] },
            Near { id: "c", dimension: "minecraft:overworld", at: [40.0, 64.0, 0.0] },
            Near { id: "d", dimension: "minecraft:the_nether", at: [0.0, 64.0, 0.0] },
            Near { id: "a", dimension: "minecraft:overworld", at: [0.0, 64.0, 0.0] },
        ];
        assert_eq!(split(&me, 10.0, &others, Rules::default()), [("a", 10.0), ("b", 5.0)]);
        assert_eq!(split(&me, 10.0, &others, Rules { radius: 0.0, fraction: 0.5 }), [("a", 10.0)]);
    }
}
