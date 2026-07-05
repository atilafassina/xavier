//! Golden tests for the full mechanical pipeline:
//! `parse_findings` -> `merge` -> `debate_markdown`.
//!
//! The fixtures are realistic captured-style `/x-review` output in the exact
//! Markdown format `parse.sh` extracts: repeated `### [severity] description`
//! blocks with `**File**:` and `**Suggestion**:` lines. No real run captures
//! exist, so these are hand-built to mirror what a `gpt-5.5` / `gemini-3.1`
//! reviewer pair actually emits through a Xavier persona.
//!
//! Each test asserts BOTH guarantees the Phase 2 bug fix requires:
//!
//! - **Collapse near-dups**: two models describing the *same* issue at the
//!   *same* location in *different words* (or with the line vs. a line range,
//!   or with/without the line) collapse into ONE `consensus`.
//! - **No over-merge**: genuinely distinct findings stay distinct — they do NOT
//!   silently become consensus. Same-file-but-different issues land in
//!   `unmatched` for the model to adjudicate; different-file issues stay
//!   `blindspot`.

use xavier_core::{debate_markdown, merge, parse_findings, MergeInput};

/// Build a `MergeInput` from two raw model texts the way `merge-text` does.
fn merge_texts(text_a: &str, text_b: &str) -> MergeInput {
    MergeInput {
        a: parse_findings(text_a, "GPT"),
        b: parse_findings(text_b, "Gemini"),
        label_a: "GPT".into(),
        label_b: "Gemini".into(),
    }
}

#[test]
fn paraphrased_same_location_collapses_to_one_consensus() {
    // Both models flag the SAME missing-field issue at the SAME line, in
    // different words. Under the old exact-text-agnostic / shell merge this was
    // already a consensus by exact ref; this test pins that paraphrase wording
    // never accidentally splits it.
    let gpt = "\
### [high] The serializer omits the `id` field from the response body
**File**: `src/api/response.rs:42`
**Suggestion**: Add `id` to the struct's serialized fields.
";
    let gemini = "\
### [medium] `id` is absent from the serialized response
**File**: `src/api/response.rs:42`
**Suggestion**: Include the id when building the JSON.
";

    let result = merge(&merge_texts(gpt, gemini));

    assert_eq!(
        result.consensus.len(),
        1,
        "same issue at same line must be ONE consensus, not two blindspots"
    );
    assert!(result.blindspot.is_empty());
    assert!(result.unmatched.is_empty());
}

#[test]
fn near_dup_at_different_line_and_missing_line_still_collapses() {
    // The bug the exact-match shell merge could NOT handle: the same issue, but
    // GPT cites a single line and Gemini cites a line RANGE that starts one line
    // earlier (or omits the line). Range canonicalization + textual similarity
    // must still collapse these into one consensus.
    let gpt = "\
### [high] Unvalidated user input flows into the SQL query string
**File**: `src/db/users.rs:88`
**Suggestion**: Use a parameterized query.
";
    let gemini = "\
### [high] User input is concatenated into the SQL query without validation
**File**: `src/db/users.rs:85-92`
**Suggestion**: Bind parameters instead of string concatenation.
";

    let result = merge(&merge_texts(gpt, gemini));

    assert_eq!(
        result.consensus.len(),
        1,
        "paraphrase at a nearby line/range, same file, must collapse to consensus"
    );
    assert!(
        result.unmatched.is_empty(),
        "a clear near-dup is mechanical, not residue"
    );
    assert!(result.blindspot.is_empty());
}

#[test]
fn distinct_issues_same_file_do_not_over_merge() {
    // Two DIFFERENT issues in the SAME file. They must NOT collapse into a
    // consensus. Because a same-file counterpart exists but the texts are
    // dissimilar, the matcher cannot decide mechanically -> both go to
    // `unmatched` for the model to adjudicate. Crucially: zero consensus.
    let gpt = "\
### [high] Off-by-one in the pagination offset calculation
**File**: `src/api/list.rs:30`
**Suggestion**: Subtract one from the page index.
";
    let gemini = "\
### [low] Missing rustdoc on the public `list_items` function
**File**: `src/api/list.rs:12`
**Suggestion**: Document the parameters and return type.
";

    let result = merge(&merge_texts(gpt, gemini));

    assert!(
        result.consensus.is_empty(),
        "distinct issues must NOT be merged into consensus"
    );
    assert_eq!(
        result.unmatched.len(),
        2,
        "same-file-but-dissimilar findings are ambiguous -> unmatched for adjudication"
    );
    assert!(result.blindspot.is_empty());
}

