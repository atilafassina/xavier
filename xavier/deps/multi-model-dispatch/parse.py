#!/usr/bin/env python3
"""
parse.py -- Extract and merge structured findings from agent CLI stream-json output.

Forked from ACE's ace-parse.sh inline Python, extended for Xavier's multi-model
debate protocol.

Usage:
    python3 parse.py --extract <file>           # Print final assistant text
    python3 parse.py --merge <file_a> <file_b>  # Merge findings into debate format

Stdlib only -- no pip dependencies.
"""

import argparse
import json
import os
import re
import sys
from difflib import SequenceMatcher


# ---------------------------------------------------------------------------
# 1. extract_text -- read stream-json, return final assistant text
# ---------------------------------------------------------------------------

def extract_text(filepath):
    """Read a stream-json file and return the final assistant text block.

    The agent CLI outputs newline-delimited JSON. Each object has a ``type``
    field; we care about ``type: "assistant"`` objects whose ``message.content``
    contains text blocks.  We return the *last* such text block (the final
    assistant response).
    """
    texts = []
    with open(filepath) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue
            if obj.get("type") == "assistant" and "message" in obj:
                for block in obj["message"].get("content", []):
                    if block.get("type") == "text":
                        texts.append(block["text"])
    return texts[-1] if texts else ""


# ---------------------------------------------------------------------------
# 2. parse_findings -- extract structured findings from model text
# ---------------------------------------------------------------------------

_SEVERITY_RE = re.compile(
    r"^###\s+\[(?P<severity>[^\]]+)\]\s+(?P<description>.+)$", re.MULTILINE
)
_FILE_REF_RE = re.compile(
    r"\*\*File\*\*:\s*(?P<file_ref>\S+)"
)
_SUGGESTION_RE = re.compile(
    r"\*\*Suggestion\*\*:\s*(?P<suggestion>.+?)(?=\n###|\n\*\*File|\Z)",
    re.DOTALL,
)
_SCENARIO_RE = re.compile(
    r"\*\*Scenario\*\*:\s*(?P<scenario>.+?)(?=\n\*\*Suggestion|\n###|\Z)",
    re.DOTALL,
)


def parse_findings(text):
    """Parse model output text into a list of structured findings.

    Each finding dict has keys: severity, file_ref, description, suggestion.
    Parsing targets the ``### [severity] description`` format used by Xavier
    personas.  If the model deviates from the format, fewer findings will be
    extracted -- callers can fall back to raw text via ``extract_text``.
    """
    findings = []
    # Split the text at each ### heading so we can parse each finding block.
    parts = _SEVERITY_RE.split(text)
    # _SEVERITY_RE.split yields: [preamble, sev1, desc1, body1, sev2, ...]
    # Groups come in triples: (severity, description, body-until-next-match).
    # However, re.split with groups gives interleaved captures.  We iterate
    # using finditer instead for clarity.
    for match in _SEVERITY_RE.finditer(text):
        severity = match.group("severity").strip().lower()
        description = match.group("description").strip()

        # Grab the block of text after this heading until the next heading or
        # end of string.
        start = match.end()
        next_match = _SEVERITY_RE.search(text, start)
        block = text[start : next_match.start()] if next_match else text[start:]

        file_ref = ""
        file_match = _FILE_REF_RE.search(block)
        if file_match:
            file_ref = file_match.group("file_ref").strip()

        suggestion = ""
        sug_match = _SUGGESTION_RE.search(block)
        if sug_match:
            suggestion = sug_match.group("suggestion").strip()

        findings.append(
            {
                "severity": severity,
                "file_ref": file_ref,
                "description": description,
                "suggestion": suggestion,
            }
        )
    return findings


# ---------------------------------------------------------------------------
# 3. merge_findings -- classify into consensus / disputes / blindspots
# ---------------------------------------------------------------------------

def _similar(a, b, threshold=0.5):
    """Return True if two strings are similar above *threshold*."""
    return SequenceMatcher(None, a.lower(), b.lower()).ratio() >= threshold


def _same_location(fa, fb):
    """Return True if two findings reference the same file location."""
    if not fa["file_ref"] or not fb["file_ref"]:
        return False
    # Normalize: strip trailing colon, compare case-insensitively.
    return fa["file_ref"].rstrip(":").lower() == fb["file_ref"].rstrip(":").lower()


