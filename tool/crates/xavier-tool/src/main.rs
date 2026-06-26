//! `xavier-tool` — Xavier's native multi-subcommand binary.
//!
//! Thin CLI over the [`xavier_core`] library. The tool ABI is intentionally
//! minimal and uniform across subcommands:
//!
//! - **Input** is JSON read from **stdin**.
//! - **Output** is JSON written to **stdout**.
//! - **Status** is signaled via the process **exit code** (see below).
//!
//! Diagnostics go to stderr; stdout carries only the result JSON so callers
//! can pipe it directly.
//!
//! # Subcommands
//!
//! - `merge [--format json|debate-md]` — read a [`xavier_core::MergeInput`]
//!   (pre-parsed findings) on stdin and write the result on stdout. The default
//!   `--format json` emits a [`xavier_core::MergeResult`] (the canonical ABI);
//!   `--format debate-md` emits the equivalent
//!   Consensus/Disputes/Blindspots/Unmatched Markdown. Markdown rendering is
//!   mechanical presentation only and does not cross the determinism boundary.
//! - `merge-text [--format json|debate-md]` — read a
//!   [`xavier_core::MergeTextInput`] (each model's **raw assistant text**) on
//!   stdin. The binary parses the Markdown into findings itself, then runs the
//!   same merge. This is the preferred entry point for `merge.sh`: it moves the
//!   brittle finding-scraping off `awk` and into the tool.
//! - `--version` / `-V` — print the version and exit 0.
//! - `--help` / `-h` — print usage and exit 0.
//!
//! # Exit codes
//!
//! | Code | Meaning |
//! |------|---------|
//! | `0`  | Success — result JSON written to stdout. |
//! | `1`  | Input error — stdin was unreadable or not valid JSON for the subcommand. |
//! | `2`  | Usage error — unknown/missing subcommand or bad flags. |
//! | `3`  | Internal error — serialization or stdout write failed. |
//!
//! These codes are part of the tool's contract; callers (and the shell
//! fallback wiring) depend on them to decide whether to fall back to
//! `parse.sh`.

use std::io::{self, Read, Write};
use std::process::ExitCode;

use xavier_core::{
    debate_markdown, merge, parse_findings, MergeInput, MergeResult, MergeTextInput,
};

/// Output format for the `merge` subcommand.
enum Format {
    Json,
    DebateMarkdown,
}

/// Input could not be read or parsed.
const EXIT_INPUT_ERROR: u8 = 1;
/// Unknown subcommand or bad usage.
const EXIT_USAGE_ERROR: u8 = 2;
/// Serialization / output write failure.
const EXIT_INTERNAL_ERROR: u8 = 3;

const USAGE: &str = "\
xavier-tool — Xavier native tool

USAGE:
    xavier-tool merge      [--format json|debate-md]
    xavier-tool merge-text [--format json|debate-md]

