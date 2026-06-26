//! Unit tests for the trivial exact-match merge.

use xavier_core::{merge, CanonRef, Finding, MergeInput};

fn finding(severity: &str, file_ref: Option<&str>, desc: &str, source: &str) -> Finding {
    Finding {
        severity: severity.to_string(),
        reference: file_ref.map(CanonRef::parse),
        description: desc.to_string(),
        suggestion: None,
        source: Some(source.to_string()),
    }
}

fn input(a: Vec<Finding>, b: Vec<Finding>) -> MergeInput {
    MergeInput {
        a,
        b,
        label_a: "GPT".to_string(),
        label_b: "Gemini".to_string(),
    }
}

#[test]
fn exact_match_becomes_consensus() {
    let a = vec![finding("high", Some("src/main.rs:42"), "off by one", "GPT")];
    let b = vec![finding(
        "medium",
        Some("src/main.rs:42"),
        "loop bound wrong",
        "Gemini",
    )];

    let result = merge(&input(a, b));

    assert_eq!(
        result.consensus.len(),
        1,
        "matching refs should be consensus"
    );
    assert!(result.blindspot.is_empty());
    assert!(result.unmatched.is_empty());
    assert!(result.dispute.is_empty(), "merge never emits disputes");

    let pair = &result.consensus[0];
    assert_eq!(pair.a.source.as_deref(), Some("GPT"));
    assert_eq!(pair.b.source.as_deref(), Some("Gemini"));
}

#[test]
fn unmatched_refs_become_blindspots() {
    // Different locations -> two blindspots, mirroring parse.sh's documented
    // exact-match trade-off.
    let a = vec![finding("high", Some("src/a.rs:1"), "issue a", "GPT")];
    let b = vec![finding("high", Some("src/b.rs:2"), "issue b", "Gemini")];

    let result = merge(&input(a, b));

    assert!(result.consensus.is_empty());
    assert_eq!(result.blindspot.len(), 2);
    assert!(result.unmatched.is_empty());

    // Side A's blindspot is listed before side B's (stable ordering).
    assert_eq!(result.blindspot[0].source.as_deref(), Some("GPT"));
    assert_eq!(result.blindspot[1].source.as_deref(), Some("Gemini"));
}

#[test]
fn reference_less_findings_are_unmatched_not_blindspots() {
    // The determinism boundary: a finding with no usable location cannot be
    // mechanically placed, so it is surfaced as `unmatched`.
    let a = vec![
        finding("low", None, "vague comment, no file", "GPT"),
        finding("high", Some(""), "empty ref", "GPT"),
    ];
    let b = vec![];

    let result = merge(&input(a, b));

    assert!(result.consensus.is_empty());
    assert!(result.blindspot.is_empty());
    assert_eq!(result.unmatched.len(), 2);
}

#[test]
fn each_b_finding_is_consumed_at_most_once() {
    // Two A findings at the same location, one B finding at that location:
    // the first A claims B (consensus), the second A becomes a blindspot.
    let a = vec![
        finding("high", Some("f.rs:10"), "first", "GPT"),
        finding("high", Some("f.rs:10"), "second", "GPT"),
    ];
    let b = vec![finding("high", Some("f.rs:10"), "only b", "Gemini")];

    let result = merge(&input(a, b));

    assert_eq!(result.consensus.len(), 1);
    assert_eq!(result.blindspot.len(), 1);
    assert_eq!(result.blindspot[0].description, "second");
    assert!(result.unmatched.is_empty());
}

#[test]
fn canon_ref_normalizes_backticks_and_whitespace() {
    // parse.sh strips backticks and trims; the binary must match so refs from
    // either path compare equal.
    let a = vec![finding("high", Some("  `src/main.rs:7` "), "x", "GPT")];
    let b = vec![finding("high", Some("src/main.rs:7"), "y", "Gemini")];

    let result = merge(&input(a, b));

    assert_eq!(result.consensus.len(), 1);
}

#[test]
fn empty_input_produces_empty_buckets() {
    let result = merge(&input(vec![], vec![]));
    assert!(result.consensus.is_empty());
    assert!(result.blindspot.is_empty());
    assert!(result.dispute.is_empty());
    assert!(result.unmatched.is_empty());
}

#[test]
fn canon_ref_key_handles_path_with_colon() {
    // Only a trailing integer is treated as a line number.
    let r = CanonRef::parse("C:/weird/path.rs:99");
    assert_eq!(r.line, Some(99));
    assert_eq!(r.key(), "C:/weird/path.rs:99");

    let no_line = CanonRef::parse("plain/file.rs");
    assert_eq!(no_line.line, None);
    assert_eq!(no_line.key(), "plain/file.rs");
}