#[test]
fn distinct_issues_different_files_are_blindspots() {
    // Genuinely one-sided findings in different files: each model saw something
    // the other did not. These are blindspots, not consensus and not unmatched.
    let gpt = "\
### [medium] Error from `parse_config` is swallowed
**File**: `src/config.rs:55`
**Suggestion**: Propagate the error with `?`.
";
    let gemini = "\
### [high] Possible panic on empty slice index
**File**: `src/render.rs:18`
**Suggestion**: Guard against an empty slice before indexing.
";

    let result = merge(&merge_texts(gpt, gemini));

    assert!(result.consensus.is_empty());
    assert!(result.unmatched.is_empty());
    assert_eq!(
        result.blindspot.len(),
        2,
        "one-sided findings in different files are blindspots"
    );
    // Stable ordering: side A's blindspot before side B's.
    assert_eq!(result.blindspot[0].source.as_deref(), Some("GPT"));
    assert_eq!(result.blindspot[1].source.as_deref(), Some("Gemini"));
}

#[test]
fn realistic_mixed_review_buckets_everything_correctly() {
    // A fuller, realistic pair of reviews exercising all four buckets at once,
    // including a multi-line description and a reference-less finding (which the
    // shell scraper would have fumbled).
    //
    // The two net/client.rs findings (backoff vs. timeout) are DIFFERENT issues
    // at DIFFERENT lines of the SAME file. This is the near-duplicate layer's
    // no-over-merge surface: a same-file candidate exists, but the texts are
    // dissimilar, so neither becomes consensus — both go to `unmatched` for the
    // model to adjudicate. (Two findings at the *exact same* line would be an
    // exact-match consensus by the location contract; that is covered
    // elsewhere.)
    let gpt = "\
Here is my review of the diff.

### [high] SQL injection via string-built query
**File**: `src/db/users.rs:88`
**Suggestion**: Use bound parameters.

### [medium] The retry loop never backs off, so a flapping
downstream service is hammered at full rate, which can
amplify an outage.
**File**: `src/net/client.rs:120`
**Suggestion**: Add exponential backoff with jitter.

### [low] Naming: `d` is an unclear variable name
**File**: `src/util/time.rs:7`

### [low] Consider adding a CHANGELOG entry for this change
";
    let gemini = "\
My findings:

### [high] User-controlled input is interpolated into the SQL string without sanitization
**File**: `src/db/users.rs:88`
**Suggestion**: Parameterize the query.

### [medium] No timeout on the outbound HTTP request
**File**: `src/net/client.rs:145`
**Suggestion**: Set a request timeout.
";

    let result = merge(&merge_texts(gpt, gemini));

    // 1) The SQL-injection finding is a paraphrase at the SAME line -> consensus.
    assert_eq!(
        result.consensus.len(),
        1,
        "the SQL-injection paraphrase pair must be the single consensus"
    );
    let c = &result.consensus[0];
    assert!(c.a.description.to_lowercase().contains("sql"));
    assert!(c.b.description.to_lowercase().contains("sql"));

    // 2) The two client.rs findings are DIFFERENT issues at DIFFERENT lines of
    //    the same file -> a same-file candidate exists but is below threshold
    //    -> both `unmatched` (NOT a second consensus). The reference-less GPT
    //    finding (no File) is also `unmatched`.
    let unmatched_descs: Vec<String> = result
        .unmatched
        .iter()
        .map(|f| f.description.to_lowercase())
        .collect();
    assert!(
        unmatched_descs
            .iter()
            .any(|d| d.contains("backs off") || d.contains("back off")),
        "the backoff finding should be unmatched, got {unmatched_descs:?}"
    );
    assert!(
        unmatched_descs.iter().any(|d| d.contains("timeout")),
        "the timeout finding should be unmatched, got {unmatched_descs:?}"
    );
    assert!(
        unmatched_descs.iter().any(|d| d.contains("changelog")),
        "the reference-less finding should be unmatched, got {unmatched_descs:?}"
    );
    assert_eq!(
        result.unmatched.len(),
        3,
        "exactly the backoff, timeout, and changelog findings are unmatched"
    );

    // 3) The naming nit in time.rs is one-sided in its own file -> blindspot.
    assert_eq!(
        result.blindspot.len(),
        1,
        "the time.rs nit is a lone blindspot"
    );
    assert!(result.blindspot[0].description.contains('d'));

    // 4) Multi-line description was folded into one line (shell scraper bug).
    let backoff = result
        .unmatched
        .iter()
        .find(|f| f.description.to_lowercase().contains("retry loop"))
        .expect("multi-line retry finding present");
    assert!(
        backoff.description.contains("amplify an outage"),
        "the wrapped continuation lines must be folded into the description"
    );

    // 5) The rendered debate markdown keeps the three pilot-fish headings AND
    //    the new Unmatched section, and puts the consensus under Consensus.
    let md = debate_markdown(&result, "GPT", "Gemini");
    assert!(md.contains("## Consensus"));
    assert!(md.contains("## Disputes"));
    assert!(md.contains("## Blindspots"));
    assert!(md.contains("## Unmatched"));
    // The SQL consensus appears in the Consensus section, before Disputes.
    let consensus_idx = md.find("## Consensus").unwrap();
    let disputes_idx = md.find("## Disputes").unwrap();
    let sql_idx = md.to_lowercase().find("sql").unwrap();
    assert!(
        consensus_idx < sql_idx && sql_idx < disputes_idx,
        "the SQL consensus must render inside the Consensus section"
    );
}

