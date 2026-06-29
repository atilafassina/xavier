//! Canonicalization of `file:line` references into a stable [`CanonRef`].
//!
//! A model attaches a location to a finding as free-form text on a `**File**:`
//! line, e.g. `` `src/main.rs:42` ``, `src/main.rs:40-52`, or just
//! `README.md`. Before two findings can be compared, those raw strings must be
//! reduced to a single canonical matching key so that cosmetic differences
//! (backticks, surrounding whitespace, `L`-prefixed lines, `40-52` vs
//! `40..52`) do not split an otherwise-identical location into two.
//!
//! This module owns that reduction. [`CanonRef`] itself lives in
//! [`crate::model`] (it is part of the serialized ABI); the parsing/comparison
//! logic lives here so the matcher and the markdown parser share one
//! definition of "the same place".
//!
//! Normalization rules (matching, and extending, what `parse.sh` applied):
//!
//! - Strip backticks and surrounding whitespace from the whole ref.
//! - Split on the LAST `:` so Windows-style paths (`C:/x.rs:9`) keep their
//!   drive colon and only a trailing line/range is peeled off.
//! - A trailing `<int>` is a single line. A trailing `<int>-<int>` or
//!   `<int>..<int>` (optionally with a leading `L`, e.g. `L40-L52`) is a line
//!   **range**; its canonical key uses the range's **start** line so that a
//!   single-line finding at the range start and the range itself collapse to
//!   one location (the common "same issue, one model gave a span" case).
//! - Anything that is not a clean integer/range tail is treated as part of the
//!   file path (no line).

use crate::model::CanonRef;

impl CanonRef {
    /// Build a [`CanonRef`] from a raw `file:line`-ish reference string.
    ///
    /// Applies the normalization documented at the [module level](crate::refs):
    /// strips backticks, trims, and peels a trailing single line or line range
    /// off the last `:`-separated segment. Ranges canonicalize to their start
    /// line. A blank/locationless ref yields a [`CanonRef`] whose
    /// [`is_usable`](CanonRef::is_usable) is `false`.
    pub fn parse(raw: &str) -> Self {
        let cleaned = raw.replace('`', "");
        let cleaned = cleaned.trim();

        // Split on the LAST ':' so paths containing colons (Windows drives,
        // URLs) keep their colon in the file part and only a trailing
        // line/range is treated as a line number.
        if let Some((head, tail)) = cleaned.rsplit_once(':') {
            if let Some(start) = parse_line_tail(tail) {
                return CanonRef {
                    file: head.trim().to_string(),
                    line: Some(start),
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

/// Parse the segment after the final `:` into a canonical start line.
///
/// Accepts a bare integer (`42`), an `L`-prefixed line (`L42`), or a range in
/// either `-` or `..` form (`40-52`, `40..52`, `L40-L52`). Ranges collapse to
/// their start line. Returns `None` when the tail is not a clean line/range, in
/// which case the caller keeps the colon as part of the file path.
fn parse_line_tail(tail: &str) -> Option<u64> {
    let tail = tail.trim();
    if tail.is_empty() {
        return None;
    }

    // Split a range on the first `-` or `..`. `rsplit`/`split` on `-` is safe
    // because line numbers are non-negative (no leading `-`).
    let start = if let Some((lo, hi)) = tail.split_once("..") {
        // Both sides must be clean lines for this to be a range.
        parse_one_line(hi)?;
        lo
    } else if let Some((lo, hi)) = tail.split_once('-') {
        parse_one_line(hi)?;
        lo
    } else {
        tail
    };

    parse_one_line(start)
}

/// Parse a single line token, tolerating an optional leading `L`/`l`.
fn parse_one_line(tok: &str) -> Option<u64> {
    let tok = tok.trim();
    let digits = tok.strip_prefix(['L', 'l']).unwrap_or(tok);
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_parses() {
        let r = CanonRef::parse("src/main.rs:42");
        assert_eq!(r.file, "src/main.rs");
        assert_eq!(r.line, Some(42));
        assert_eq!(r.key(), "src/main.rs:42");
    }

    #[test]
    fn backticks_and_whitespace_stripped() {
        let r = CanonRef::parse("  `src/main.rs:7` ");
        assert_eq!(r.key(), "src/main.rs:7");
    }

    #[test]
    fn range_collapses_to_start_line() {
        for raw in [
            "src/main.rs:40-52",
            "src/main.rs:40..52",
            "src/main.rs:L40-L52",
        ] {
            let r = CanonRef::parse(raw);
            assert_eq!(r.file, "src/main.rs", "{raw}");
            assert_eq!(r.line, Some(40), "{raw} should collapse to start line");
            assert_eq!(r.key(), "src/main.rs:40", "{raw}");
        }
    }

    #[test]
    fn single_line_and_range_at_same_start_share_a_key() {
        let single = CanonRef::parse("src/main.rs:40");
        let range = CanonRef::parse("src/main.rs:40-52");
        assert_eq!(single.key(), range.key());
    }

    #[test]
    fn l_prefixed_single_line() {
        let r = CanonRef::parse("src/main.rs:L99");
        assert_eq!(r.line, Some(99));
    }

    #[test]
    fn missing_line_keeps_file_only() {
        let r = CanonRef::parse("README.md");
        assert_eq!(r.line, None);
        assert_eq!(r.key(), "README.md");
        assert!(r.is_usable());
    }

    #[test]
    fn windows_drive_colon_preserved() {
        let r = CanonRef::parse("C:/weird/path.rs:99");
        assert_eq!(r.file, "C:/weird/path.rs");
        assert_eq!(r.line, Some(99));
    }

    #[test]
    fn non_integer_tail_is_part_of_path() {
        // A trailing word is not a line; the whole thing is the file.
        let r = CanonRef::parse("docs/guide.md#section");
        assert_eq!(r.file, "docs/guide.md#section");
        assert_eq!(r.line, None);
    }

    #[test]
    fn empty_ref_is_unusable() {
        let r = CanonRef::parse("   ");
        assert!(!r.is_usable());
        let r2 = CanonRef::parse("``");
        assert!(!r2.is_usable());
    }

    #[test]
    fn malformed_range_high_side_falls_back_to_file() {
        // `40-abc` is not a clean range, so the `:` stays in the path.
        let r = CanonRef::parse("file.rs:40-abc");
        assert_eq!(r.line, None);
        assert_eq!(r.file, "file.rs:40-abc");
    }
}
