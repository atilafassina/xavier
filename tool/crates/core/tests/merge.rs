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

// --- Phase 2: near-duplicate matching and the unmatched residue. ---

#[test]
fn exact_same_line_is_consensus_regardless_of_text() {
    // The location contract: two findings at the EXACT same canonical line are
    // a consensus even when the descriptions are unrelated. Exact match is the
    // high-confidence path and is never gated on similarity.
    let a = vec![finding("high", Some("src/x.rs:10"), "off by one", "GPT")];
    let b = vec![finding(
        "medium",
        Some("src/x.rs:10"),
        "completely different wording",
        "Gemini",
    )];

    let result = merge(&input(a, b));
    assert_eq!(result.consensus.len(), 1);
    assert!(result.unmatched.is_empty());
    assert!(result.blindspot.is_empty());
}

#[test]
fn paraphrase_at_different_line_same_file_is_near_dup_consensus() {
    // Same issue, different words, DIFFERENT lines of the same file. Exact match
    // misses it (different keys); the near-dup layer must collapse it.
    let a = vec![finding(
        "high",
        Some("src/api.rs:42"),
        "The response is missing the id field",
        "GPT",
    )];
    let b = vec![finding(
        "high",
        Some("src/api.rs:50"),
        "id field is absent from the response",
        "Gemini",
    )];

    let result = merge(&input(a, b));
    assert_eq!(
        result.consensus.len(),
        1,
        "paraphrase at a nearby line in the same file must collapse"
    );
    assert!(result.unmatched.is_empty());
    assert!(result.blindspot.is_empty());
}

#[test]
fn line_vs_range_collapses_via_exact_key() {
    // `:42` and `:42-60` canonicalize to the same start-line key, so even with
    // different wording they are an exact-match consensus.
    let a = vec![finding("high", Some("src/api.rs:42"), "issue here", "GPT")];
    let b = vec![finding(
        "high",
        Some("src/api.rs:42-60"),
        "different words",
        "Gemini",
    )];

    let result = merge(&input(a, b));
    assert_eq!(result.consensus.len(), 1);
}

#[test]
fn dissimilar_findings_same_file_different_line_go_to_unmatched() {
    // A same-file candidate exists but the texts are unrelated -> ambiguous, so
    // BOTH findings are surfaced as unmatched for the model to adjudicate. NOT a
    // consensus, NOT blindspots.
    let a = vec![finding(
        "high",
        Some("src/api.rs:10"),
        "off by one in the pagination offset",
        "GPT",
    )];
    let b = vec![finding(
        "low",
        Some("src/api.rs:80"),
        "the public function lacks rustdoc comments",
        "Gemini",
    )];

    let result = merge(&input(a, b));
    assert!(result.consensus.is_empty(), "must not over-merge");
    assert_eq!(result.unmatched.len(), 2);
    assert!(result.blindspot.is_empty());
}

#[test]
fn located_finding_with_no_other_side_candidate_is_blindspot() {
    // No finding from the other side in this file at all -> genuine blindspot.
    let a = vec![finding(
        "high",
        Some("only/here.rs:1"),
        "lone finding",
        "GPT",
    )];
    let b = vec![finding(
        "high",
        Some("elsewhere.rs:9"),
        "unrelated",
        "Gemini",
    )];

    let result = merge(&input(a, b));
    assert!(result.consensus.is_empty());
    assert!(result.unmatched.is_empty());
    assert_eq!(
        result.blindspot.len(),
        2,
        "different files -> two blindspots"
    );
}

#[test]
fn near_dup_picks_highest_similarity_candidate_deterministically() {
    // One `a` finding, two same-file `b` candidates; the more similar one wins,
    // and the loser (dissimilar, same file) falls to unmatched.
    let a = vec![finding(
        "high",
        Some("src/db.rs:5"),
        "null pointer dereference in the handler",
        "GPT",
    )];
    let b = vec![
        finding(
            "low",
            Some("src/db.rs:90"),
            "unrelated nit about spacing",
            "Gemini",
        ),
        finding(
            "high",
            Some("src/db.rs:7"),
            "handler dereferences a null pointer",
            "Gemini",
        ),
    ];

    let result = merge(&input(a, b));
    assert_eq!(result.consensus.len(), 1, "the similar b wins the match");
    assert_eq!(
        result.consensus[0].b.description,
        "handler dereferences a null pointer"
    );
    // Once `a` is consumed by the match, the dissimilar same-file `b` has no
    // remaining counterpart on side `a`, so it is a genuine one-sided
    // blindspot (there is nothing left to adjudicate it against).
    assert!(result.unmatched.is_empty());
    assert_eq!(result.blindspot.len(), 1);
    assert_eq!(
        result.blindspot[0].description,
        "unrelated nit about spacing"
    );
}
