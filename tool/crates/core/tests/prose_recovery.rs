//! RED tracer bullet for the "prose findings dropped" bug (Phase 1).
//!
//! `/xavier review` silently loses findings when a reviewer model answers in
//! PROSE instead of the required persona format (`### [severity] …` heading +
//! `**File**: path` line). The captured fixtures in `fixtures/` are the exact
//! assistant text two real reviewer models emitted for the SAME issue — both
//! clearly flag the SAME prompt-injection finding in the SAME file
//! (`xavier/skills/review/SKILL.md`) — yet neither uses the persona format:
//!
//! - `sec_gpt.txt` opens with a bare `## Findings` heading and bold prose; it
//!   has no `**File**:` line, so it parses to ONE `severity: "unknown"`,
//!   reference-less finding (the whole body folded into one description).
//! - `sec_gemini.txt` has no `##`/`###` heading at all (its "Finding:" is bold,
//!   and its `File:` line precedes any heading so it is ignored), so it parses
//!   to ZERO findings.
//!
//! Because neither side yields a located finding, the merge forms NO consensus
//! even though the two models plainly agree. This test PINS that bug: it asserts
//! the consensus that SHOULD exist. On today's code it FAILS by design; it flips
//! green only once the parse/merge pipeline recovers findings from prose
//! (Phase 4). Do not weaken or skip it.

use xavier_core::{merge, parse_findings, MergeInput};

/// The extracted assistant text from each model for the security persona.
const SEC_GPT: &str = include_str!("fixtures/sec_gpt.txt");
const SEC_GEMINI: &str = include_str!("fixtures/sec_gemini.txt");

/// Build a `MergeInput` from two raw model texts the way `merge-text` does
/// (identical to the helper in `golden.rs`).
fn merge_texts(text_a: &str, text_b: &str) -> MergeInput {
    MergeInput {
        a: parse_findings(text_a, "GPT"),
        b: parse_findings(text_b, "Gemini"),
        label_a: "GPT".into(),
        label_b: "Gemini".into(),
    }
}

#[test]
fn sec_gpt_sec_gemini_reach_consensus() {
    // Both reviewers flag the same prompt-injection issue in the same file, in
    // prose. The mechanical pipeline must recover a real, severity-bearing
    // consensus from that — not drop everything into `unknown`/unmatched.
    let result = merge(&merge_texts(SEC_GPT, SEC_GEMINI));

    assert!(
        !result.consensus.is_empty(),
        "sec_gpt + sec_gemini clearly agree (same prompt-injection issue, same \
         file) but formed NO consensus. consensus={} blindspot={} unmatched={}. \
         Prose findings are being dropped instead of recovered.",
        result.consensus.len(),
        result.blindspot.len(),
        result.unmatched.len(),
    );

    // The consensus must carry a real severity, not the `unknown` fallback the
    // heading parser assigns to a bracket-less prose line.
    let real_severity = result.consensus.iter().any(|pair| {
        pair.a.severity != "unknown" || pair.b.severity != "unknown"
    });
    assert!(
        real_severity,
        "consensus formed but every side is `unknown` severity: {:?}",
        result
            .consensus
            .iter()
            .map(|p| (p.a.severity.as_str(), p.b.severity.as_str()))
            .collect::<Vec<_>>(),
    );
}
