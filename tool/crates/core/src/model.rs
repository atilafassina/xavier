//! Data models for the merge ABI.
//!
//! These types define the JSON contract the `xavier-tool merge` subcommand
//! reads on stdin ([`MergeInput`]) and writes on stdout ([`MergeResult`]).

use serde::{Deserialize, Serialize};

/// A canonical, normalized reference to a location in the codebase.
///
/// This is the key the mechanical matcher uses to decide whether two findings
/// describe "the same place". The only thing that matters for matching is the
/// normalized `file:line` string ([`CanonRef::key`]); `file` and `line` are
/// carried along for downstream consumers.
///
/// The parsing/normalization logic (single lines, ranges, missing lines) lives
/// in [`crate::refs`]; this struct is the serialized shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonRef {
    /// The file path component, normalized (trimmed, surrounding backticks
    /// stripped). Empty when the source finding had no parseable file ref.
    #[serde(default)]
    pub file: String,

    /// The line number component, if one was present in the raw ref. For a line
    /// range (`40-52`) this is the **start** line — see [`crate::refs`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
}

/// A single review finding, as parsed out of one model's output by the shell
/// extraction step. Mirrors the columns `parse.sh` carries in its TSV:
/// severity, file ref, description, suggestion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// Severity label, lower-cased by the upstream parser (e.g. `high`).
    #[serde(default)]
    pub severity: String,

    /// The raw/normalized location reference. May be absent or empty when the
    /// model did not attach a file to the finding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<CanonRef>,

    /// One-line description of the finding.
    #[serde(default)]
    pub description: String,

    /// Optional suggested fix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,

    /// Which model/source produced this finding (e.g. `GPT`). Used purely for
    /// attribution in the merged output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl Finding {
    /// The comparison key for this finding, or `None` when it has no usable
    /// reference.
    pub fn match_key(&self) -> Option<String> {
        self.reference
            .as_ref()
            .filter(|r| r.is_usable())
            .map(CanonRef::key)
    }
}

/// The stdin payload for `xavier-tool merge`: two sides of findings plus
/// optional labels identifying each side in the output (mirrors `parse.sh
/// merge <a> <b> [label_a] [label_b]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeInput {
    /// Findings from the first model.
    #[serde(default)]
    pub a: Vec<Finding>,

    /// Findings from the second model.
    #[serde(default)]
    pub b: Vec<Finding>,

    /// Label for side `a` (default `Model A`).
    #[serde(default = "default_label_a")]
    pub label_a: String,

    /// Label for side `b` (default `Model B`).
    #[serde(default = "default_label_b")]
    pub label_b: String,
}

fn default_label_a() -> String {
    "Model A".to_string()
}

fn default_label_b() -> String {
    "Model B".to_string()
}

/// The stdin payload for `xavier-tool merge-text`: the **raw assistant text**
/// from each model (already extracted from stream-json) plus optional labels.
///
/// This is the binary's finding-ingestion entry point: the tool parses each
/// side's Markdown into [`Finding`]s itself (via [`crate::findings`]), so the
/// shell front door no longer has to scrape findings with `awk`. It exists
/// alongside [`MergeInput`] (pre-parsed findings) so the original JSON ABI
/// stays pinned while the text path is added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeTextInput {
    /// Raw assistant text from the first model.
    #[serde(default)]
    pub text_a: String,

    /// Raw assistant text from the second model.
    #[serde(default)]
    pub text_b: String,

    /// Label for side `a` (default `Model A`). Also used as the `source`
    /// attribution on every finding parsed from `text_a`.
    #[serde(default = "default_label_a")]
    pub label_a: String,

    /// Label for side `b` (default `Model B`). Also used as the `source`
    /// attribution on every finding parsed from `text_b`.
    #[serde(default = "default_label_b")]
    pub label_b: String,
}

/// A consensus match: one finding from each side that share an exact
/// normalized reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchedPair {
    /// The finding from side `a`.
    pub a: Finding,
    /// The finding from side `b`.
    pub b: Finding,
}

/// The stdout payload from `xavier-tool merge`.
///
/// The four buckets are the durable contract across every phase:
/// - `consensus`: findings both sides flagged at the same location — either an
///   exact canonical `file:line` match, or a textual near-duplicate at the same
///   file (paraphrases of the same issue).
/// - `blindspot`: a located finding only one side flagged, where the other side
///   had no finding in that file at all.
/// - `dispute`: always empty from the mechanical merge. Disputes are produced
///   later, by the pilot fish overlaying vault knowledge — never here.
/// - `unmatched`: findings the mechanical matcher could not confidently place —
///   either because they lack a usable location, or because a same-file
///   counterpart existed but fell below the similarity threshold (same place,
///   different words: same issue or two distinct ones?). These are the residue
///   handed to the model for adjudication. ALWAYS present (possibly empty) so
///   the schema is stable. The other three buckets are final and are NOT
///   re-adjudicated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeResult {
    pub consensus: Vec<MatchedPair>,
    pub blindspot: Vec<Finding>,
    pub dispute: Vec<Finding>,
    pub unmatched: Vec<Finding>,
}
