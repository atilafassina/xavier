//! Parse a model's review Markdown into typed [`Finding`] records.
//!
//! This replaces the `awk` markdown scraping that lived in `parse.sh`
//! (`parse_findings`) and `merge.sh` (`extract_findings_json`). A reviewer
//! persona emits findings as repeated blocks of the form:
//!
//! ```text
//! ### [high] Off-by-one in the loop bound
//! **File**: `src/main.rs:42`
//! **Suggestion**: Use `<=` instead of `<`.
//! ```
//!
//! The shell scraper had three concrete bugs this parser fixes:
//!
//! 1. **Multi-line descriptions** — `awk` kept only the text on the `### […]`
//!    heading line, dropping any wrapped continuation lines. This parser folds
//!    every line up to the next field/heading into the description.
//! 2. **`\uXXXX` escapes** — `parse.sh extract` decodes `\"`, `\n`, `\t`,
//!    `\\`, `\/` but passes `\uXXXX` through as a literal backslash-u-hex
//!    sequence. This parser decodes those (including UTF-16 surrogate pairs) so
//!    a finding that mentions e.g. a smart quote compares correctly.
//! 3. **Non-strict Markdown** — the scraper required an exact `### [sev] desc`
//!    and `**File**:`/`**Suggestion**:` spelling. This parser tolerates extra
//!    `#`/spaces, a missing `] ` separator, `**File:**` as well as `**File**:`,
//!    leading list bullets, and case-insensitive field labels.
//!
//! Output ordering matches input order, which keeps the downstream merge
//! deterministic.

use crate::model::{CanonRef, Finding};

/// Parse one model's assistant text (already extracted from stream-json) into a
/// list of [`Finding`]s, attributing each to `source`.
///
/// Robust to the non-strict Markdown reviewers actually produce; lines that do
/// not belong to any finding block are ignored. A block with no usable
/// description is dropped (mirrors the shell scraper, which only emitted a
/// finding once it had a non-empty description).
pub fn parse_findings(text: &str, source: &str) -> Vec<Finding> {
    let decoded = decode_unicode_escapes(text);

    // GATING INVARIANT: the prose-recovery stage runs ONLY when the response
    // contains NO conforming `### [severity]` block. If even one conforming
    // heading exists, the input is treated as persona-formatted and flows
    // through the existing path untouched (byte-identical output). This keeps
    // the recovery heuristics from ever perturbing already-well-formed reviews.
    let has_conforming = decoded
        .lines()
        .any(|l| is_conforming_heading(l.trim_start()));
    if !has_conforming {
        let recovered = recover_from_prose(&decoded, source);
        // Only take the recovered findings when the stage actually salvaged
        // something. On pure prose (no location, no severity) it yields nothing,
        // and we fall through to the existing path so no previously-parsed
        // finding is ever lost.
        if !recovered.is_empty() {
            return recovered;
        }
    }

    let mut out: Vec<Finding> = Vec::new();
    let mut cur: Option<Builder> = None;

    for raw_line in decoded.lines() {
        let line = raw_line.trim_end();
        let trimmed = line.trim_start();

        if let Some((severity, desc)) = parse_heading(trimmed) {
            // New finding heading flushes the previous block.
            if let Some(b) = cur.take() {
                b.push_into(&mut out, source);
            }
            cur = Some(Builder::new(severity, desc));
            continue;
        }

        // Field lines only matter inside an open finding block.
        let Some(b) = cur.as_mut() else { continue };

        if let Some(val) = parse_field(trimmed, "file") {
            b.set_reference(val);
        } else if let Some(val) = parse_field(trimmed, "suggestion") {
            b.start_suggestion(val);
        } else if parse_field(trimmed, "source").is_some()
            || parse_field(trimmed, "severity").is_some()
        {
            // Recognized-but-ignored fields (source is supplied by the caller;
            // severity already came from the heading). Swallow so they don't
            // bleed into a multi-line description or suggestion.
            b.end_suggestion();
        } else {
            // A plain continuation line: extend whichever field is "open".
            b.continuation(line);
        }
    }

    if let Some(b) = cur.take() {
        b.push_into(&mut out, source);
    }

    out
}

// ===========================================================================
// Prose-recovery stage (Phase 4a).
//
// Runs ONLY when the response has no conforming `### [severity]` block (see the
// gate in `parse_findings`). It recovers findings a model emitted as prose:
// segmenting the body on bold "lead" lines, deriving a per-segment severity
// hint, and locating each segment via the existing Layer-B salvage with a
// hoisted shared location as fallback. Conforming input never reaches here, so
// well-formed reviews stay byte-identical.
// ===========================================================================

/// True iff `line` is a *conforming* persona finding heading — `##`+ followed by
/// a `[severity]` bracket. This is the gate: any such line means the response is
/// persona-formatted and must flow through the existing parser untouched. A
/// bracket-less heading (`## Findings`, `### File:`) is NOT conforming.
fn is_conforming_heading(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("##") else {
        return false;
    };
    let rest = rest.trim_start_matches('#').trim_start();
    // Needs an opening `[…]` bracket whose content is a recognized SEVERITY
    // word. A bracket holding a category or anything else (`## [Correctness]
    // Findings`) is NOT conforming — otherwise the gate would wrongly route a
    // prose review with a category heading through the existing parser and skip
    // prose recovery.
    let Some(after) = rest.strip_prefix('[') else {
        return false;
    };
    let Some(close) = after.find(']') else {
        return false;
    };
    let inner = after[..close].trim();
    // An EMPTY bracket (`### []`) is a degenerate persona heading, not a prose
    // review — keep it on the existing path (it yields nothing on its own), so
    // the gate only diverts headings whose bracket holds a NON-severity word
    // (`## [Correctness] Findings`) into prose recovery.
    inner.is_empty() || is_severity_label(inner)
}

/// True iff `line` is a TOP-LEVEL list bullet (`- `/`* `/`+ `, no leading
/// indent) whose content carries at least one path-like backtick span. Used as
/// a fallback finding boundary for bullet-list prose reviews that have no bold
/// leads. The path-span requirement keeps ordinary narrative bullets (a list of
/// symptoms under one finding) from each becoming a spurious finding.
fn is_bullet_with_path(line: &str) -> bool {
    // Must be a bullet with no leading whitespace (a nested/indented bullet is a
    // sub-point of the current finding, not a new one).
    let after = match line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")).or_else(|| line.strip_prefix("+ ")) {
        Some(a) => a,
        None => return false,
    };
    code_spans(after).iter().any(|s| path_like_file(s).is_some())
}

/// True iff `tok` names a severity level used in a finding heading bracket.
/// Accepts the security/performance vocabulary (critical|high|medium|low), the
/// correctness vocabulary (major|minor), and common synonyms (severe|moderate).
/// Rejects category words (correctness, security, performance) and prose.
fn is_severity_label(tok: &str) -> bool {
    matches!(
        tok.to_lowercase().as_str(),
        "critical" | "high" | "medium" | "low" | "major" | "minor" | "severe" | "moderate"
    )
}

