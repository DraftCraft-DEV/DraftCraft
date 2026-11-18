//! Adaptive AutoComplete: the command line learns which commands its user runs and lists the
//! ones used most, and most recently, first (as AutoCAD's adaptive suggestions and the fish
//! shell do), without any alias editing.
//!
//! Each command run from the command line adds one to its score. Scores fade with use, not with
//! time: every later command run counts one tick of a clock, and a score halves every
//! [`HALF_LIFE`] ticks. So a command used often lately ranks above one used often long ago, a
//! weekend away changes nothing, and the result depends only on what was run, in what order
//! (deterministic, and the same on the web).
//!
//! The counts are kept with the interface preferences ([`crate::CadApp::prefs_json`]); what is
//! loaded back is treated as hostile: malformed entries are dropped and everything is capped.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

/// A score halves after this many later command runs.
pub const HALF_LIFE: f64 = 300.0;
/// The most commands remembered; the lowest scores go first.
pub const MAX_ENTRIES: usize = 200;
/// The highest score kept. Steady use of one command alone tends to about `HALF_LIFE / ln 2`
/// (≈ 433), so this only bounds what a stored file claims.
pub const MAX_SCORE: f64 = 1000.0;
/// What an exactly typed alias is worth in the ranking, in uses: another command matching the
/// same letters lists above it (and Enter runs it) once its score is higher than this.
pub const EXACT_ALIAS_BONUS: f64 = 3.0;
/// The clock is rebased before it reaches this, so ages always stay exact in an `f64`.
const MAX_CLOCK: u64 = 1 << 40;
/// The longest command id kept (the longest real id is far shorter).
const MAX_ID_LEN: usize = 64;
/// The most entries read from stored preferences before the rest are ignored.
const MAX_LOADED: usize = MAX_ENTRIES * 4;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Entry {
    /// The score as of `at`.
    score: f64,
    /// The clock when the command was last run.
    at: u64,
}

/// What the command line has learned about its user's commands.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Usage {
    /// Command runs recorded so far (since the last rebase).
    clock: u64,
    /// Command id (lower case) → its score.
    entries: BTreeMap<String, Entry>,
}

/// `score` after `age` later command runs: halved every [`HALF_LIFE`] runs.
pub fn decayed(score: f64, age: u64) -> f64 {
    if !score.is_finite() || score <= 0.0 {
        return 0.0;
    }
    score * 0.5f64.powf(age as f64 / HALF_LIFE)
}

/// The order of items with these `weights`: highest first; equal weights keep their order.
pub fn rank(weights: &[f64]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..weights.len()).collect();
    let w = |i: usize| weights.get(i).copied().filter(|w| !w.is_nan()).unwrap_or(0.0);
    order.sort_by(|&a, &b| w(b).total_cmp(&w(a)));
    order
}

/// Whether `id` looks like a command id worth keeping.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ID_LEN && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-+".contains(&b))
}

impl Usage {
    /// Nothing learned yet.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many commands are remembered.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Forget everything learned.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// `id`'s score now (0 for a command never run).
    pub fn score(&self, id: &str) -> f64 {
        let id = id.to_ascii_lowercase();
        self.entries.get(&id).map_or(0.0, |e| decayed(e.score, self.clock.saturating_sub(e.at)))
    }

    /// The command `id` was run from the command line.
    pub fn record(&mut self, id: &str) {
        let id = id.to_ascii_lowercase();
        if !valid_id(&id) {
            return;
        }
        if self.clock >= MAX_CLOCK {
            self.rebase();
        }
        let now = self.clock;
        let before = self.score(&id);
        self.entries.insert(id, Entry { score: (before + 1.0).min(MAX_SCORE), at: now });
        self.clock = now.saturating_add(1);
        self.prune();
    }

    /// Bring every score to now and restart the clock at zero.
    fn rebase(&mut self) {
        let clock = self.clock;
        for e in self.entries.values_mut() {
            e.score = decayed(e.score, clock.saturating_sub(e.at));
            e.at = 0;
        }
        self.clock = 0;
    }