#[test]
fn deterministic_output_for_equal_input() {
    // Byte-stable output matters: the result feeds a downstream model pass.
    let gpt = "\
### [high] Missing null check before deref
**File**: `src/a.rs:10`
### [low] Typo in comment
**File**: `src/b.rs:3`
";
    let gemini = "\
### [high] Pointer dereferenced without a null guard
**File**: `src/a.rs:10`
### [medium] Unused import
**File**: `src/c.rs:1`
";

    let md1 = {
        let r = merge(&merge_texts(gpt, gemini));
        debate_markdown(&r, "GPT", "Gemini")
    };
    let md2 = {
        let r = merge(&merge_texts(gpt, gemini));
        debate_markdown(&r, "GPT", "Gemini")
    };
    assert_eq!(md1, md2, "equal input must yield byte-identical output");

    // And the null-deref paraphrase pair collapsed to consensus.
    let r = merge(&merge_texts(gpt, gemini));
    assert_eq!(r.consensus.len(), 1);
}

#[test]
fn conforming_pipeline_renders_classic_markdown_byte_for_byte() {
    // Story 8 — happy-path golden at pipeline/render scope. Two CONFORMING
    // persona-format reviews (real `### [severity]` blocks with `**File**:`
    // lines) that form ONE consensus (same file:line, paraphrased) plus one
    // one-sided blindspot. Because the input carries conforming headings, the
    // prose-recovery stage is gated OFF (see `findings.rs`), so the full
    // parse -> merge -> render pipeline must produce EXACTLY the classic
    // rendering, byte for byte. This fences at OUTPUT scope the guarantee the
    // Phase-4b unit test fences at parse scope: the prose stage never perturbs a
    // well-formed review. (The gating fence in `findings.rs` guarantees the
    // stage does not run on this input; here we pin what that means for the
    // rendered artifact the downstream model actually sees.)
    let gpt = "\
### [high] Missing bounds check before slice index
**File**: `src/foo.rs:10`
**Suggestion**: Guard the index against the slice length.
";
    let gemini = "\
### [high] Slice is indexed without checking its length first
**File**: `src/foo.rs:10`
**Suggestion**: Check the length before indexing.

### [low] Public function lacks a doc comment
**File**: `src/bar.rs:3`
**Suggestion**: Add a rustdoc summary line.
";

    // The conforming path (NOT prose recovery) produced these findings: every
    // one carries a real severity, so had the prose stage wrongly run it would
    // surface as a structural diff in the byte-for-byte assertion below.
    let a = parse_findings(gpt, "GPT");
    let b = parse_findings(gemini, "Gemini");
    assert_eq!(a.len(), 1, "gpt has exactly one conforming finding");
    assert_eq!(b.len(), 2, "gemini has exactly two conforming findings");
    assert!(
        a.iter().chain(b.iter()).all(|f| f.severity != "unknown"),
        "conforming headings must yield real severities, never the prose fallback"
    );

    let result = merge(&MergeInput {
        a,
        b,
        label_a: "GPT".into(),
        label_b: "Gemini".into(),
    });
    let md = debate_markdown(&result, "GPT", "Gemini");

    // The classic debate rendering, exactly. Note `**File**: src/foo.rs:10`
    // carries the line from the REAL `**File**:` field (legitimate), which is
    // distinct from the never-fabricate case fenced in `prose_recovery.rs`.
    let expected = "## Consensus\n\
\n\
### [high] Missing bounds check before slice index\n\
**File**: src/foo.rs:10\n\
**Suggestion**: GPT: Guard the index against the slice length.\n\
**Suggestion**: Gemini: Check the length before indexing.\n\
\n\
\n\
## Disputes\n\
\n\
No disputes from merge — disputes arise from vault overlay in the pilot fish step.\n\
\n\
\n\
## Blindspots\n\
\n\
### [low] Public function lacks a doc comment\n\
**File**: src/bar.rs:3\n\
**Source**: Gemini only\n\
**Suggestion**: Add a rustdoc summary line.\n\
\n\
\n\
## Unmatched\n\
\n\
No unmatched findings -- everything was placed mechanically.\n\
\n\
\n";

    assert_eq!(
        md, expected,
        "a conforming review must render to the classic debate markdown byte-for-byte \
         (the prose stage is gated off and must not perturb the happy path)"
    );
}