/// Recover findings from a non-conforming (prose) response. Returns an empty
/// vec when nothing finding-like can be salvaged, in which case `parse_findings`
/// falls back to its existing path so no already-parseable finding is lost.
fn recover_from_prose(text: &str, source: &str) -> Vec<Finding> {
    let lines: Vec<&str> = text.lines().collect();

    // --- 1. Segment on bold-lead openers, ignoring fenced code regions. ---
    let mut openers: Vec<(usize, Option<String>)> = Vec::new();
    let mut in_fence = false;
    for (i, raw) in lines.iter().enumerate() {
        let t = raw.trim_start();
        if t.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(hint) = segment_opener(t) {
            openers.push((i, hint));
        }
    }

    // Fallback segmentation: a bullet-list review with NO bold-lead openers —
    // each top-level `- `/`* `/`+ ` bullet that carries its own path-like span
    // is its own finding (the `corr_gpt` shape). Only runs when bold openers are
    // absent, so a review that DOES use bold leads keeps its sub-bullets folded
    // into the current finding (the `sec_gpt` shape) rather than shattering.
    if openers.is_empty() {
        let mut in_fence = false;
        for (i, raw) in lines.iter().enumerate() {
            let t = raw.trim_start();
            if t.starts_with("```") {
                in_fence = !in_fence;
                continue;
            }
            if in_fence {
                continue;
            }
            // Pass the RAW (untrimmed) line: is_bullet_with_path rejects a
            // leading-indent bullet as a sub-point, and that guard only works if
            // it sees the original indentation (not the fence-trimmed `t`).
            if is_bullet_with_path(raw) {
                openers.push((i, None));
            }
        }
    }

    // Segment ranges: each opener to the next; text before the first opener is
    // preamble (a location source, never a finding). With no opener at all, the
    // whole body is a single implicit segment (covers a review that is entirely
    // prose but still names a file + verdict, e.g. `sec_gemini`).
    let preamble_end = openers.first().map_or(0, |(i, _)| *i);
    let segments: Vec<(usize, usize, Option<String>)> = if openers.is_empty() {
        vec![(0, lines.len(), None)]
    } else {
        (0..openers.len())
            .map(|k| {
                let (start, ref hint) = openers[k];
                let end = openers.get(k + 1).map_or(lines.len(), |(j, _)| *j);
                (start, end, hint.clone())
            })
            .collect()
    };

    // --- 2. Hoisted shared location + severity (fallbacks for segments that
    // lack their own). A response-level `Verdict:` / severity word applies to
    // every segment that didn't state its own — this is what lets bullet-split
    // findings under a single shared verdict inherit a real severity instead of
    // each landing at `unknown`. ---
    let hoist = compute_hoist(&lines, preamble_end);
    let hoisted_severity = {
        let all_lines: Vec<String> = lines.iter().map(|l| l.trim_end().to_string()).collect();
        scan_severity(&all_lines)
    };

    // --- 3. Build one finding per qualifying segment. ---
    let mut out: Vec<Finding> = Vec::new();
    for (start, end, opener_hint) in segments {
        let seg_lines: Vec<String> = lines[start..end]
            .iter()
            .filter(|l| !l.trim_start().starts_with("```")) // drop bare fence markers
            .map(|l| l.trim_end().to_string())
            .collect();

        let description = join_lines(&seg_lines);
        if description.is_empty() {
            continue;
        }

        // Severity precedence: the opener's own hint, else a severity word /
        // `Verdict:` line inside the segment, else the response-level hoisted
        // severity (a shared verdict), else `unknown`.
        let severity = opener_hint
            .or_else(|| scan_severity(&seg_lines))
            .or_else(|| hoisted_severity.clone())
            .unwrap_or_else(|| "unknown".to_string());

        // Location precedence, most specific first:
        //   1. the segment's OWN `**File**:` field (a drifted-but-explicit path),
        //   2. a single path-like backtick span inside the segment (Layer-B),
        //   3. the hoisted shared location.
        // A segment that names its own file must never inherit a sibling's
        // hoisted path. Never a fabricated line — all sources are file-or-
        // file:line refs only.
        let reference = segment_file_field(&seg_lines)
            .or_else(|| salvage_reference(&[seg_lines.as_slice()]))
            .or_else(|| hoist.clone());

        // Emit only a segment that is actually finding-like: it must carry a
        // real severity OR a usable location. This is what keeps pure prose
        // ("just prose, no findings here") from manufacturing a finding.
        let real_severity = severity != "unknown";
        let has_location = reference.as_ref().is_some_and(CanonRef::is_usable);
        if !(real_severity || has_location) {
            continue;
        }

        out.push(Finding {
            severity,
            reference,
            description,
            suggestion: None,
            source: Some(source.to_string()),
        });
    }

    out
}

/// If `line` is a bold lead (`**…**` at line start) that OPENS a new finding,
/// return its severity hint (`Some(level)` from a severity/verdict lead, `None`
/// from a number-only lead unless a severity word is embedded). Returns `None`
/// (the outer option) when the line does not open a finding.
///
/// Opens iff the bold lead is a **number** (`1.`, `2)`), a **severity word**
/// (`[high]`, `Critical`, `Severe`, …), or a **verdict phrase** (`Request
/// changes`, `Approve`, `Rethink`) AND is NOT a bold **field label** (Defect,
/// Failure Scenario, Hunk, Suggestion, Finding, Verdict, File, …). Field labels
/// belong to the current finding, so they never start a new one.
fn segment_opener(line: &str) -> Option<Option<String>> {
    let content = bold_lead(line)?;
    let lead = content.split(':').next().unwrap_or(content).trim();
    if is_field_label(lead) {
        return None;
    }
    if leads_with_number(content) {
        // A numbered lead opens; its severity comes from an embedded word
        // (e.g. `**1. … (Severe)**`) if present, else stays unknown for now.
        return Some(severity_from_text(content));
    }
    if let Some(sev) = severity_lead(content) {
        return Some(Some(sev));
    }
    if let Some(sev) = verdict_lead(content) {
        return Some(Some(sev));
    }
    None
}

/// The text of the first `**bold**` run at the start of `line`, or `None` when
/// the line does not begin with `**`. Tolerates an unclosed run (takes the rest
/// of the line).
fn bold_lead(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix("**")?;
    let end = rest.find("**").unwrap_or(rest.len());
    Some(rest[..end].trim())
}

/// Case-insensitive check that a bold lead is a known FIELD label (so it stays
/// part of the current finding rather than opening a new one). A keyword marks
/// a label only when it is the WHOLE lead, or a whole word at the lead's START
/// or END — `Concrete Failure Scenario`, `Diff Hunk`, `Instruction Added`, and
/// `File (path)` all resolve to labels. A finding title that merely *contains* a
/// keyword in the middle (`Missing file check`) is NOT a label, so it can open a
/// new finding instead of being swallowed.
fn is_field_label(lead: &str) -> bool {
    let l = lead.trim().trim_end_matches(':').to_lowercase();
    if l.is_empty() {
        return false;
    }
    const LABEL_KEYWORDS: &[&str] = &[
        "finding",
        "scenario",
        "hunk",
        "suggestion",
        "attack vector",
        "cwe",
        "file",
        "instruction",
        "impact",
        "fix",
        "evidence",
        "defect",
        "verdict",
        "note",
        "rationale",
        "recommendation",
        "reference",
        "detail",
        "description",
        "summary",
        "context",
        "mitigation",
        "remediation",
    ];
    LABEL_KEYWORDS.iter().any(|k| lead_matches_label(&l, k))
}

