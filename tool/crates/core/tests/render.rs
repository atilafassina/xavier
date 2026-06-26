//! Tests for the debate-Markdown rendering of a MergeResult.

use xavier_core::{debate_markdown, merge, CanonRef, Finding, MergeInput};

fn finding(severity: &str, file_ref: Option<&str>, desc: &str, source: &str) -> Finding {
    Finding {
        severity: severity.to_string(),
        reference: file_ref.map(CanonRef::parse),
        description: desc.to_string(),
        suggestion: None,
        source: Some(source.to_string()),
    }
}

#[test]
fn render_has_all_three_sections() {
    let input = MergeInput {
        a: vec![],
        b: vec![],
        label_a: "GPT".into(),
        label_b: "Gemini".into(),
    };
    let md = debate_markdown(&merge(&input), &input.label_a, &input.label_b);

    // The pilot fish detects debate format by these three headings.
    assert!(md.contains("## Consensus"));
    assert!(md.contains("## Disputes"));
    assert!(md.contains("## Blindspots"));

    // Empty buckets produce the same "no findings" prose as parse.sh.
    assert!(md.contains("No consensus findings"));
    assert!(md.contains("No blindspots"));
    assert!(md.contains("No disputes from merge"));
}

#[test]
fn render_consensus_merges_severity_and_attributes_suggestions() {
    let mut a = finding("high", Some("src/main.rs:42"), "off by one", "GPT");
    a.suggestion = Some("use <=".into());
    let mut b = finding("medium", Some("src/main.rs:42"), "loop bound", "Gemini");
    b.suggestion = Some("guard the index".into());

    let input = MergeInput {
        a: vec![a],
        b: vec![b],
        label_a: "GPT".into(),
        label_b: "Gemini".into(),
    };
    let md = debate_markdown(&merge(&input), &input.label_a, &input.label_b);

    // Differing severities are joined with " / " like parse.sh.
    assert!(md.contains("### [high / medium] off by one"));
    assert!(md.contains("**File**: src/main.rs:42"));
    // Each side's suggestion is attributed by label.
    assert!(md.contains("**Suggestion**: GPT: use <="));
    assert!(md.contains("**Suggestion**: Gemini: guard the index"));
}

#[test]
fn render_blindspot_includes_unmatched_residue() {
    // A located one-sided finding (blindspot) and a reference-less finding
    // (unmatched) should BOTH appear under Blindspots, matching parse.sh.
    let a = vec![
        finding("high", Some("only/here.rs:1"), "located blindspot", "GPT"),
        finding("low", None, "no location", "GPT"),
    ];
    let input = MergeInput {
        a,
        b: vec![],
        label_a: "GPT".into(),
        label_b: "Gemini".into(),
    };
    let md = debate_markdown(&merge(&input), &input.label_a, &input.label_b);

    assert!(md.contains("### [high] located blindspot"));
    assert!(md.contains("**File**: only/here.rs:1"));
    assert!(md.contains("**Source**: GPT only"));
    // The reference-less one is rendered too (no File line).
    assert!(md.contains("### [low] no location"));
}
