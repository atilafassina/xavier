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

/// The extracted assistant text from each model for the correctness persona.
/// Both flag MULTIPLE issues in the same file (`xavier/skills/review/SKILL.md`)
/// in prose: `corr_gpt` as a bulleted list closing with `Verdict: request
/// changes`; `corr_gemini` naming the file once in a `### File:` header, then
/// numbered / `**Defect:**`-led segments.
const CORR_GPT: &str = include_str!("fixtures/corr_gpt.txt");
const CORR_GEMINI: &str = include_str!("fixtures/corr_gemini.txt");

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

#[test]
fn corr_gpt_corr_gemini_recover_multiple_located_findings() {
    // The correctness pair: both models flag MULTIPLE issues in the SAME file,
    // in prose. `corr_gpt` is a bulleted list ending `Verdict: request changes`;
    // `corr_gemini` names the file once in a `### File:` header, then leads each
    // issue with a numbered / `**Defect:**` bold. Neither uses the persona
    // format, so pre-4a both collapsed to `unknown`/unmatched. The prose stage
    // must recover several severity-bearing, LOCATABLE findings.
    let a = parse_findings(CORR_GPT, "GPT");
    let b = parse_findings(CORR_GEMINI, "Gemini");

    // Both sides recovered findings from prose, and gemini's numbered segments
    // split into several (not one folded blob).
    assert!(!a.is_empty(), "corr_gpt must recover at least one finding");
    assert!(
        b.len() > 1,
        "corr_gemini's numbered segments must recover >1 finding, got {}",
        b.len()
    );

    // Every recovered finding is locatable at the ONE shared file — proving the
    // `### File:` header (gemini) and the inline path span (gpt) were hoisted
    // onto every segment. Never a fabricated line: these fixtures name no line.
    for f in a.iter().chain(b.iter()) {
        let r = f
            .reference
            .as_ref()
            .expect("every recovered corr finding must carry the hoisted location");
        assert_eq!(r.file, "xavier/skills/review/SKILL.md");
        assert_eq!(r.line, None, "the corr fixtures name no line; none may be invented");
    }

    let result = merge(&merge_texts(CORR_GPT, CORR_GEMINI));

    // Collect every finding the merge surfaced, across all buckets.
    let all: Vec<&xavier_core::Finding> = result
        .consensus
        .iter()
        .flat_map(|p| [&p.a, &p.b])
        .chain(result.blindspot.iter())
        .chain(result.unmatched.iter())
        .collect();

    // The bar: MORE THAN ONE recovered finding carries a real (non-`unknown`)
    // severity. (Here: gpt's `request changes` -> high, plus gemini's two
    // `(Severe)` / `Request Changes` segments -> high.)
    let real_sev = all.iter().filter(|f| f.severity != "unknown").count();
    assert!(
        real_sev > 1,
        "expected >1 recovered finding with a real severity, got {real_sev} \
         (consensus={} blindspot={} unmatched={})",
        result.consensus.len(),
        result.blindspot.len(),
        result.unmatched.len(),
    );
    assert!(
        !all.iter().all(|f| f.severity == "unknown"),
        "severities must not all be `unknown`",
    );

    // Consensus is clearly right here: both models reached `request changes`
    // (high) on the same file, so the exact-key match forms at least one
    // consensus pair, and it carries a real severity on both sides.
    assert!(
        !result.consensus.is_empty(),
        "both models flag the same file at high severity -> at least one consensus"
    );
    assert!(
        result
            .consensus
            .iter()
            .any(|p| p.a.severity != "unknown" && p.b.severity != "unknown"),
        "the consensus pair must carry a real severity on both sides",
    );
}