/// True when keyword `k` marks lead `l` as a field label: `l` equals `k`, or `k`
/// appears as a whole word at the start or end of `l`. Whole-word boundaries
/// prevent a keyword buried mid-title (`missing file check`) from matching,
/// while still catching real labels that prefix/suffix a keyword (`concrete
/// failure scenario`, `instruction added`, `attack vector`).
fn lead_matches_label(l: &str, k: &str) -> bool {
    if l == k {
        return true;
    }
    // Whole-word at the START: `k` followed by a word boundary (non-alnum).
    if let Some(after) = l.strip_prefix(k) {
        if after.chars().next().is_none_or(|c| !c.is_alphanumeric()) {
            return true;
        }
    }
    // Whole-word at the END: `k` preceded by a word boundary (non-alnum).
    if let Some(before) = l.strip_suffix(k) {
        if before.chars().last().is_none_or(|c| !c.is_alphanumeric()) {
            return true;
        }
    }
    false
}

/// True if `content` leads with an ordinal like `1.` or `2)`.
fn leads_with_number(content: &str) -> bool {
    let c = content.trim_start();
    let digits: String = c.chars().take_while(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        return false;
    }
    let after = &c[digits.len()..];
    matches!(after.chars().next(), Some('.') | Some(')') | Some(':'))
}

/// If `content` leads with a severity word (optionally in `[…]`), return the
/// canonical severity level.
fn severity_lead(content: &str) -> Option<String> {
    let first = content
        .split_whitespace()
        .next()?
        .trim_matches(|c: char| c == '[' || c == ']' || c == ':' || c == '.');
    severity_word(first)
}

/// Scan `text`'s tokens for an embedded severity word (e.g. a `(Severe)` tag),
/// returning the first canonical level found.
fn severity_from_text(text: &str) -> Option<String> {
    text.split(|c: char| !c.is_ascii_alphabetic())
        .find_map(severity_word)
}

/// Map a single token to a canonical severity level, or `None`. `severe` folds
/// to `high` (the persona vocabulary is critical|high|medium|low).
fn severity_word(tok: &str) -> Option<String> {
    match tok.trim().to_lowercase().as_str() {
        "critical" => Some("critical".to_string()),
        "severe" | "high" => Some("high".to_string()),
        "medium" | "moderate" => Some("medium".to_string()),
        "low" | "minor" => Some("low".to_string()),
        _ => None,
    }
}

/// If `content` leads with a verdict phrase, map it to a severity level.
fn verdict_lead(content: &str) -> Option<String> {
    map_verdict(&content.to_lowercase())
}

/// Verdict → severity mapping (documented once):
/// - `request changes` / `reject` / `block` → `high`
/// - `rethink` → `medium`
/// - `approve` → `low`
///
/// `needle` is matched with `starts_with` for a leading phrase and `contains`
/// for a `Verdict: …` line (see [`scan_severity`]).
fn map_verdict(text: &str) -> Option<String> {
    let t = text.trim();
    if t.starts_with("request change")
        || t.starts_with("requests change")
        || t.starts_with("reject")
        || t.starts_with("block")
    {
        Some("high".to_string())
    } else if t.starts_with("rethink") {
        Some("medium".to_string())
    } else if t.starts_with("approve") {
        Some("low".to_string())
    } else {
        None
    }
}

/// Derive a segment's severity from its body when the opener gave no hint: a
/// leading severity word wins; otherwise a `Verdict:` line's phrase is mapped.
fn scan_severity(seg_lines: &[String]) -> Option<String> {
    // Pass 1: a leading severity word / bracket anywhere in the segment.
    for line in seg_lines {
        let stripped = strip_lead_markup(line);
        if let Some(sev) = severity_lead(stripped) {
            return Some(sev);
        }
    }
    // Pass 2: a `Verdict: …` line (bold or plain).
    for line in seg_lines {
        let stripped = strip_lead_markup(line).trim();
        let lower = stripped.to_lowercase();
        if let Some(rest) = lower.strip_prefix("verdict") {
            let rest = rest.trim_start_matches([':', ' ', '*']).trim();
            if let Some(sev) = map_verdict(rest) {
                return Some(sev);
            }
        }
    }
    None
}

/// Strip leading list bullets and `*`/`#`/space markup so a line's first
/// semantic token can be inspected.
fn strip_lead_markup(line: &str) -> &str {
    line.trim_start()
        .trim_start_matches(['-', '*', '+', '#', '>', ' '])
        .trim_start()
}

/// Compute a hoisted default location shared across segments: a `File:` /
/// `### File:` header (anywhere) is preferred, else a single path-like span
/// salvaged from the preamble. Returns a file-or-file:line ref (never a
/// fabricated line).
/// A segment's OWN location: the first `File:` / `**File**:` field appearing
/// inside the segment's own lines. Unlike [`compute_hoist`] (which scans the
/// whole response for a shared header), this looks only within one segment, so
/// two segments that each name a different file are located distinctly instead
/// of both inheriting the first. Returns a file-or-file:line ref (never a
/// fabricated line).
fn segment_file_field(seg_lines: &[String]) -> Option<CanonRef> {
    for line in seg_lines {
        let bare = line.trim_start().trim_start_matches('#').trim_start();
        if let Some(val) = parse_field(bare, "file") {
            let r = CanonRef::parse(&val);
            if r.is_usable() {
                return Some(r);
            }
        }
    }
    None
}

fn compute_hoist(lines: &[&str], preamble_end: usize) -> Option<CanonRef> {
    // A drifted `### File:` / `**File…**` / `File:` header is a shared LOCATION
    // source — but ONLY when it sits in the PREAMBLE (before the first segment).
    // A `File:` inside a later segment belongs to THAT segment (handled by
    // `segment_file_field`); scanning the whole response here would let an
    // earlier segment with no file of its own wrongly inherit a sibling's path.
    let scan_end = preamble_end.min(lines.len());
    for line in &lines[..scan_end] {
        let bare = line.trim_start().trim_start_matches('#').trim_start();
        if let Some(val) = parse_field(bare, "file") {
            let r = CanonRef::parse(&val);
            if r.is_usable() {
                return Some(r);
            }
        }
    }
    // Fallback: salvage a single unambiguous path-like span from the preamble.
    let preamble: Vec<String> = lines[..scan_end]
        .iter()
        .map(|l| l.to_string())
        .collect();
    salvage_reference(&[preamble.as_slice()])
}

/// Accumulates the parts of one finding across multiple input lines.
struct Builder {
    severity: String,
    desc: Vec<String>,
    reference: Option<String>,
    suggestion: Option<Vec<String>>,
    /// Which multi-line field a bare continuation line should extend.
    open: OpenField,
}

