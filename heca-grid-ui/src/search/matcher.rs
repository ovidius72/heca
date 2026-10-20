//! Fuzzy subsequence matching and its case policy. No memory, no clock — given a
//! query and a string, how well does it fit and which characters did it land on.

/// A match: how well the query fits, and which characters it landed on.
///
/// The hits are what a [`Label`](crate::widgets::Label) draws as marks, which is why they are
/// character indices into the source text and not byte offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// Higher is better.
    pub score: i32,
    /// Indices of the matched characters, in order.
    pub hits: Vec<usize>,
}

/// How a query's case is treated.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MatchCase {
    /// Case-insensitive **until the query carries an uppercase character**, then sensitive. What
    /// fzf, ripgrep and most editors call smart-case: typing lowercase finds anything, and reaching
    /// for Shift is how you say you meant it.
    #[default]
    Smart,
    /// Always case-sensitive.
    Sensitive,
    /// Never case-sensitive.
    Insensitive,
}

impl MatchCase {
    /// Does a query in this mode match case-sensitively?
    pub fn is_sensitive(self, query: &str) -> bool {
        match self {
            MatchCase::Smart => smart_case(query),
            MatchCase::Sensitive => true,
            MatchCase::Insensitive => false,
        }
    }
}

/// Should this query match case-sensitively under **smart-case**? Only when it carries an uppercase
/// character.
///
/// Named rather than left inline so the second search surface cannot spell the rule differently.
pub fn smart_case(query: &str) -> bool {
    query.chars().any(|c| c.is_uppercase())
}

/// Fuzzy subsequence match: every query character must appear in `text`, in order.
///
/// Scores consecutive runs (+5) and start-of-word hits (+8), and prefers tighter matches. An empty
/// query matches everything with a score of zero — which is what lets frecency alone order a list
/// nobody has typed into yet.
///
/// **The best alignment wins, not the first one.** A single greedy pass takes each query character
/// where it first appears, which reads the query out of whatever happens to come earliest: `left`
/// against `Toggle Left Sidebar` took the `l` and `e` out of *Toggle*, then jumped to the `f` and
/// `t` of *Left*, never matching the word at all. That scattered reading also scored the same as a
/// clean one, so the ranking between such rows was arbitrary — and the highlight, which draws
/// exactly what was matched, showed the nonsense plainly.
pub fn fuzzy(query: &str, text: &str, case_sensitive: bool) -> Option<Match> {
    let q: Vec<char> = query.chars().collect();
    if q.is_empty() {
        return Some(Match {
            score: 0,
            hits: Vec::new(),
        });
    }
    let t: Vec<char> = text.chars().collect();
    let norm = |c: char| {
        if case_sensitive {
            c
        } else {
            c.to_ascii_lowercase()
        }
    };
    // Every place the query could begin. Few in practice — one per occurrence of its first
    // character — and each walk stops at the first failure.
    (0..t.len())
        .filter(|&start| norm(t[start]) == norm(q[0]))
        .filter_map(|start| match_from(&q, &t, start, &norm))
        .max_by_key(|m| m.score)
}

/// One greedy walk, pinned to begin at `start`.
fn match_from(q: &[char], t: &[char], start: usize, norm: &impl Fn(char) -> char) -> Option<Match> {
    let mut qi = 0;
    let mut hits = Vec::with_capacity(q.len());
    let mut score = 0i32;
    let mut prev: Option<usize> = None;
    for ti in start..t.len() {
        if norm(t[ti]) != norm(q[qi]) {
            continue;
        }
        score += 1;
        if prev == Some(ti.wrapping_sub(1)) {
            score += 5; // consecutive run
        }
        if ti == 0 || !t[ti - 1].is_alphanumeric() {
            score += 8; // start of a word
        }
        hits.push(ti);
        prev = Some(ti);
        qi += 1;
        if qi == q.len() {
            // Prefer shorter / tighter matches.
            score -= (t.len() as i32 - q.len() as i32) / 4;
            return Some(Match { score, hits });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_matches_subsequence_and_scores_consecutive_higher() {
        assert!(fuzzy("xyz", "abc", false).is_none(), "not a subsequence");
        assert!(fuzzy("ace", "abcde", false).is_some());
        assert_eq!(fuzzy("ce", "abcde", false).unwrap().hits, vec![2, 4]);
        let consecutive = fuzzy("ab", "abxx", false).unwrap().score;
        let scattered = fuzzy("ab", "axbx", false).unwrap().score;
        assert!(consecutive > scattered, "{consecutive} vs {scattered}");
        // An empty query matches everything at zero — which is what lets frecency alone order a
        // list nobody has typed into.
        assert_eq!(fuzzy("", "anything", false).unwrap().score, 0);
    }

    /// **The best alignment, not the first.** The reported case: `left` against
    /// `Toggle Left Sidebar` must match the word *Left*, not the `l`/`e` inside *Toggle* followed
    /// by the `f`/`t` of *Left*. The hits are what gets highlighted, so a wrong alignment is
    /// visible on screen — and it scored the same as a clean match, which made the ranking between
    /// such rows arbitrary.
    #[test]
    fn the_best_alignment_wins_not_the_first_one() {
        let m = fuzzy("left", "Toggle Left Sidebar", false).expect("it matches");
        assert_eq!(
            m.hits,
            vec![7, 8, 9, 10],
            "the word Left, not letters out of Toggle"
        );
        // And that reading scores strictly better than the scattered one a greedy pass would take.
        let scattered = super::match_from(
            &"left".chars().collect::<Vec<_>>(),
            &"Toggle Left Sidebar".chars().collect::<Vec<_>>(),
            4,
            &|c: char| c.to_ascii_lowercase(),
        )
        .expect("the scattered reading exists");
        assert!(
            m.score > scattered.score,
            "{} vs {}",
            m.score,
            scattered.score
        );
    }

    #[test]
    fn smart_case_is_one_rule() {
        assert!(!smart_case("git"));
        assert!(smart_case("Git"));
        assert!(fuzzy("git", "Git Push", smart_case("git")).is_some());
        assert!(fuzzy("G", "git pull", smart_case("G")).is_none());
    }
}
