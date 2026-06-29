//! Mechanical rendering of a [`MergeResult`] into the debate Markdown layout.
//!
//! This is **presentation only** — deterministic string assembly over an
//! already-computed result. It does no adjudication, so it stays on the
//! mechanical side of the determinism boundary. The output mirrors, section
//! for section, what `parse.sh`'s `merge_and_format` produces — `## Consensus`,
//! `## Disputes`, `## Blindspots` — and adds one section the shell never had:
//! `## Unmatched`, holding the residue the model must adjudicate. The pilot
//! fish detects "debate format" by the first three headings, so the extra
//! section is additive and does not change detection.

use crate::model::{Finding, MergeResult};

/// Render a [`MergeResult`] to the `## Consensus` / `## Disputes` /
/// `## Blindspots` / `## Unmatched` Markdown, using `label_a` / `label_b` to
/// attribute suggestions (matching `parse.sh merge <a> <b> [label_a]
/// [label_b]`).
pub fn debate_markdown(result: &MergeResult, label_a: &str, label_b: &str) -> String {
    let mut out = String::new();

    // --- Consensus ---
    out.push_str("## Consensus\n\n");
    if result.consensus.is_empty() {
        out.push_str("No consensus findings -- the models did not flag the same locations.\n\n");
    } else {
        for pair in &result.consensus {
            let sev = match (&pair.a.severity, &pair.b.severity) {
                (a, b) if !b.is_empty() && a != b => format!("{a} / {b}"),
                (a, _) => a.clone(),
            };
            out.push_str(&format!("### [{}] {}\n", sev, pair.a.description));
            if let Some(file) = ref_string(&pair.a) {
                out.push_str(&format!("**File**: {file}\n"));
            }
            if let Some(s) = &pair.a.suggestion {
                out.push_str(&format!("**Suggestion**: {label_a}: {s}\n"));
            }
            if let Some(s) = &pair.b.suggestion {
                out.push_str(&format!("**Suggestion**: {label_b}: {s}\n"));
            }
            out.push('\n');
        }
    }
    out.push('\n');

    // --- Disputes (never produced by the mechanical merge) ---
    out.push_str("## Disputes\n\n");
    out.push_str(
        "No disputes from merge — disputes arise from vault overlay in the pilot fish step.\n\n",
    );
    out.push('\n');

    // --- Blindspots ---
    // Located findings only one model flagged (the other had nothing in that
    // file). These are final — the model pass does not re-adjudicate them.
    out.push_str("## Blindspots\n\n");
    if result.blindspot.is_empty() {
        out.push_str("No blindspots -- both models covered the same ground.\n\n");
    } else {
        for f in &result.blindspot {
            render_one_sided(&mut out, f);
        }
    }
    out.push('\n');

    // --- Unmatched ---
    // The residue the mechanical matcher could not confidently place: either no
    // usable location, or a same-file counterpart below the similarity
    // threshold. This is the ONLY bucket the downstream model adjudicates.
    out.push_str("## Unmatched\n\n");
    if result.unmatched.is_empty() {
        out.push_str("No unmatched findings -- everything was placed mechanically.\n\n");
    } else {
        for f in &result.unmatched {
            render_one_sided(&mut out, f);
        }
    }
    out.push('\n');

    out
}

/// Render a single one-sided finding (`### [sev] desc` + optional File + Source
/// + optional Suggestion), as used by the Blindspots and Unmatched sections.
fn render_one_sided(out: &mut String, f: &Finding) {
    out.push_str(&format!("### [{}] {}\n", f.severity, f.description));
    if let Some(file) = ref_string(f) {
        out.push_str(&format!("**File**: {file}\n"));
    }
    // Only emit the attribution line when there is a non-blank source; a bare
    // "**Source**:  only" with an empty label is meaningless and can arise via
    // the JSON `merge` ABI, whose findings need not carry a source.
    if let Some(source) = f.source.as_deref().filter(|s| !s.trim().is_empty()) {
        out.push_str(&format!("**Source**: {source} only\n"));
    }
    if let Some(s) = &f.suggestion {
        out.push_str(&format!("**Suggestion**: {s}\n"));
    }
    out.push('\n');
}

/// The `file:line` (or bare `file`) string for a finding, or `None` when it has
/// no usable reference.
fn ref_string(f: &Finding) -> Option<String> {
    f.reference
        .as_ref()
        .filter(|r| r.is_usable())
        .map(|r| r.key())
}