#[derive(PartialEq)]
enum OpenField {
    Description,
    Suggestion,
    None,
}

impl Builder {
    fn new(severity: String, first_desc_line: String) -> Self {
        let mut desc = Vec::new();
        if !first_desc_line.is_empty() {
            desc.push(first_desc_line);
        }
        Builder {
            severity,
            desc,
            reference: None,
            suggestion: None,
            open: OpenField::Description,
        }
    }

    fn set_reference(&mut self, val: String) {
        self.reference = Some(val);
        // A field line closes any open free-text field.
        self.open = OpenField::None;
    }

    fn start_suggestion(&mut self, val: String) {
        let mut lines = Vec::new();
        if !val.is_empty() {
            lines.push(val);
        }
        self.suggestion = Some(lines);
        self.open = OpenField::Suggestion;
    }

    fn end_suggestion(&mut self) {
        self.open = OpenField::None;
    }

    /// Extend the currently-open multi-line field with a continuation line.
    fn continuation(&mut self, line: &str) {
        match self.open {
            OpenField::Description => {
                // Skip leading blank lines before any description text, but keep
                // internal structure once text has started.
                if self.desc.is_empty() && line.trim().is_empty() {
                    return;
                }
                self.desc.push(line.to_string());
            }
            OpenField::Suggestion => {
                if let Some(s) = self.suggestion.as_mut() {
                    if s.is_empty() && line.trim().is_empty() {
                        return;
                    }
                    s.push(line.to_string());
                }
            }
            OpenField::None => {}
        }
    }

    /// Finalize and append to `out`, unless the description is empty.
    fn push_into(self, out: &mut Vec<Finding>, source: &str) {
        let description = join_lines(&self.desc);
        if description.is_empty() {
            return;
        }
        let reference = match self.reference.as_deref() {
            // A `**File**:` field was present: canonicalize it as before. Any
            // line it carries is kept — only the Layer B salvage path below is
            // constrained to file-only refs.
            Some(raw) => Some(CanonRef::parse(raw)).filter(CanonRef::is_usable),
            // Layer B — the block had no `**File**:` field. Try to salvage a
            // location CONSERVATIVELY from a single unambiguous inline path
            // span mentioned in the block (description or suggestion lines).
            None => {
                let suggestion_lines: &[String] = self.suggestion.as_deref().unwrap_or(&[]);
                salvage_reference(&[self.desc.as_slice(), suggestion_lines])
            }
        };
        let suggestion = self
            .suggestion
            .as_ref()
            .map(|s| join_lines(s))
            .filter(|s| !s.is_empty());

        out.push(Finding {
            severity: self.severity,
            reference,
            description,
            suggestion,
            source: Some(source.to_string()),
        });
    }
}

/// Collapse a set of accumulated lines into a single normalized string: trim
/// each line's trailing space, drop trailing blank lines, and join the rest
/// with single spaces (the merge compares on a normalized single line).
fn join_lines(lines: &[String]) -> String {
    let mut parts: Vec<&str> = lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    // `parts` already has blanks removed; join with a space.
    let joined = {
        let mut s = String::new();
        for (i, p) in parts.drain(..).enumerate() {
            if i > 0 {
                s.push(' ');
            }
            s.push_str(p);
        }
        s
    };
    joined.trim().to_string()
}

/// Try to parse a heading line into `(severity, first_description_line)`.
///
/// Accepts non-strict variants:
/// - any run of `#` (2+) then optional spaces, then `[severity]`
/// - `### [high] desc`, `###  [High]  desc`, `## [high]: desc`
/// - a heading with a `[severity]` but no following text yields an empty
///   first description line (continuation lines may still fill it)
/// - `unknown` severity when the bracket is malformed but the line is clearly a
///   finding heading (`### something`)
fn parse_heading(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("##")?; // at least two '#'
    let rest = rest.trim_start_matches('#');
    let rest = rest.trim_start();

    if let Some(after_open) = rest.strip_prefix('[') {
        if let Some(close) = after_open.find(']') {
            let severity = after_open[..close].trim().to_lowercase();
            let mut desc = after_open[close + 1..].trim_start();
            // Tolerate `### [high]: desc` and `### [high] - desc`.
            desc = desc
                .strip_prefix(':')
                .or_else(|| desc.strip_prefix('-'))
                .unwrap_or(desc)
                .trim_start();
            let severity = if severity.is_empty() {
                "unknown".to_string()
            } else {
                severity
            };
            return Some((severity, desc.to_string()));
        }
    }

    // A heading line with no `[severity]` bracket: treat the whole remainder as
    // the description with an `unknown` severity, but only if there is actual
    // text (so a bare `###` divider is not mistaken for a finding).
    if !rest.is_empty() {
        return Some(("unknown".to_string(), rest.to_string()));
    }

    None
}

/// If `line` is a `**Field**:` / `**Field:**` / `Field:` label (optionally
/// behind a list bullet), return the trimmed value after the label. `name` is
/// matched case-insensitively.
fn parse_field(line: &str, name: &str) -> Option<String> {
    // Strip an optional leading list bullet (`- `, `* `, `+ `).
    let line = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
        .unwrap_or(line)
        .trim_start();

    // Strip optional bold markers around the label: `**File**:` or `**File:**`
    // or `File:`. We normalize by removing `*` characters from the label region
    // only up to the first `:`.
    let colon = line.find(':')?;
    let (label_region, after) = line.split_at(colon);
    let after = &after[1..]; // skip ':'

    // The label is everything before ':' with `*` and whitespace stripped.
    let label: String = label_region
        .chars()
        .filter(|c| *c != '*')
        .collect::<String>()
        .trim()
        .to_lowercase();

    if label == name {
        // For `**File:**` style, the closing `**` landed after the colon;
        // strip a leading `*` run from the value.
        let val = after.trim_start_matches('*').trim();
        Some(val.to_string())
    } else {
        None
    }
}

/// Layer B salvage. When a finding block carried **no `**File**:` field**, try
/// to recover a location from a SINGLE unambiguous inline backtick code-span
/// that looks like a path. Conservative by design — the asymmetry is the whole
/// point: a MISSED salvage is safely deferred to `## Unmatched` (recoverable),
/// while a WRONG salvage silently corrupts consensus/disputes. When in doubt we
/// do NOT salvage.
///
/// Rules (see Phase 3 spec):
/// 1. Consider only inline single-backtick code-spans across the whole block
///    (description + suggestion lines). A bare prose filename that is NOT inside
///    a backtick span is deliberately ignored, to avoid false positives from
///    ordinary prose.
/// 2. Keep a span only if its file component "looks like a path": it contains a
///    `/` or ends in a known code/doc extension.
/// 3. Salvage iff there is **exactly one distinct** path-like file. Zero → no
///    ref. Two or more distinct → ambiguous → no ref (never guess).
/// 4. NEVER synthesize a line: the salvaged ref is file-only (`line: None`),
///    even when the span itself carried a trailing `:line`/range. A fabricated
///    line breaks the exact-key match when two models cite different hunk lines,
///    so it is strictly worse than a file-only ref.
fn salvage_reference(line_groups: &[&[String]]) -> Option<CanonRef> {
    let mut distinct: Vec<String> = Vec::new();
    for group in line_groups {
        for line in *group {
            for span in code_spans(line) {
                if let Some(file) = path_like_file(&span) {
                    if !distinct.iter().any(|f| *f == file) {
                        distinct.push(file);
                    }
                }
            }
        }
    }

    // Exactly one distinct path-like span salvages; zero or many do not.
    match distinct.as_slice() {
        [only] => Some(CanonRef {
            file: only.clone(),
            line: None,
        }),
        _ => None,
    }
    .filter(CanonRef::is_usable)
}