def merge_findings(findings_a, findings_b):
    """Merge two lists of findings into consensus, disputes, and blindspots.

    Returns a tuple of three lists: (consensus, disputes, blindspots).

    - **Consensus**: both models flagged the same file_ref with a similar
      description (SequenceMatcher ratio >= 0.5).
    - **Disputes**: both models flagged the same file_ref but the severity or
      nature of the description differs substantially.
    - **Blindspots**: a finding from only one model, with source attribution.
    """
    matched_a = set()
    matched_b = set()
    consensus = []
    disputes = []

    for i, fa in enumerate(findings_a):
        for j, fb in enumerate(findings_b):
            if j in matched_b:
                continue
            if not _same_location(fa, fb):
                continue
            # Same location -- decide consensus vs dispute
            if _similar(fa["description"], fb["description"]):
                consensus.append(
                    {
                        "file_ref": fa["file_ref"],
                        "severity_a": fa["severity"],
                        "severity_b": fb["severity"],
                        "description_a": fa["description"],
                        "description_b": fb["description"],
                        "suggestion_a": fa["suggestion"],
                        "suggestion_b": fb["suggestion"],
                    }
                )
            else:
                disputes.append(
                    {
                        "file_ref": fa["file_ref"],
                        "finding_a": fa,
                        "finding_b": fb,
                    }
                )
            matched_a.add(i)
            matched_b.add(j)
            break

    blindspots = []
    for i, fa in enumerate(findings_a):
        if i not in matched_a:
            blindspots.append({"source": "model_a", "finding": fa})
    for j, fb in enumerate(findings_b):
        if j not in matched_b:
            blindspots.append({"source": "model_b", "finding": fb})

    return consensus, disputes, blindspots


# ---------------------------------------------------------------------------
# 4. format_debate_output -- render merged findings as Markdown
# ---------------------------------------------------------------------------

def _severity_badge(sev):
    """Return a severity string suitable for Markdown display."""
    return f"**{sev}**"


def format_debate_output(consensus, disputes, blindspots):
    """Format the merged output as Markdown following the debate protocol.

    Sections: Consensus, Disputes, Blindspots.
    """
    lines = []

    # -- Consensus --
    lines.append("## Consensus")
    lines.append("")
    if not consensus:
        lines.append("No consensus findings -- the models did not flag the same locations with similar descriptions.")
    else:
        for item in consensus:
            sev = item["severity_a"]
            if item["severity_a"] != item["severity_b"]:
                sev = f'{item["severity_a"]} / {item["severity_b"]}'
            lines.append(f'### [{sev}] {item["description_a"]}')
            lines.append(f'**File**: {item["file_ref"]}')
            if item["suggestion_a"]:
                lines.append(f'**Suggestion (model A)**: {item["suggestion_a"]}')
            if item["suggestion_b"]:
                lines.append(f'**Suggestion (model B)**: {item["suggestion_b"]}')
            lines.append("")
    lines.append("")

    # -- Disputes --
    lines.append("## Disputes")
    lines.append("")
    if not disputes:
        lines.append("No disputes -- the models did not flag the same location with conflicting assessments.")
    else:
        for item in disputes:
            fa = item["finding_a"]
            fb = item["finding_b"]
            lines.append(f'### Dispute at {item["file_ref"]}')
            lines.append("")
            lines.append(f'**Model A** [{_severity_badge(fa["severity"])}]: {fa["description"]}')
            if fa["suggestion"]:
                lines.append(f'  Suggestion: {fa["suggestion"]}')
            lines.append("")
            lines.append(f'**Model B** [{_severity_badge(fb["severity"])}]: {fb["description"]}')
            if fb["suggestion"]:
                lines.append(f'  Suggestion: {fb["suggestion"]}')
            lines.append("")
    lines.append("")

    # -- Blindspots --
    lines.append("## Blindspots")
    lines.append("")
    if not blindspots:
        lines.append("No blindspots -- both models covered the same ground.")
    else:
        for item in blindspots:
            f = item["finding"]
            src = "Model A" if item["source"] == "model_a" else "Model B"
            lines.append(f'### [{f["severity"]}] {f["description"]}')
            lines.append(f'**File**: {f["file_ref"]}')
            lines.append(f'**Source**: {src} only')
            if f["suggestion"]:
                lines.append(f'**Suggestion**: {f["suggestion"]}')
            lines.append("")
    lines.append("")

    return "\n".join(lines)


# ---------------------------------------------------------------------------
# CLI entry point
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="Parse and merge agent CLI stream-json output."
    )
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument(
        "--extract",
        metavar="FILE",
        help="Extract final assistant text from a stream-json file.",
    )
    group.add_argument(
        "--merge",
        nargs=2,
        metavar=("FILE_A", "FILE_B"),
        help="Merge findings from two stream-json files into debate format.",
    )
    args = parser.parse_args()

    if args.extract:
        text = extract_text(args.extract)
        if text:
            print(text)
        else:
            print("(no assistant text found)", file=sys.stderr)
            sys.exit(1)
    elif args.merge:
        file_a, file_b = args.merge

        text_a = extract_text(file_a)
        text_b = extract_text(file_b)

        if not text_a and not text_b:
            print("ERROR: no assistant text found in either file.", file=sys.stderr)
            sys.exit(1)

        findings_a = parse_findings(text_a) if text_a else []
        findings_b = parse_findings(text_b) if text_b else []

        consensus, disputes, blindspots = merge_findings(findings_a, findings_b)
        print(format_debate_output(consensus, disputes, blindspots))


if __name__ == "__main__":
    main()
