//! Experience shared with friends nearby: the earner keeps all of theirs, and every other player
//! in the same dimension within the radius earns a fraction of it on the same tree.

/// A player as sharing sees them.
pub struct Near<'a> {
    pub name: &'a str,
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
pub fn split<'a>(earner: &Near<'a>, xp: f64, others: &[Near<'a>], rules: Rules) -> Vec<(&'a str, f64)> {
    let mut out = vec![(earner.name, xp)];
    let share = xp * rules.fraction.clamp(0.0, 1.0);
    if share <= 0.0 || rules.radius <= 0.0 {
        return out;
    }
    let r2 = rules.radius * rules.radius;
    for o in others {
        let d2: f64 = (0..3).map(|i| (o.at[i] - earner.at[i]).powi(2)).sum();
        if o.name != earner.name && o.dimension == earner.dimension && d2 <= r2 {
            out.push((o.name, share));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friends_in_range_and_dimension_share() {
        let me = Near { name: "a", dimension: "minecraft:overworld", at: [0.0, 64.0, 0.0] };
        let others = [
            Near { name: "b", dimension: "minecraft:overworld", at: [20.0, 64.0, 20.0] },
            Near { name: "c", dimension: "minecraft:overworld", at: [40.0, 64.0, 0.0] },
            Near { name: "d", dimension: "minecraft:the_nether", at: [0.0, 64.0, 0.0] },
            Near { name: "a", dimension: "minecraft:overworld", at: [0.0, 64.0, 0.0] },
        ];
        assert_eq!(split(&me, 10.0, &others, Rules::default()), [("a", 10.0), ("b", 5.0)]);
        assert_eq!(split(&me, 10.0, &others, Rules { radius: 0.0, fraction: 0.5 }), [("a", 10.0)]);
    }
}