/// Extract the contents of every CLOSED inline single-backtick code-span on one
/// line. A trailing unpaired backtick opens no span and is ignored. Spans never
/// cross a line boundary, so scanning per line is correct; a triple-backtick
/// fence marker yields only an empty span (rejected downstream as non-path).
fn code_spans(line: &str) -> Vec<String> {
    let segments: Vec<&str> = line.split('`').collect();
    // N backticks -> N+1 segments -> N/2 closed pairs; span p is segment 2p+1.
    let pairs = segments.len().saturating_sub(1) / 2;
    (0..pairs).map(|p| segments[2 * p + 1].to_string()).collect()
}

/// If `span` looks like a file path, return its file component with any trailing
/// `:line`/range peeled off (the line is deliberately discarded — salvage never
/// synthesizes a line). Returns `None` for spans that do not look like a path.
///
/// Reuses [`CanonRef::parse`] purely for its file/line split so the path-likeness
/// test runs on the same file component the matcher would key on.
fn path_like_file(span: &str) -> Option<String> {
    if span.trim().is_empty() {
        return None;
    }
    let file = CanonRef::parse(span).file;
    if looks_like_path(&file) {
        Some(file)
    } else {
        None
    }
}

/// A file component "looks like a path" if it contains a `/` or ends in one of a
/// tight set of known code/doc extensions. Kept intentionally narrow: broadening
/// it trades a safe missed-salvage for a risky wrong-salvage.
fn looks_like_path(file: &str) -> bool {
    let f = file.trim();
    if f.is_empty() {
        return false;
    }
    if f.contains('/') {
        return true;
    }
    const EXTS: &[&str] = &[
        ".rs", ".ts", ".tsx", ".js", ".py", ".go", ".sh", ".md", ".json", ".toml", ".yaml",
        ".yml", ".txt",
    ];
    let lower = f.to_lowercase();
    EXTS.iter().any(|ext| lower.ends_with(ext))
}

/// Decode literal `\uXXXX` escape sequences (including UTF-16 surrogate pairs)
/// in `input`, leaving every other character — including already-decoded
/// newlines/tabs from `parse.sh extract` — untouched.
///
/// Invalid or truncated escapes are left verbatim so the parser never loses
/// content it cannot interpret.
fn decode_unicode_escapes(input: &str) -> String {
    if !input.contains("\\u") {
        return input.to_string();
    }

    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        // Look for the 6-byte sequence `\uXXXX`.
        if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] == b'u' {
            if let Some(hi) = parse_hex4(&bytes[i + 2..]) {
                // Possible high surrogate -> look for a following \uXXXX low.
                if (0xD800..=0xDBFF).contains(&hi) {
                    let lo_start = i + 6;
                    if lo_start + 1 < bytes.len()
                        && bytes[lo_start] == b'\\'
                        && bytes[lo_start + 1] == b'u'
                    {
                        if let Some(lo) = parse_hex4(&bytes[lo_start + 2..]) {
                            if (0xDC00..=0xDFFF).contains(&lo) {
                                let c =
                                    0x10000 + (((hi - 0xD800) as u32) << 10) + (lo - 0xDC00) as u32;
                                if let Some(ch) = char::from_u32(c) {
                                    out.push(ch);
                                    i = lo_start + 6;
                                    continue;
                                }
                            }
                        }
                    }
                    // Unpaired high surrogate: emit verbatim, advance one byte.
                    out.push('\\');
                    i += 1;
                    continue;
                }

                // BMP scalar (or unpaired low surrogate). char::from_u32 returns
                // None for lone surrogates, in which case we keep the literal.
                if let Some(ch) = char::from_u32(hi as u32) {
                    out.push(ch);
                    i += 6;
                    continue;
                }
            }
            // Not a valid \uXXXX: emit the backslash verbatim and move on.
            out.push('\\');
            i += 1;
            continue;
        }

        // Copy the current UTF-8 character whole.
        let ch_len = utf8_len(bytes[i]);
        let end = (i + ch_len).min(bytes.len());
        out.push_str(&input[i..end]);
        i = end;
    }

    out
}

/// Parse exactly four hex digits at the start of `b` into a `u16`.
fn parse_hex4(b: &[u8]) -> Option<u16> {
    if b.len() < 4 {
        return None;
    }
    let mut v: u16 = 0;
    for &d in &b[..4] {
        let nibble = match d {
            b'0'..=b'9' => d - b'0',
            b'a'..=b'f' => d - b'a' + 10,
            b'A'..=b'F' => d - b'A' + 10,
            _ => return None,
        };
        v = (v << 4) | nibble as u16;
    }
    Some(v)
}

