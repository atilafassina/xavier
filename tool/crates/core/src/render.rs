//! Mechanical rendering of a [`MergeResult`] into the debate Markdown layout.
//!
//! This is **presentation only** — deterministic string assembly over an
//! already-computed result. It does no adjudication, so it stays on the
//! mechanical side of the determinism boundary. The output mirrors, section
//! for section, what `parse.sh`'s `merge_and_format` produces, so the binary
//! path and the shell fallback yield equivalent Markdown.

use crate::model::{Finding, MergeResult};

/// Render a [`MergeResult`] to the `## Consensus` / `## Disputes` /
/// `## Blindspots` Markdown, using `label_a` / `label_b` to attribute
/// suggestions (matching `parse.sh merge <a> <b> [label_a] [label_b]`).
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
    // The binary keeps reference-less residue in `unmatched`; parse.sh folded
    // those into Blindspots, so render both buckets here for output parity.
    out.push_str("## Blindspots\n\n");
    let blindspots: Vec<&Finding> = result
        .blindspot
        .iter()
        .chain(result.unmatched.iter())
        .collect();
    if blindspots.is_empty() {
        out.push_str("No blindspots -- both models covered the same ground.\n\n");
    } else {
        for f in blindspots {
            out.push_str(&format!("### [{}] {}\n", f.severity, f.description));
            if let Some(file) = ref_string(f) {
                out.push_str(&format!("**File**: {file}\n"));
            }
            let source = f.source.as_deref().unwrap_or("");
            out.push_str(&format!("**Source**: {source} only\n"));
            if let Some(s) = &f.suggestion {
                out.push_str(&format!("**Suggestion**: {s}\n"));
            }
            out.push('\n');
        }
    }
    out.push('\n');

    out
}

/// The `file:line` (or bare `file`) string for a finding, or `None` when it has
/// no usable reference.
fn ref_string(f: &Finding) -> Option<String> {
    f.reference
        .as_ref()
        .filter(|r| r.is_usable())
        .map(|r| r.key())
}
