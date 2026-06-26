//! The trivial, mechanical exact-match merge (Phase 1).
//!
//! This mirrors the bucket semantics of `parse.sh`'s `merge_and_format`:
//!
//! - Findings are matched by **exact** normalized `file:line` reference.
//! - Matching is greedy and first-match-wins: each `a` finding takes the first
//!   not-yet-consumed `b` finding with the same key; each `b` finding is
//!   consumed at most once.
//! - Findings with an empty/unusable reference are never matched.
//!
//! Where this intentionally diverges from `parse.sh`: the shell drops
//! reference-less findings into Blindspots, but the binary's determinism
//! boundary requires it to surface anything it cannot mechanically place in
//! the [`MergeResult::unmatched`] residue instead. Reference-less findings are
//! therefore `unmatched`, not `blindspot`.
//!
//! Fuzzy / paraphrase matching is explicitly NOT done here — it is a later
//! phase.

use crate::model::{Finding, MatchedPair, MergeInput, MergeResult};

/// Run the trivial exact-match merge over both sides of [`MergeInput`].
///
/// Output ordering is deterministic and stable: consensus pairs follow the
/// order of side `a`; blindspots list side `a`'s residue before side `b`'s;
/// unmatched likewise lists `a` before `b`. This determinism matters because
/// the result feeds a downstream model pass and we want byte-stable output for
/// equal input.
pub fn merge(input: &MergeInput) -> MergeResult {
    let MergeInput { a, b, .. } = input;

    // Track which side-`b` findings have already been claimed by a consensus
    // match so each is consumed at most once.
    let mut b_consumed = vec![false; b.len()];

    let mut consensus: Vec<MatchedPair> = Vec::new();
    let mut blindspot: Vec<Finding> = Vec::new();
    let mut unmatched: Vec<Finding> = Vec::new();

    // Side A: place each finding into consensus / blindspot / unmatched.
    for fa in a.iter() {
        let key = match fa.match_key() {
            // No usable location: cannot be mechanically placed — defer.
            None => {
                unmatched.push(fa.clone());
                continue;
            }
            Some(k) => k,
        };

        // First not-yet-consumed b finding with the same key wins.
        let hit = b
            .iter()
            .enumerate()
            .find(|(j, fb)| !b_consumed[*j] && fb.match_key().as_deref() == Some(key.as_str()));

        match hit {
            Some((j, fb)) => {
                b_consumed[j] = true;
                consensus.push(MatchedPair {
                    a: fa.clone(),
                    b: fb.clone(),
                });
            }
            None => blindspot.push(fa.clone()),
        }
    }

    // Side B residue: anything not consumed by a consensus match. Reference-less
    // findings go to unmatched; the rest are blindspots.
    for (j, fb) in b.iter().enumerate() {
        if b_consumed[j] {
            continue;
        }
        match fb.match_key() {
            None => unmatched.push(fb.clone()),
            Some(_) => blindspot.push(fb.clone()),
        }
    }

    // `dispute` is always empty from the mechanical merge — disputes are a
    // later vault-overlay concern.
    MergeResult {
        consensus,
        blindspot,
        dispute: Vec::new(),
        unmatched,
    }
}
