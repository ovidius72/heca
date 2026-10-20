//! Ranking by use: how often and how recently each entry has been chosen, as a
//! bounded score added to a match. No clock — recency is distance in a use-sequence.

use super::DEFAULT_USAGE_CAP;
use std::collections::HashMap;

/// How much a match's **use count** is worth: `COUNT_WEIGHT × log2(1 + count)`, so the first few
/// uses separate an entry and the hundredth barely moves it.
///
/// Computed in floating point on purpose. Integer `log2` only steps at 1, 3, 7, 15 — so one use and
/// two scored *identically*, and going from the first to the second time you ran something changed
/// nothing at all. That is the boundary a user actually notices.
const COUNT_WEIGHT: f64 = 8.0;
/// How much a match's **recency** is worth, decaying as `RECENCY_WEIGHT / (1 + age)`.
///
/// Deliberately **below** what a second use is worth: running something twice puts it above a single
/// more-recent run, and recency then separates entries of equal count. "I reach for this often" is a
/// steadier signal than "I touched this last".
const RECENCY_WEIGHT: i32 = 6;
/// The ceiling on the whole frecency term.
///
/// **The cap is the design decision.** A textual score is small — a start-of-word hit is +8 — so an
/// uncapped frecency term would let an entry the user picks often outrank one the query actually
/// spells better, and typing would stop feeling like it did anything. Frecency decides ties and
/// near-ties; a clearly better match still wins.
///
/// It must also sit **above** where ordinary use lands, or it stops being a ceiling and becomes the
/// answer: an early calibration capped at 18 while five-uses-recently and once-just-now both summed
/// past it, so the two scored identically and the ranking said nothing. Everyday values land around
/// 8–20; this is the wall for the outliers.
const BOOST_MAX: i32 = 28;

/// One entry's usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Use {
    count: u32,
    last_seq: u64,
}

/// How often and how recently each entry has been chosen.
///
/// **Recency is distance in the use-sequence, never a timestamp.** No clock in a UI library means no
/// clock skew, no timezone and no sleeping in a test — a test asserts an exact number. The sequence
/// is part of what gets persisted, or every restart would reset "everything is equally old".
#[derive(Debug, Clone)]
pub struct Frecency {
    uses: HashMap<String, Use>,
    seq: u64,
    cap: usize,
}

impl Default for Frecency {
    fn default() -> Self {
        Self {
            uses: HashMap::new(),
            seq: 0,
            cap: DEFAULT_USAGE_CAP,
        }
    }
}

impl Frecency {
    /// An empty table — the first-run state, in which [`boost`](Self::boost) is always 0 and a list's
    /// order is exactly what the matcher alone would give.
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty table keeping at most `cap` entries.
    pub fn with_cap(cap: usize) -> Self {
        Self {
            cap,
            ..Self::default()
        }
    }

    /// How many entries this table keeps.
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// Record that `id` was chosen.
    pub fn record(&mut self, id: &str) {
        self.seq += 1;
        let seq = self.seq;
        let entry = self.uses.entry(id.to_string()).or_insert(Use {
            count: 0,
            last_seq: seq,
        });
        entry.count = entry.count.saturating_add(1);
        entry.last_seq = seq;
        self.prune();
    }

    /// What this id adds to a match score. 0 for an id never chosen.
    pub fn boost(&self, id: &str) -> i32 {
        let Some(u) = self.uses.get(id) else {
            return 0;
        };
        // 1 use -> 8, 2 -> 12, 3 -> 16, 7 -> 24 … diminishing, but every early use counts.
        let count_term = (COUNT_WEIGHT * (f64::from(u.count) + 1.0).log2()) as i32;
        let age = self.seq.saturating_sub(u.last_seq);
        let recency_term = RECENCY_WEIGHT / (1 + age.min(i32::MAX as u64) as i32);
        (count_term + recency_term).min(BOOST_MAX)
    }

    /// Every entry, for the host that persists this.
    pub fn entries(&self) -> impl Iterator<Item = (&str, u32, u64)> {
        self.uses
            .iter()
            .map(|(id, u)| (id.as_str(), u.count, u.last_seq))
    }

    /// The current use-sequence — persisted with the entries, or recency resets on restart.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// Rebuild from persisted state.
    pub fn restore(
        seq: u64,
        cap: usize,
        entries: impl IntoIterator<Item = (String, u32, u64)>,
    ) -> Self {
        let mut out = Self {
            uses: entries
                .into_iter()
                .map(|(id, count, last_seq)| (id, Use { count, last_seq }))
                .collect(),
            seq,
            cap,
        };
        out.prune();
        out
    }

    /// Keep the table bounded: a long-lived install must not grow without limit. The least valuable
    /// entries go, which is the same judgement `boost` makes.
    fn prune(&mut self) {
        if self.uses.len() <= self.cap {
            return;
        }
        let mut ranked: Vec<(String, i32)> = self
            .uses
            .keys()
            .map(|id| (id.clone(), self.boost(id)))
            .collect();
        ranked.sort_by_key(|(_, boost)| *boost);
        for (id, _) in ranked.into_iter().take(self.uses.len() - self.cap) {
            self.uses.remove(&id);
        }
    }
}
