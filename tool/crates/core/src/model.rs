//! Data models for the merge ABI.
//!
//! These types define the JSON contract the `xavier-tool merge` subcommand
//! reads on stdin ([`MergeInput`]) and writes on stdout ([`MergeResult`]).

use serde::{Deserialize, Serialize};

/// A canonical, normalized reference to a location in the codebase.
///
/// This is the key the mechanical matcher uses to decide whether two findings
/// describe "the same place". In Phase 1 the only thing that matters is the
/// normalized `file:line` string ([`CanonRef::key`]); `file` and `line` are
/// carried along for downstream consumers and future phases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonRef {
    /// The file path component, normalized (trimmed, surrounding backticks
    /// stripped). Empty when the source finding had no parseable file ref.
    #[serde(default)]
    pub file: String,

    /// The line number component, if one was present in the raw ref.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
}

impl CanonRef {
    /// Build a [`CanonRef`] from a raw `file:line`-ish reference string, applying
    /// the same normalization `parse.sh` applies before comparison: strip
    /// backticks, trim surrounding whitespace.
    ///
    /// The trailing `:<line>` segment is parsed into [`CanonRef::line`] when it is
    /// a pure integer; otherwise the whole string is treated as the file part.
    /// Comparison is always done on [`CanonRef::key`], so callers never need to
    /// reason about the split themselves.
    pub fn parse(raw: &str) -> Self {
        let cleaned = raw.replace('`', "");
        let cleaned = cleaned.trim();

        // Split on the LAST ':' so paths containing colons (rare, but possible)
        // keep their colon in the file part and only a trailing integer is
        // treated as a line number.
        if let Some((head, tail)) = cleaned.rsplit_once(':') {
            if let Ok(line) = tail.trim().parse::<u64>() {
                return CanonRef {
                    file: head.trim().to_string(),
                    line: Some(line),
                };
            }
        }

        CanonRef {
            file: cleaned.to_string(),
            line: None,
        }
    }

    /// The normalized comparison key, e.g. `src/main.rs:42` or just `README.md`
    /// when no line was present. Two findings are an exact match iff their keys
    /// are equal and non-empty.
    pub fn key(&self) -> String {
        match self.line {
            Some(line) => format!("{}:{}", self.file, line),
            None => self.file.clone(),
        }
    }

    /// Whether this reference carries enough information for the mechanical
    /// matcher to place the finding. An empty file part means the finding has
    /// no location and must be deferred to a semantic pass.
    pub fn is_usable(&self) -> bool {
        !self.file.trim().is_empty()
    }
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
/// - `consensus`: findings both sides flagged at the same exact location.
/// - `blindspot`: findings only one side flagged, but which DO have a usable
///   location.
/// - `dispute`: always empty from the mechanical merge. Disputes are produced
///   later, by the pilot fish overlaying vault knowledge — never here.
/// - `unmatched`: findings the mechanical matcher could not place because they
///   lack a usable location. These are the residue handed to the later
///   semantic / paraphrase pass. ALWAYS present (possibly empty) so the schema
///   is stable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeResult {
    pub consensus: Vec<MatchedPair>,
    pub blindspot: Vec<Finding>,
    pub dispute: Vec<Finding>,
    pub unmatched: Vec<Finding>,
}
