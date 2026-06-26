//! The mechanical merge: exact-location match plus textual near-duplicate
//! detection.
//!
//! This buckets findings the two reviewer models produced, mirroring (and
//! extending) the semantics of `parse.sh`'s `merge_and_format`:
//!
//! - **Pass 1 — exact location.** Findings whose canonical `file:line` key is
//!   equal (after [`crate::refs`] normalization, which now also collapses a
//!   line range onto its start line) are a `consensus` match. Greedy and
//!   first-match-wins; each side-`b` finding is consumed at most once. This is
//!   the original Phase 1 behavior and the high-confidence path.
//! - **Pass 2 — textual near-duplicate.** Among the findings *both* sides left
//!   unconsumed, a side-`a` finding and a side-`b` finding that point at the
//!   **same file** and whose descriptions clear
//!   [`SIMILARITY_THRESHOLD`](crate::similarity::SIMILARITY_THRESHOLD) are also
//!   a `consensus` match. This is the bug fix: "missing field `id`" and "`id`
//!   is absent" at the same file collapse into ONE consensus instead of two
//!   blindspots, even when the models disagree on the exact line or one omits
//!   it.
//!
//! Everything else is residue:
//!
//! - A located finding with **no candidate on the other side** (the other model
//!   flagged nothing in that file) is a genuine `blindspot` — one model saw a
//!   location the other did not.
//! - A located finding that *did* have a same-file candidate but which fell
//!   **below the similarity threshold** is ambiguous (same place, different
//!   words — same issue, or two distinct issues?). That is NOT mechanically
//!   decidable, so it goes to `unmatched` for the model to adjudicate.
//! - A finding with **no usable location** cannot be placed mechanically and
//!   goes to `unmatched`.
//!
//! `dispute` is always empty here — disputes are a later vault-overlay concern.
//!
//! The similarity metric is fully isolated behind [`crate::similarity`]; this
//! module only calls `similarity(..)` and reads the threshold constant, so the
//! metric can be swapped for a crate later without touching the bucketing.

use crate::model::{Finding, MatchedPair, MergeInput, MergeResult};
use crate::similarity::{similarity, SIMILARITY_THRESHOLD};

/// Run the mechanical merge over both sides of [`MergeInput`].
///
/// Output ordering is deterministic and stable: consensus pairs follow the
/// order of side `a` (exact matches first, then near-duplicate matches);
/// blindspots and unmatched list side `a`'s residue before side `b`'s. This
/// determinism matters because the result feeds a downstream model pass and we
/// want byte-stable output for equal input.
pub fn merge(input: &MergeInput) -> MergeResult {
    let MergeInput { a, b, .. } = input;

    let mut a_consumed = vec![false; a.len()];
    let mut b_consumed = vec![false; b.len()];

    let mut consensus: Vec<MatchedPair> = Vec::new();

    // --- Pass 1: exact canonical-key match (high confidence). ---
    for (i, fa) in a.iter().enumerate() {
        let Some(key) = fa.match_key() else { continue };

        let hit = b
            .iter()
            .enumerate()
            .find(|(j, fb)| !b_consumed[*j] && fb.match_key().as_deref() == Some(key.as_str()));

        if let Some((j, fb)) = hit {
            a_consumed[i] = true;
            b_consumed[j] = true;
            consensus.push(MatchedPair {
                a: fa.clone(),
                b: fb.clone(),
            });
        }
    }

    // --- Pass 2: textual near-duplicate match among the remainder. ---
    // For each unconsumed located `a`, pick the unconsumed located `b` in the
    // SAME FILE with the highest similarity; if that best score clears the
    // threshold it is a near-duplicate consensus. Ties break to the lowest `b`
    // index for determinism.
    for (i, fa) in a.iter().enumerate() {
        if a_consumed[i] {
            continue;
        }
        let Some(fa_file) = located_file(fa) else {
            continue;
        };

        let mut best: Option<(usize, f64)> = None;
        for (j, fb) in b.iter().enumerate() {
            if b_consumed[j] {
                continue;
            }
            let Some(fb_file) = located_file(fb) else {
                continue;
            };
            if fa_file != fb_file {
                continue;
            }
            let score = similarity(&fa.description, &fb.description);
            match best {
                Some((_, best_score)) if score <= best_score => {}
                _ => best = Some((j, score)),
            }
        }

        if let Some((j, score)) = best {
            if score >= SIMILARITY_THRESHOLD {
                a_consumed[i] = true;
                b_consumed[j] = true;
                consensus.push(MatchedPair {
                    a: fa.clone(),
                    b: b[j].clone(),
                });
            }
        }
    }

    // --- Residue. ---
    // A located finding is a `blindspot` only if the OTHER side has no
    // unmatched finding in the same file. If a same-file counterpart exists but
    // did not clear the threshold, the pair is ambiguous -> `unmatched`.
    let mut blindspot: Vec<Finding> = Vec::new();
    let mut unmatched: Vec<Finding> = Vec::new();

    for (i, fa) in a.iter().enumerate() {
        if a_consumed[i] {
            continue;
        }
        classify_residue(fa, b, &b_consumed, &mut blindspot, &mut unmatched);
    }
    for (j, fb) in b.iter().enumerate() {
        if b_consumed[j] {
            continue;
        }
        classify_residue(fb, a, &a_consumed, &mut blindspot, &mut unmatched);
    }

    MergeResult {
        consensus,
        blindspot,
        dispute: Vec::new(),
        unmatched,
    }
}

/// The canonical file part of a finding's reference, if it has a usable one.
fn located_file(f: &Finding) -> Option<&str> {
    f.reference
        .as_ref()
        .filter(|r| r.is_usable())
        .map(|r| r.file.as_str())
}

/// Place a single unconsumed finding into either `blindspot` or `unmatched`.
///
/// - No usable location -> `unmatched` (cannot be placed mechanically).
/// - Located, and the opposite side has an unconsumed finding in the same file
///   -> `unmatched` (a same-file candidate exists but fell below the similarity
///   threshold; the model adjudicates whether it is the same issue).
/// - Located, and the opposite side has nothing left in that file ->
///   `blindspot` (only this model saw the location).
fn classify_residue(
    f: &Finding,
    other: &[Finding],
    other_consumed: &[bool],
    blindspot: &mut Vec<Finding>,
    unmatched: &mut Vec<Finding>,
) {
    let Some(file) = located_file(f) else {
        unmatched.push(f.clone());
        return;
    };

    let has_same_file_candidate = other
        .iter()
        .enumerate()
        .any(|(k, of)| !other_consumed[k] && located_file(of) == Some(file));

    if has_same_file_candidate {
        unmatched.push(f.clone());
    } else {
        blindspot.push(f.clone());
    }
}
