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
}
