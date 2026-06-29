//! Pure-Rust textual similarity for near-duplicate finding detection.
//!
//! Two reviewers frequently flag the *same* issue at the *same* location in
//! different words ("missing field `id`" vs "`id` is absent from the
//! response"). Exact `file:line` matching already collapses those when both
//! sides agree on the exact location; this module adds the *textual* axis so
//! the merge can recognize the same issue when the models phrase it differently
//! and disagree on the precise line (e.g. a single line vs. a span), and —
//! just as importantly — keep two genuinely different issues at the same file
//! apart.
//!
//! # Algorithm (and why)
//!
//! The crates the task pointed at (`similar` / `imara-diff` / `strsim`) are not
//! reachable in this build, so the metric is hand-written from the standard
//! library. It is still a composition of standard, well-understood metrics, not
//! an ad-hoc heuristic:
//!
//! 1. **Content tokenization.** Lowercase, split on non-alphanumerics, then drop
//!    (a) very short tokens and (b) a fixed list of English function words and
//!    *generic review filler* ("missing", "consider", "issue", "function", …).
//!    Those words carry no localizing signal — both "missing X" and "missing Y"
//!    contain "missing" — so leaving them in makes unrelated findings look
//!    similar. What remains are the content words that actually identify the
//!    issue ("sql", "input", "query", "id", "null", …).
//! 2. **Overlap coefficient** of the two content-token *sets*:
//!    `|A ∩ B| / min(|A|, |B|)`. This is robust to one reviewer being more
//!    verbose than the other (Jaccard would be punished by the extra words),
//!    which is the common case for paraphrases.
//! 3. **Normalized Levenshtein** (`1 - edit_distance / max_len`) over the
//!    **sorted content tokens joined by spaces**. Sorting makes it
//!    order-independent (reordered paraphrases score high) and dropping filler
//!    first means structural filler like "for the … call" cannot inflate it.
//!    This rescues pairs that share a key word with a different surface form
//!    (e.g. plural/inflection) where the set overlap alone is low.
//!
//! The combined score is `max(overlap, normalized_levenshtein)`: a pair is
//! similar if *either* lens says so, which matches how a human reads two
//! paraphrases.
//!
//! # Threshold
//!
//! [`SIMILARITY_THRESHOLD`] is `0.30`. It was calibrated against realistic
//! reviewer paraphrases (the golden fixtures): genuine same-issue pairs scored
//! in `0.33..=1.0` (the weakest being "missing null check before deref" vs
//! "pointer dereferenced without a null guard", which share only "null" →
//! `0.33`), while genuinely distinct findings — even ones sharing generic
//! review vocabulary — scored `<= 0.23`. `0.30` sits in that empirical gap with
//! margin on both sides. It is deliberately on the lower side of that gap
//! because the residue of a *false split* is merely handed to the model to
//! adjudicate (cheap), whereas a *false merge* hides a real second finding
//! (expensive) — but the gap is wide enough that `0.30` does not actually admit
//! any of the distinct fixtures. The value is unit-pinned by the tests in this
//! module and by the golden merge fixtures, so any future tuning is a visible,
//! deliberate change.
//!
//! # Isolation for a later crate swap
//!
//! Everything the merge needs is behind two items: the [`SIMILARITY_THRESHOLD`]
//! constant and the [`similarity`] function (`(&str, &str) -> f64` in
//! `0.0..=1.0`). `core::merge` calls only those. To swap in `strsim`/`similar`
//! later, replace the body of [`similarity`] (and optionally retune the
//! constant) — no caller changes, and the golden tests pin the
//! externally-observable behavior.

use std::collections::BTreeSet;

/// Findings whose [`similarity`] is at least this are treated as the same issue
/// (a near-duplicate) when they also share a file. See the module docs for the
/// calibration and justification.
pub const SIMILARITY_THRESHOLD: f64 = 0.30;

/// English function words plus generic code-review filler that carry no
/// localizing signal. Kept small and fixed; sorted only for readability.
const STOPWORDS: &[&str] = &[
    // function words
    "the",
    "a",
    "an",
    "is",
    "are",
    "was",
    "were",
    "be",
    "been",
    "being",
    "to",
    "of",
    "in",
    "on",
    "at",
    "for",
    "and",
    "or",
    "not",
    "no",
    "without",
    "with",
    "into",
    "from",
    "that",
    "which",
    "can",
    "could",
    "this",
    "it",
    "its",
    "as",
    "by",
    "but",
    "so",
    "if",
    "then",
    "than",
    "there",
    "here",
    "when",
    "while",
    "does",
    "do",
    "done",
    "has",
    "have",
    "had",
    "will",
    "would",
    "should",
    "may",
    "might",
    "must",
    "via",
    "per",
    "up",
    "out",
    "off",
    "over",
    "under",
    "before",
    "after",
    "because",
    "about",
    "also",
    "only",
    "any",
    "all",
    "some",
    "each",
    // generic review filler (no localization value)
    "missing",
    "unused",
    "possible",
    "potential",
    "consider",
    "add",
    "added",
    "adds",
    "use",
    "used",
    "uses",
    "using",
    "issue",
    "finding",
    "code",
    "function",
    "method",
    "line",
    "file",
    "value",
    "field",
    "name",
    "names",
    "naming",
];