/// UTF-8 byte length from a leading byte.
fn utf8_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else if b >> 3 == 0b11110 {
        4
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_simple_block() {
        let text = "### [high] Off-by-one in loop\n\
                    **File**: `src/main.rs:42`\n\
                    **Suggestion**: Use <= instead of <.\n";
        let f = parse_findings(text, "GPT");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, "high");
        assert_eq!(f[0].description, "Off-by-one in loop");
        assert_eq!(f[0].reference.as_ref().unwrap().key(), "src/main.rs:42");
        assert_eq!(f[0].suggestion.as_deref(), Some("Use <= instead of <."));
        assert_eq!(f[0].source.as_deref(), Some("GPT"));
    }

    #[test]
    fn folds_multi_line_description() {
        // The shell scraper dropped everything after the heading line.
        let text = "### [medium] The handler does not validate the request body\n\
                    before dereferencing the user id, which can panic on\n\
                    a malformed payload.\n\
                    **File**: src/api.rs:88\n";
        let f = parse_findings(text, "GPT");
        assert_eq!(f.len(), 1);
        assert_eq!(
            f[0].description,
            "The handler does not validate the request body before \
             dereferencing the user id, which can panic on a malformed payload."
        );
        assert_eq!(f[0].reference.as_ref().unwrap().key(), "src/api.rs:88");
    }

    #[test]
    fn folds_multi_line_suggestion() {
        let text = "### [low] Naming nit\n\
                    **File**: src/x.rs:3\n\
                    **Suggestion**: Rename `tmp` to something descriptive\n\
                    like `parsed_config` to aid readers.\n";
        let f = parse_findings(text, "Gemini");
        assert_eq!(f.len(), 1);
        assert_eq!(
            f[0].suggestion.as_deref(),
            Some("Rename `tmp` to something descriptive like `parsed_config` to aid readers.")
        );
    }

    #[test]
    fn decodes_unicode_escapes() {
        // `‘`/`’` are smart quotes; `é` is é.
        let text = "### [low] Use the \\u2018right\\u2019 quotes for caf\\u00e9\n\
                    **File**: src/i18n.rs:5\n";
        let f = parse_findings(text, "GPT");
        assert_eq!(f.len(), 1);
        assert_eq!(
            f[0].description,
            "Use the \u{2018}right\u{2019} quotes for café"
        );
    }

    #[test]
    fn decodes_surrogate_pair() {
        // U+1F600 GRINNING FACE as a UTF-16 surrogate pair.
        let text = "### [low] Emoji \\ud83d\\ude00 in source\n**File**: a.rs:1\n";
        let f = parse_findings(text, "GPT");
        assert_eq!(f[0].description, "Emoji \u{1F600} in source");
    }

    #[test]
    fn invalid_escape_kept_verbatim() {
        let text = "### [low] A windows path C:\\users\\x and \\uZZZZ\n**File**: a.rs:1\n";
        let f = parse_findings(text, "GPT");
        // `\u` followed by non-hex stays literal; other backslashes untouched.
        assert!(f[0].description.contains("\\uZZZZ"));
        assert!(f[0].description.contains("C:\\users\\x"));
    }

    #[test]
    fn tolerates_nonstrict_field_spellings() {
        // `**File:**` (bold spans the colon) and a list-bulleted suggestion.
        let text = "###  [High]  Loose spacing and bold\n\
                    **File:** src/loose.rs:10\n\
                    - **Suggestion:** tighten it up\n";
        let f = parse_findings(text, "GPT");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, "high");
        assert_eq!(f[0].reference.as_ref().unwrap().key(), "src/loose.rs:10");
        assert_eq!(f[0].suggestion.as_deref(), Some("tighten it up"));
    }

    #[test]
    fn heading_with_colon_separator() {
        let text = "### [high]: missing guard\n**File**: a.rs:1\n";
        let f = parse_findings(text, "GPT");
        assert_eq!(f[0].description, "missing guard");
    }

    #[test]
    fn multiple_findings_split_correctly() {
        let text = "### [high] first\n**File**: a.rs:1\n\
                    ### [low] second\n**File**: b.rs:2\n";
        let f = parse_findings(text, "GPT");
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].description, "first");
        assert_eq!(f[1].description, "second");
    }

    #[test]
    fn block_without_description_is_dropped() {
        // A bare heading with no text and no continuation yields nothing.
        let text = "### []\n**File**: a.rs:1\n";
        let f = parse_findings(text, "GPT");
        assert!(f.is_empty());
    }

    #[test]
    fn reference_less_finding_has_no_ref() {
        let text = "### [low] general advice, no file\n";
        let f = parse_findings(text, "GPT");
        assert_eq!(f.len(), 1);
        assert!(f[0].reference.is_none());
    }

    #[test]
    fn ignores_preamble_before_first_heading() {
        let text = "Here is my review.\n\nSummary: looks ok.\n\
                    ### [high] real finding\n**File**: a.rs:1\n";
        let f = parse_findings(text, "GPT");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].description, "real finding");
    }

    #[test]
    fn empty_text_yields_no_findings() {
        assert!(parse_findings("", "GPT").is_empty());
        assert!(parse_findings("just prose, no findings here", "GPT").is_empty());
    }

    // --- Phase 3: Layer B — conservative single-span location salvage. ---

    #[test]
    fn salvages_single_inline_path_span_when_no_file_field() {
        // No `**File**:` field, but the block mentions exactly one path-like
        // backtick span. Salvage it as a file-only reference.
        let text = "### [high] The index is off\n\
                    The bug lives in `src/parser.rs` near the top of the loop.\n";
        let f = parse_findings(text, "GPT");
        assert_eq!(f.len(), 1);
        let r = f[0].reference.as_ref().expect("path span should be salvaged");
        assert_eq!(r.file, "src/parser.rs");
        assert_eq!(r.line, None);
        assert_eq!(r.key(), "src/parser.rs");
    }

    #[test]
    fn ambiguous_two_inline_path_spans_salvages_nothing() {
        // Two DISTINCT path-like spans -> ambiguous -> do not guess, leave None.
        let text = "### [high] The two files disagree\n\
                    `src/a.rs` and `src/b.rs` use different formats.\n";
        let f = parse_findings(text, "GPT");
        assert_eq!(f.len(), 1);
        assert!(
            f[0].reference.is_none(),
            "two path-like spans are ambiguous and must NOT salvage a reference"
        );
    }

    #[test]
    fn bare_prose_filename_is_not_salvaged() {
        // `main.rs` looks like a path, but it is NOT inside a backtick span, so
        // ordinary prose must never manufacture a reference.
        let text = "### [medium] There is a problem in main.rs somewhere\n\
                    The handler in main.rs does not validate its input.\n";
        let f = parse_findings(text, "GPT");
        assert_eq!(f.len(), 1);
        assert!(
            f[0].reference.is_none(),
            "a bare prose filename (no backticks) must not create a reference"
        );
    }

    #[test]
    fn salvage_never_fabricates_a_line_from_span_or_hunk() {
        // The salvage span carries a `:42`, and a `@@ -40,6 +42,8 @@` hunk is
        // present. Neither may become a line number: the salvaged ref is
        // file-only (a fabricated line breaks the exact-key match).
        let text = "### [high] Off-by-one in the loop bound\n\
                    See `src/loop.rs:42` inside hunk `@@ -40,6 +42,8 @@`.\n";
        let f = parse_findings(text, "GPT");
        assert_eq!(f.len(), 1);
        let r = f[0]
            .reference
            .as_ref()
            .expect("the single path-like span should salvage a file");
        assert_eq!(r.file, "src/loop.rs");
        assert_eq!(r.line, None, "salvage must never synthesize a line");
        assert_eq!(r.key(), "src/loop.rs");
    }

    #[test]
    fn explicit_file_field_is_not_overridden_by_inline_span() {
        // A real `**File**:` field is authoritative: salvage must not run, must
        // not override it, and the field's line is preserved.
        let text = "### [high] Off-by-one in loop\n\
                    **File**: `src/main.rs:42`\n\
                    The issue is also near `src/other.rs` in a comment.\n";
        let f = parse_findings(text, "GPT");
        assert_eq!(f.len(), 1);
        let r = f[0].reference.as_ref().expect("File field ref present");
        assert_eq!(r.file, "src/main.rs");
        assert_eq!(r.line, Some(42));
        assert_eq!(r.key(), "src/main.rs:42");
    }

    #[test]
    fn non_path_code_span_is_not_salvaged() {
        // A backtick span that does not look like a path (a bare identifier with
        // no `/` and no known extension) must not be salvaged.
        let text = "### [low] Rename the temporary\n\
                    Rename `tmp` to something descriptive.\n";
        let f = parse_findings(text, "GPT");
        assert_eq!(f.len(), 1);
        assert!(
            f[0].reference.is_none(),
            "a non-path code span must not create a reference"
        );
    }

    // --- Phase 4b: prose-recovery helper unit tests + the gating fence. ---
    //
    // These pin the closed boundary rule (`segment_opener`), the severity
    // recovery table (`map_verdict`/`severity_word`/`scan_severity`), the shared
    // location hoist, and — most importantly — the gating invariant that a
    // conforming `### [severity]` block never reaches the prose stage.

    #[test]
    fn boundary_numbered_bold_lead_opens_finding() {
        // A numbered bold lead (`**1. …**`) OPENS a new finding. With no embedded
        // severity word its hint is `None` (severity is recovered from the body
        // later); an embedded `(Severe)` rides along as the hint.
        assert_eq!(
            segment_opener("**1. The retry loop never backs off**"),
            Some(None),
            "a numbered bold lead must open a finding"
        );
        assert_eq!(
            segment_opener("**1. Missing downstream consumption (Severe)**"),
            Some(Some("high".to_string())),
            "an embedded severity word rides along on the numbered lead"
        );
    }

    #[test]
    fn boundary_severity_word_bold_lead_opens_finding() {
        // A bracketed severity word opens with that severity...
        assert_eq!(
            segment_opener("**[high] SQL injection in the query**"),
            Some(Some("high".to_string()))
        );
        // ...and so does a bare severity word before a colon.
        assert_eq!(
            segment_opener("**Critical: buffer overflow on parse**"),
            Some(Some("critical".to_string()))
        );
    }

    #[test]
    fn boundary_verdict_phrase_bold_lead_opens_finding() {
        // A verdict-phrase bold lead opens and maps to a severity.
        assert_eq!(
            segment_opener("**Request changes: wire the PR intent into step 4**"),
            Some(Some("high".to_string()))
        );
    }

    #[test]
    fn boundary_field_label_bolds_do_not_open_a_finding() {
        // Field-label bolds belong to the CURRENT finding; they must NEVER open a
        // new one, or the recovery stage would shatter one finding into many.
        for label in [
            "**Defect:**",
            "**Failure Scenario:**",
            "**Suggestion:**",
            "**Attack vector:**",
            "**CWE:**",
            "**Impact:**",
            "**Fix:**",
        ] {
            assert!(
                segment_opener(label).is_none(),
                "field label {label:?} must NOT open a new finding"
            );
        }
    }

    #[test]
    fn severity_recovery_from_verdict_line_maps_exactly() {
        // A `Verdict: …` line inside a segment yields the mapped severity.
        assert_eq!(
            scan_severity(&["Verdict: request changes".to_string()]),
            Some("high".to_string())
        );
        assert_eq!(
            scan_severity(&["Verdict: rethink".to_string()]),
            Some("medium".to_string())
        );
        assert_eq!(
            scan_severity(&["Verdict: approve".to_string()]),
            Some("low".to_string())
        );
        // A leading severity word is recovered directly, no verdict needed.
        assert_eq!(
            scan_severity(&["Critical flaw in the parser".to_string()]),
            Some("critical".to_string())
        );
        // Pure prose with neither a severity word nor a verdict recovers nothing.
        assert_eq!(scan_severity(&["Just some ordinary prose.".to_string()]), None);
    }

    #[test]
    fn verdict_and_severity_word_mapping_is_pinned() {
        // Verdict phrases (`map_verdict`): request changes / reject / block ->
        // high, rethink -> medium, approve -> low.
        assert_eq!(map_verdict("request changes"), Some("high".to_string()));
        assert_eq!(map_verdict("reject"), Some("high".to_string()));
        assert_eq!(map_verdict("block"), Some("high".to_string()));
        assert_eq!(map_verdict("rethink"), Some("medium".to_string()));
        assert_eq!(map_verdict("approve"), Some("low".to_string()));
        assert_eq!(map_verdict("looks good and nothing else"), None);
        // Severity words (`severity_word`): severe -> high, moderate -> medium,
        // minor -> low, and the canonical vocabulary maps to itself.
        assert_eq!(severity_word("severe"), Some("high".to_string()));
        assert_eq!(severity_word("moderate"), Some("medium".to_string()));
        assert_eq!(severity_word("minor"), Some("low".to_string()));
        assert_eq!(severity_word("critical"), Some("critical".to_string()));
        assert_eq!(severity_word("high"), Some("high".to_string()));
        assert_eq!(severity_word("medium"), Some("medium".to_string()));
        assert_eq!(severity_word("low"), Some("low".to_string()));
        assert_eq!(severity_word("banana"), None);
    }

    #[test]
    fn hoist_shares_a_single_file_header_across_all_segments() {
        // A prose review names the file ONCE in a `### File:` header, then lists
        // multiple boundaried segments that carry no location of their own. Every
        // recovered finding must inherit that hoisted, file-only location.
        let text = "\
### File: `src/thing.rs`

**1. First problem (Critical)**
The first issue is genuinely real.

**2. Second problem (Minor)**
The second issue is also real.
";
        let f = parse_findings(text, "GPT");
        assert_eq!(f.len(), 2, "two boundaried segments -> two findings");
        for finding in &f {
            let r = finding
                .reference
                .as_ref()
                .expect("each segment inherits the hoisted location");
            assert_eq!(r.key(), "src/thing.rs", "every segment gets the hoisted file");
            assert_eq!(r.line, None, "hoist is file-only; never a fabricated line");
        }
        assert_eq!(f[0].severity, "critical");
        assert_eq!(f[1].severity, "low");
    }

    #[test]
    fn gating_conforming_block_is_not_resegmented_by_prose_rules() {
        // THE GATING FENCE. If ANY conforming `### [severity]` heading is present,
        // the prose-recovery stage must NOT run. We prove it by appending lines
        // that ARE prose-segment openers (a numbered lead and a verdict lead) to a
        // conforming block: they must be swallowed as ordinary continuation, NOT
        // turned into extra findings.
        let conforming_only = "\
### [high] Real conforming finding
**File**: `src/main.rs:10`
";
        // Sanity: the trailing lines really WOULD open prose segments if the stage
        // ran, so the gate is what makes the difference (not inert text).
        assert!(segment_opener("**1. A numbered lead that would open a segment**").is_some());
        assert!(
            segment_opener("**Request changes: a verdict lead that would open a segment**")
                .is_some()
        );

        let mixed = format!(
            "{conforming_only}**1. A numbered lead that would open a segment**\n\
             **Request changes: a verdict lead that would open a segment**\n"
        );

        // The known conforming sample parses to exactly one high finding — the
        // pre-4a behavior the prose stage must never perturb.
        let expected = parse_findings(conforming_only, "GPT");
        assert_eq!(expected.len(), 1);
        assert_eq!(expected[0].severity, "high");
        assert_eq!(expected[0].description, "Real conforming finding");
        assert_eq!(
            expected[0].reference.as_ref().unwrap().key(),
            "src/main.rs:10"
        );

        // The mixed input must produce byte-identical findings: the conforming
        // block flowed through the existing parser untouched and the prose
        // openers did NOT re-segment it.
        let got = parse_findings(&mixed, "GPT");
        assert_eq!(
            got, expected,
            "a conforming block must be parsed by the existing path; prose \
             openers must not re-segment it"
        );
    }

    // ---- Review-round regression fixes (found by dogfooding /xavier review) ----

    #[test]
    fn field_label_keyword_midtitle_still_opens_finding() {
        // Fix 1: `is_field_label` used substring `contains`, so a finding title
        // that merely CONTAINED a label keyword ("file") was swallowed. A
        // numbered lead whose title contains "file" must still open a finding.
        assert!(
            segment_opener("**1. Missing file check**").is_some(),
            "a numbered title containing 'file' must open a finding, not be swallowed"
        );
        // But genuine field labels (keyword as a whole word at start/end) still
        // must NOT open — including the multi-word ones.
        for label in [
            "**Defect:**",
            "**Failure Scenario:**",
            "**Attack vector:**",
            "**Instruction Added:**",
            "**CWE:**",
        ] {
            assert!(
                segment_opener(label).is_none(),
                "field label {label:?} must still NOT open a finding"
            );
        }
    }

    #[test]
    fn non_severity_bracket_heading_is_not_conforming() {
        // Fix 2: a bracket holding a CATEGORY (not a severity) must NOT count as
        // a conforming heading, or the gate skips prose recovery for a prose
        // review that merely used a `## [Correctness] Findings` header.
        assert!(is_conforming_heading("### [high] real finding"));
        assert!(is_conforming_heading("## [critical] x"));
        assert!(!is_conforming_heading("## [Correctness] Findings"));
        assert!(!is_conforming_heading("### [Security] notes"));
        // Empty bracket stays on the existing path (degenerate, yields nothing).
        assert!(is_conforming_heading("### []"));
    }

    #[test]
    fn each_segment_keeps_its_own_file_field() {
        // Fix 3: two prose segments that each name their OWN `**File**:` must be
        // located distinctly — the second must not inherit the first's path via
        // the shared hoist.
        let text = "\
**1. First issue (high)**
**File**: `src/alpha.rs`
Something is wrong here.

**2. Second issue (high)**
**File**: `src/beta.rs`
Something else is wrong there.
";
        let f = parse_findings(text, "GPT");
        assert_eq!(f.len(), 2, "two numbered segments -> two findings");
        assert_eq!(f[0].reference.as_ref().unwrap().file, "src/alpha.rs");
        assert_eq!(
            f[1].reference.as_ref().unwrap().file,
            "src/beta.rs",
            "the second segment must keep its OWN file, not inherit the first's"
        );
    }

    #[test]
    fn bullet_list_prose_recovers_multiple_findings() {
        // Fix 4: a bullet-list review with NO bold openers, where each top-level
        // bullet carries its own path-like span, must recover one finding per
        // bullet instead of collapsing into a single blob. A shared `Verdict:`
        // hoists a real severity onto each.
        let text = "\
Here are the problems I found.

- `src/a.rs`: the first function mishandles the empty case.
- `src/b.rs`: the second function leaks a handle on the error path.

Verdict: request changes.
";
        let f = parse_findings(text, "GPT");
        assert_eq!(f.len(), 2, "two path-bearing bullets -> two findings");
        assert!(
            f.iter().all(|x| x.severity == "high"),
            "the shared `Verdict: request changes` hoists 'high' onto each bullet"
        );
        assert_eq!(f[0].reference.as_ref().unwrap().file, "src/a.rs");
        assert_eq!(f[1].reference.as_ref().unwrap().file, "src/b.rs");
    }

    #[test]
    fn narrative_bullets_without_paths_do_not_oversplit() {
        // Fix 4 guard: ordinary narrative bullets (no path span) under a single
        // prose finding must NOT each become a finding — only path-bearing
        // top-level bullets are boundaries. This keeps the `sec_gpt` shape (sub-
        // bullets under one bold lead) intact.
        let text = "\
**Request changes: the handler is unsafe.**
`src/handler.rs`

- it does not validate the input length
- it logs the raw token
- it retries without backoff

Verdict: request changes.
";
        let f = parse_findings(text, "GPT");
        assert_eq!(
            f.len(),
            1,
            "a single bold-lead finding with narrative sub-bullets stays one finding"
        );
        assert_eq!(f[0].severity, "high");
        assert_eq!(f[0].reference.as_ref().unwrap().file, "src/handler.rs");
    }

    // ---- Round-2 regression fixes (found by re-reviewing the fixed branch) ----

    #[test]
    fn hoist_does_not_leak_a_later_segments_file_backward() {
        // Fix 5: a segment with NO file of its own must not inherit a LATER
        // segment's `**File**:` via the shared hoist. The hoist header scan is
        // preamble-only; a segment file belongs to that segment alone.
        let text = "\
**1. First issue (high)**
This one names no file of its own.

**2. Second issue (high)**
**File**: `src/second.rs`
This one owns src/second.rs.
";
        let f = parse_findings(text, "GPT");
        assert_eq!(f.len(), 2, "two numbered segments -> two findings");
        // Segment 2 keeps its own file.
        assert_eq!(f[1].reference.as_ref().unwrap().file, "src/second.rs");
        // Segment 1 must NOT have back-inherited src/second.rs. With no preamble
        // header and no own location, it stays reference-less.
        assert!(
            f[0].reference.is_none(),
            "segment 1 (no own file, no preamble header) must not inherit \
             segment 2's file; got {:?}",
            f[0].reference
        );
    }

    #[test]
    fn indented_path_bullet_does_not_open_a_finding() {
        // Fix 6: only TOP-LEVEL bullets (no indent) are finding boundaries. An
        // indented sub-bullet that happens to carry a path span is a sub-point
        // of the current finding, not a new finding. The key assertion is the
        // COUNT: the nested `src/nested.rs` bullet must not open a third finding.
        let text = "\
- `src/top_a.rs`: the first real finding.
    - see also `src/nested.rs` for the related helper
- `src/top_b.rs`: the second real finding.

Verdict: request changes.
";
        let f = parse_findings(text, "GPT");
        assert_eq!(
            f.len(),
            2,
            "two top-level path bullets -> two findings; the indented \
             `src/nested.rs` sub-bullet must not open a third"
        );
        // Segment 2 is a single clean top-level bullet -> located at its path.
        assert_eq!(f[1].reference.as_ref().unwrap().file, "src/top_b.rs");
        // Segment 1 now legitimately contains TWO path spans (its own bullet
        // plus the absorbed nested one), so Layer-B salvage sees an ambiguous
        // pair and correctly declines to guess a location — the important thing
        // is that the nested bullet stayed folded in, not that it opened a
        // finding. (A missed salvage is safely reference-less; a wrong one would
        // corrupt matching.)
        assert!(
            f[0].reference.is_none(),
            "segment 1 has two path spans (own + absorbed nested) -> ambiguous \
             -> no single salvaged location; got {:?}",
            f[0].reference
        );
    }
}