SUBCOMMANDS:
    merge       Read MergeInput JSON (pre-parsed findings) on stdin, write the
                merge result on stdout.
    merge-text  Read MergeTextInput JSON (each model's raw assistant text) on
                stdin; parse findings in the binary, then merge.

                --format json       (default) emit MergeResult JSON.
                --format debate-md  emit Consensus/Disputes/Blindspots/Unmatched
                                    Markdown.

FLAGS:
    -h, --help       Print this help and exit.
    -V, --version    Print version and exit.

Each subcommand reads JSON on stdin and writes to stdout.
Exit codes: 0 ok, 1 input error, 2 usage error, 3 internal error.";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);

    let Some(cmd) = args.next() else {
        eprintln!("error: missing subcommand\n\n{USAGE}");
        return ExitCode::from(EXIT_USAGE_ERROR);
    };

    match cmd.as_str() {
        "merge" => run_merge(args.collect()),
        "merge-text" => run_merge_text(args.collect()),
        "-h" | "--help" => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        "-V" | "--version" => {
            println!("xavier-tool {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("error: unknown subcommand '{other}'\n\n{USAGE}");
            ExitCode::from(EXIT_USAGE_ERROR)
        }
    }
}

/// `merge` subcommand: stdin [`MergeInput`] JSON -> stdout (JSON or Markdown).
fn run_merge(args: Vec<String>) -> ExitCode {
    let format = match parse_format(&args) {
        Ok(format) => format,
        Err(code) => return code,
    };

    let raw = match read_stdin() {
        Ok(raw) => raw,
        Err(code) => return code,
    };

    let input: MergeInput = match serde_json::from_str(&raw) {
        Ok(input) => input,
        Err(e) => {
            eprintln!("error: invalid MergeInput JSON on stdin: {e}");
            return ExitCode::from(EXIT_INPUT_ERROR);
        }
    };

    emit(merge(&input), &input.label_a, &input.label_b, format)
}

/// `merge-text` subcommand: stdin [`MergeTextInput`] JSON -> stdout. The binary
/// parses each side's raw assistant text into findings, then runs the merge.
fn run_merge_text(args: Vec<String>) -> ExitCode {
    let format = match parse_format(&args) {
        Ok(format) => format,
        Err(code) => return code,
    };

    let raw = match read_stdin() {
        Ok(raw) => raw,
        Err(code) => return code,
    };

    let input: MergeTextInput = match serde_json::from_str(&raw) {
        Ok(input) => input,
        Err(e) => {
            eprintln!("error: invalid MergeTextInput JSON on stdin: {e}");
            return ExitCode::from(EXIT_INPUT_ERROR);
        }
    };

    // Parse each model's Markdown into findings, attributing by label.
    let merge_input = MergeInput {
        a: parse_findings(&input.text_a, &input.label_a),
        b: parse_findings(&input.text_b, &input.label_b),
        label_a: input.label_a.clone(),
        label_b: input.label_b.clone(),
    };

    emit(
        merge(&merge_input),
        &merge_input.label_a,
        &merge_input.label_b,
        format,
    )
}

/// Read all of stdin, mapping an I/O failure to the input-error exit code.
fn read_stdin() -> Result<String, ExitCode> {
    let mut raw = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut raw) {
        eprintln!("error: failed to read stdin: {e}");
        return Err(ExitCode::from(EXIT_INPUT_ERROR));
    }
    Ok(raw)
}

/// Serialize a [`MergeResult`] in the chosen format and write it to stdout.
fn emit(result: MergeResult, label_a: &str, label_b: &str, format: Format) -> ExitCode {
    let out = match format {
        Format::Json => match serde_json::to_string(&result) {
            Ok(out) => out,
            Err(e) => {
                eprintln!("error: failed to serialize MergeResult: {e}");
                return ExitCode::from(EXIT_INTERNAL_ERROR);
            }
        },
        // `debate_markdown` already ends with a newline; trim the trailing one
        // so the uniform `writeln!` below does not double it.
        Format::DebateMarkdown => {
            let md = debate_markdown(&result, label_a, label_b);
            md.strip_suffix('\n').map(str::to_string).unwrap_or(md)
        }
    };

    let mut stdout = io::stdout().lock();
    if writeln!(stdout, "{out}").is_err() || stdout.flush().is_err() {
        eprintln!("error: failed to write result to stdout");
        return ExitCode::from(EXIT_INTERNAL_ERROR);
    }

    ExitCode::SUCCESS
}

/// Parse the optional `--format json|debate-md` flag. Defaults to JSON. On a
/// bad flag, prints the diagnostic + usage and yields the usage exit code so
/// callers can `return` it directly.
fn parse_format(args: &[String]) -> Result<Format, ExitCode> {
    let mut format = Format::Json;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--format" => {
                let Some(value) = it.next() else {
                    return Err(usage_error("--format requires a value (json|debate-md)"));
                };
                format = parse_format_value(value)?;
            }
            other if other.starts_with("--format=") => {
                format = parse_format_value(&other["--format=".len()..])?;
            }
            other => return Err(usage_error(&format!("unexpected argument '{other}'"))),
        }
    }
    Ok(format)
}

fn parse_format_value(value: &str) -> Result<Format, ExitCode> {
    match value {
        "json" => Ok(Format::Json),
        "debate-md" => Ok(Format::DebateMarkdown),
        other => Err(usage_error(&format!(
            "unknown --format value '{other}' (expected json|debate-md)"
        ))),
    }
}

/// Print a usage diagnostic to stderr and return the usage exit code.
fn usage_error(msg: &str) -> ExitCode {
    eprintln!("error: {msg}\n\n{USAGE}");
    ExitCode::from(EXIT_USAGE_ERROR)
}