/// Textual similarity of two finding descriptions in `0.0..=1.0`.
///
/// `1.0` is identical-after-normalization (or token-reordered); `0.0` is no
/// shared content. The score is `max(overlap_coefficient, normalized_levenshtein)`
/// over content tokens. Deterministic and symmetric:
/// `similarity(a, b) == similarity(b, a)`.
pub fn similarity(a: &str, b: &str) -> f64 {
    let ta = content_tokens(a);
    let tb = content_tokens(b);

    // Two contentless strings are vacuously identical; one contentless shares
    // nothing meaningful.
    if ta.is_empty() && tb.is_empty() {
        return 1.0;
    }
    if ta.is_empty() || tb.is_empty() {
        return 0.0;
    }

    let overlap = overlap_coefficient(&ta, &tb);
    let lev = normalized_levenshtein(&joined(&ta), &joined(&tb));
    overlap.max(lev)
}

/// Lowercase, split on non-alphanumerics, drop short tokens and stopwords, and
/// dedupe into a stable (sorted) set of content tokens.
fn content_tokens(s: &str) -> BTreeSet<String> {
    fn flush(cur: &mut String, set: &mut BTreeSet<String>) {
        if cur.len() > 1 && !STOPWORDS.contains(&cur.as_str()) {
            set.insert(std::mem::take(cur));
        } else {
            cur.clear();
        }
    }

    let mut set = BTreeSet::new();
    let mut cur = String::new();
    for ch in s.chars() {
        if ch.is_alphanumeric() {
            for lc in ch.to_lowercase() {
                cur.push(lc);
            }
        } else {
            flush(&mut cur, &mut set);
        }
    }
    flush(&mut cur, &mut set);
    set
}

/// `|A ∩ B| / min(|A|, |B|)` — the Szymkiewicz–Simpson overlap coefficient.
fn overlap_coefficient(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f64 {
    let min_len = a.len().min(b.len());
    if min_len == 0 {
        return 0.0;
    }
    let inter = a.intersection(b).count();
    inter as f64 / min_len as f64
}

/// Join a sorted content-token set into a single space-delimited string for the
/// edit-distance lens (order-independent because the set is sorted).
fn joined(set: &BTreeSet<String>) -> String {
    let mut s = String::new();
    for (i, t) in set.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(t);
    }
    s
}

/// `1 - levenshtein(a, b) / max(len_a, len_b)` over `char`s, in `0.0..=1.0`.
fn normalized_levenshtein(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let max_len = a.len().max(b.len());
    if max_len == 0 {
        return 1.0;
    }
    let dist = levenshtein(&a, &b);
    1.0 - (dist as f64 / max_len as f64)
}

/// Classic two-row dynamic-programming Levenshtein edit distance over `char`
/// slices. O(len_a * len_b) time, O(min) space.
fn levenshtein(a: &[char], b: &[char]) -> usize {
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    // Keep the shorter sequence as the row width to bound memory.
    let (a, b) = if a.len() < b.len() { (b, a) } else { (a, b) };

    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur: Vec<usize> = vec![0; b.len() + 1];

    for (i, &ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1) // deletion
                .min(cur[j] + 1) // insertion
                .min(prev[j] + cost); // substitution
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_is_one() {
        assert_eq!(similarity("missing null check", "missing null check"), 1.0);
    }

    #[test]
    fn normalization_ignores_punctuation_and_case() {
        // Same content words, different casing/punctuation/backticks -> 1.0.
        let s = similarity("SQL injection in `query`.", "sql injection query");
        assert!(s > 0.99, "got {s}");
    }

    #[test]
    fn paraphrase_same_issue_clears_threshold() {
        // The canonical bug case: same issue, different words.
        let s = similarity(
            "The response is missing the `id` field",
            "`id` field is absent from the response",
        );
        assert!(
            s >= SIMILARITY_THRESHOLD,
            "paraphrase should clear threshold, got {s}"
        );
    }

    #[test]
    fn reordered_tokens_score_high_via_sorted_levenshtein() {
        let s = similarity(
            "null pointer dereference in handler",
            "handler dereference null pointer",
        );
        assert!(s >= 0.99, "reordered same tokens -> ~1.0, got {s}");
    }

    #[test]
    fn weakest_real_paraphrase_still_clears_threshold() {
        // Shares only "null" — the weakest same-issue pair in the fixtures.
        let s = similarity(
            "Missing null check before deref",
            "Pointer dereferenced without a null guard",
        );
        assert!(s >= SIMILARITY_THRESHOLD, "got {s}");
    }

    #[test]
    fn distinct_issues_stay_below_threshold() {
        let s = similarity(
            "Off-by-one error in the loop bound",
            "Unvalidated user input is logged at info level",
        );
        assert!(
            s < SIMILARITY_THRESHOLD,
            "distinct issues should stay below threshold, got {s}"
        );
    }

    #[test]
    fn shared_filler_words_do_not_force_a_match() {
        // Both start with "missing" (a stopword) but are otherwise unrelated.
        let s = similarity(
            "missing error handling for the network call",
            "missing documentation for the public API",
        );
        assert!(s < SIMILARITY_THRESHOLD, "got {s}");
    }

    #[test]
    fn symmetric() {
        let a = "the cache is never invalidated on write";
        let b = "writes do not invalidate the cache entry";
        assert_eq!(similarity(a, b), similarity(b, a));
    }

    #[test]
    fn empty_and_stopword_only_handling() {
        assert_eq!(similarity("", ""), 1.0);
        assert_eq!(similarity("anything", ""), 0.0);
        // A string of pure stopwords has no content tokens.
        assert_eq!(similarity("the and of to", "the and of to"), 1.0);
        assert_eq!(similarity("the and of to", "sql injection"), 0.0);
    }

    #[test]
    fn levenshtein_basic() {
        let a: Vec<char> = "kitten".chars().collect();
        let b: Vec<char> = "sitting".chars().collect();
        assert_eq!(levenshtein(&a, &b), 3);
    }
}