    /// Drop the lowest scores until at most [`MAX_ENTRIES`] remain.
    fn prune(&mut self) {
        while self.entries.len() > MAX_ENTRIES {
            let lowest = self
                .entries
                .iter()
                .map(|(id, e)| (id, decayed(e.score, self.clock.saturating_sub(e.at))))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(id, _)| id.clone());
            match lowest {
                Some(id) => self.entries.remove(&id),
                None => break,
            };
        }
    }

    /// The learned scores as JSON for the preferences: `{"clock": n, "commands": {"line": [score, at]}}`.
    pub fn to_json(&self) -> Value {
        let commands: Map<String, Value> = self.entries.iter().map(|(id, e)| (id.clone(), json!([e.score, e.at]))).collect();
        json!({ "clock": self.clock, "commands": commands })
    }

    /// Read what [`Self::to_json`] wrote. Hostile input is expected: a malformed whole gives
    /// nothing learned, a malformed entry is skipped, and the sizes are capped.
    pub fn from_json(v: &Value) -> Self {
        let mut out = Self::default();
        let Some(clock) = v.get("clock").and_then(Value::as_u64).filter(|c| *c <= MAX_CLOCK) else { return out };
        let Some(commands) = v.get("commands").and_then(Value::as_object) else { return out };
        out.clock = clock;
        for (id, e) in commands.iter().take(MAX_LOADED) {
            let id = id.to_ascii_lowercase();
            let score = e.get(0).and_then(Value::as_f64).filter(|s| s.is_finite() && *s > 0.0 && *s <= MAX_SCORE);
            let at = e.get(1).and_then(Value::as_u64).filter(|at| *at <= clock);
            if let (true, Some(score), Some(at)) = (valid_id(&id), score, at) {
                out.entries.insert(id, Entry { score, at });
            }
        }
        out.prune();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_score_halves_every_half_life_and_never_goes_negative() {
        assert_eq!(decayed(4.0, 0), 4.0);
        assert!((decayed(4.0, HALF_LIFE as u64) - 2.0).abs() < 1e-9);
        assert!((decayed(4.0, 2 * HALF_LIFE as u64) - 1.0).abs() < 1e-9);
        assert_eq!(decayed(-1.0, 3), 0.0);
        assert_eq!(decayed(f64::NAN, 3), 0.0);
        assert_eq!(decayed(f64::INFINITY, 3), 0.0);
        assert!(decayed(MAX_SCORE, u64::MAX) >= 0.0);
    }

    #[test]
    fn ranking_is_by_weight_and_ties_keep_their_order() {
        assert_eq!(rank(&[0.0, 0.0, 0.0]), vec![0, 1, 2]);
        assert_eq!(rank(&[1.0, 3.0, 1.0, 2.0]), vec![1, 3, 0, 2]);
        assert_eq!(rank(&[f64::NAN, 1.0]), vec![1, 0]);
        assert_eq!(rank(&[f64::INFINITY, 5.0]), vec![0, 1]);
        assert!(rank(&[]).is_empty());
    }

    #[test]
    fn recent_use_outranks_old_use() {
        let mut u = Usage::default();
        for _ in 0..5 {
            u.record("list");
        }
        for _ in 0..5 {
            u.record("line");
        }
        // Same count, but LINE is more recent.
        assert!(u.score("line") > u.score("list"));
        assert!(u.score("LINE") > 4.9, "ids are case-insensitive");
        // Many other commands later, LIST's five old uses have faded below one fresh use.
        for _ in 0..(3 * HALF_LIFE as usize) {
            u.record("pan");
        }
        u.record("erase");
        assert!(u.score("list") < u.score("erase"), "{} < {}", u.score("list"), u.score("erase"));
        assert_eq!(u.score("never"), 0.0);
    }

    #[test]
    fn sizes_and_scores_are_capped() {
        let mut u = Usage::default();
        for i in 0..(MAX_ENTRIES + 50) {
            u.record(&format!("cmd{i}"));
        }
        assert_eq!(u.len(), MAX_ENTRIES);
        // The oldest were the ones dropped.
        assert_eq!(u.score("cmd0"), 0.0);
        assert!(u.score(&format!("cmd{}", MAX_ENTRIES + 49)) > 0.0);
        let mut one = Usage::default();
        for _ in 0..5000 {
            one.record("line");
        }
        assert!(one.score("line") <= MAX_SCORE);
        // Ids that are no command id are not kept.
        let mut bad = Usage::default();
        bad.record("");
        bad.record(&"x".repeat(MAX_ID_LEN + 1));
        bad.record("line; rm");
        assert!(bad.is_empty());
    }

    #[test]
    fn the_clock_is_rebased_without_changing_scores() {
        let mut u = Usage { clock: MAX_CLOCK, entries: BTreeMap::new() };
        u.entries.insert("line".into(), Entry { score: 8.0, at: MAX_CLOCK - HALF_LIFE as u64 });
        let before = u.score("line");
        u.record("list");
        assert!(u.clock < 10);
        assert!((u.score("line") - before).abs() < 0.1, "{} vs {before}", u.score("line"));
    }

    #[test]
    fn learned_scores_survive_a_round_trip() {
        let mut u = Usage::default();
        for id in ["line", "line", "circle", "list"] {
            u.record(id);
        }
        let back = Usage::from_json(&u.to_json());
        assert_eq!(back, u);
        let mut cleared = back;
        cleared.clear();
        assert!(cleared.is_empty());
    }

    #[test]
    fn corrupt_stored_values_are_ignored() {
        let good = |v: Value| Usage::from_json(&v);
        // Not the expected shape at all.
        for v in [json!(null), json!("line"), json!([1, 2]), json!({"clock": -1, "commands": {}}), json!({"clock": 3})] {
            assert!(good(v).is_empty());
        }
        assert!(good(json!({"clock": u64::MAX, "commands": {"line": [1.0, 0]}})).is_empty());
        // Malformed entries are dropped, good ones kept.
        let u = good(json!({
            "clock": 10,
            "commands": {
                "line": [4.0, 9],
                "list": ["many", 1],
                "copy": [1e300, 1],
                "move": [-2.0, 1],
                "pan": [1.0, 11],
                "erase": [1.0],
                "trim": {"score": 1},
                "bad id!": [1.0, 1],
                "": [1.0, 1]
            }
        }));
        assert_eq!(u.len(), 1);
        assert!(u.score("line") > 3.9);
        // A stored file listing far too many commands keeps only the best.
        let many: Map<String, Value> = (0..5000).map(|i| (format!("c{i:05}"), json!([1.0 + f64::from(i) / 10000.0, 0]))).collect();
        let u = good(json!({"clock": 1, "commands": many}));
        assert_eq!(u.len(), MAX_ENTRIES);
    }
}
