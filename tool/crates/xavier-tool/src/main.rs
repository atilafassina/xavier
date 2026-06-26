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
//! - `merge [--format json|debate-md]` — read a [`xavier_core::MergeInput`] on
//!   stdin and write the result on stdout. Phase 1 logic is a trivial
//!   exact-match merge. The default `--format json` emits a
//!   [`xavier_core::MergeResult`] (the canonical ABI); `--format debate-md`
//!   emits the equivalent Consensus/Disputes/Blindspots Markdown that mirrors
//!   `parse.sh merge`. Markdown rendering is mechanical presentation only and
//!   does not cross the determinism boundary.
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

use xavier_core::{debate_markdown, merge, MergeInput};

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
    xavier-tool merge [--format json|debate-md]

SUBCOMMANDS:
    merge       Read MergeInput JSON on stdin, write the merge result on stdout.
                --format json       (default) emit MergeResult JSON.
                --format debate-md  emit Consensus/Disputes/Blindspots Markdown.

FLAGS:
    -h, --help       Print this help and exit.
    -V, --version    Print version and exit.

The merge subcommand reads JSON on stdin and writes to stdout.
Exit codes: 0 ok, 1 input error, 2 usage error, 3 internal error.";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);

    let Some(cmd) = args.next() else {
        eprintln!("error: missing subcommand\n\n{USAGE}");
        return ExitCode::from(EXIT_USAGE_ERROR);
    };

    match cmd.as_str() {
        "merge" => run_merge(args.collect()),
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

/// `merge` subcommand: stdin JSON -> stdout (JSON or Markdown).
fn run_merge(args: Vec<String>) -> ExitCode {
    let format = match parse_format(&args) {
        Ok(format) => format,
        Err(msg) => {
            eprintln!("error: {msg}\n\n{USAGE}");
            return ExitCode::from(EXIT_USAGE_ERROR);
        }
    };

    let mut raw = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut raw) {
        eprintln!("error: failed to read stdin: {e}");
        return ExitCode::from(EXIT_INPUT_ERROR);
    }

    let input: MergeInput = match serde_json::from_str(&raw) {
        Ok(input) => input,
        Err(e) => {
            eprintln!("error: invalid MergeInput JSON on stdin: {e}");
            return ExitCode::from(EXIT_INPUT_ERROR);
        }
    };

    let result = merge(&input);

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
            let md = debate_markdown(&result, &input.label_a, &input.label_b);
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

/// Parse the optional `--format json|debate-md` flag. Defaults to JSON.
fn parse_format(args: &[String]) -> Result<Format, String> {
    let mut format = Format::Json;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--format" => {
                let value = it
                    .next()
                    .ok_or_else(|| "--format requires a value (json|debate-md)".to_string())?;
                format = parse_format_value(value)?;
            }
            other if other.starts_with("--format=") => {
                format = parse_format_value(&other["--format=".len()..])?;
            }
            other => return Err(format!("unexpected argument '{other}'")),
        }
    }
    Ok(format)
}

fn parse_format_value(value: &str) -> Result<Format, String> {
    match value {
        "json" => Ok(Format::Json),
        "debate-md" => Ok(Format::DebateMarkdown),
        other => Err(format!(
            "unknown --format value '{other}' (expected json|debate-md)"
        )),
    }
}
