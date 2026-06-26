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
//! # Caching
//!
//! Both `merge` and `merge-text` are pure functions of their stdin and the
//! binary version, so their (exit-0) output is memoized on disk by
//! [`xavier_core::cache`]. The CLI consults the cache *before* computing and
//! writes through on a miss; a hit replays byte-identical stdout, so caching is
//! invisible to the JSON ABI. The cache is keyed on the subcommand **and the
//! chosen `--format`** (so the JSON and Markdown renderings of one input never
//! collide) plus the raw input bytes and the version.
//!
//! - Base dir: `$XAVIER_TOOL_CACHE_DIR` if set, else `$XDG_CACHE_HOME/xavier-tool`,
//!   else `~/.cache/xavier-tool`. Override it (e.g. to an isolated temp dir) for
//!   tests and the review pipeline.
//! - `--no-cache` bypasses the cache entirely (no read, no write).
//! - `XAVIER_TOOL_CACHE_DEBUG=1` prints `cache: hit` / `cache: miss` to **stderr**
//!   (never stdout) so a cache hit is observable without polluting the ABI.
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
    debate_markdown, merge, parse_findings, Cache, MergeInput, MergeResult, MergeTextInput,
};

/// Debug env var: when set to a non-empty value, the cache outcome
/// (`cache: hit` / `cache: miss`) is printed to stderr.
const CACHE_DEBUG_ENV: &str = "XAVIER_TOOL_CACHE_DEBUG";

/// Output format for the `merge` subcommand.
#[derive(Clone, Copy)]
enum Format {
    Json,
    DebateMarkdown,
}

impl Format {
    /// A short, stable tag mixed into the cache key so the JSON and Markdown
    /// renderings of the same input never collide on one cache entry.
    fn key_tag(self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::DebateMarkdown => "debate-md",
        }
    }
}

/// Parsed CLI options shared by both subcommands: the output format and whether
/// the cache is bypassed.
struct Options {
    format: Format,
    no_cache: bool,
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
    xavier-tool merge      [--format json|debate-md] [--no-cache]
    xavier-tool merge-text [--format json|debate-md] [--no-cache]

SUBCOMMANDS:
    merge       Read MergeInput JSON (pre-parsed findings) on stdin, write the
                merge result on stdout.
    merge-text  Read MergeTextInput JSON (each model's raw assistant text) on
                stdin; parse findings in the binary, then merge.

                --format json       (default) emit MergeResult JSON.
                --format debate-md  emit Consensus/Disputes/Blindspots/Unmatched
                                    Markdown.

FLAGS:
    --no-cache       Bypass the on-disk result cache (no read, no write).
    -h, --help       Print this help and exit.
    -V, --version    Print version and exit.

CACHING:
    merge/merge-text are pure functions of their input + binary version, so
    exit-0 output is memoized on disk and replayed byte-for-byte on a repeat.
    XAVIER_TOOL_CACHE_DIR    Override the cache base dir (default:
                             $XDG_CACHE_HOME/xavier-tool or ~/.cache/xavier-tool).
    XAVIER_TOOL_CACHE_DEBUG  When non-empty, print 'cache: hit|miss' to stderr.

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
    let opts = match parse_options(&args) {
        Ok(opts) => opts,
        Err(code) => return code,
    };

    let raw = match read_stdin() {
        Ok(raw) => raw,
        Err(code) => return code,
    };

    // Read-through: identical input + version + format replays cached stdout.
    if let Some(code) = serve_from_cache("merge", &raw, &opts) {
        return code;
    }

    let input: MergeInput = match serde_json::from_str(&raw) {
        Ok(input) => input,
        Err(e) => {
            eprintln!("error: invalid MergeInput JSON on stdin: {e}");
            return ExitCode::from(EXIT_INPUT_ERROR);
        }
    };

    emit_and_cache(
        merge(&input),
        &input.label_a,
        &input.label_b,
        "merge",
        &raw,
        &opts,
    )
}

/// `merge-text` subcommand: stdin [`MergeTextInput`] JSON -> stdout. The binary
/// parses each side's raw assistant text into findings, then runs the merge.
fn run_merge_text(args: Vec<String>) -> ExitCode {
    let opts = match parse_options(&args) {
        Ok(opts) => opts,
        Err(code) => return code,
    };

    let raw = match read_stdin() {
        Ok(raw) => raw,
        Err(code) => return code,
    };

    // Read-through before any parsing: a hit means we already merged this exact
    // input on a prior run.
    if let Some(code) = serve_from_cache("merge-text", &raw, &opts) {
        return code;
    }

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

    emit_and_cache(
        merge(&merge_input),
        &merge_input.label_a,
        &merge_input.label_b,
        "merge-text",
        &raw,
        &opts,
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

/// The cache namespace for `(subcommand, format)`: e.g. `merge:json`. Keeping
/// the format in the namespace means the JSON and Markdown renderings of one
/// input get distinct cache entries instead of clobbering each other.
fn cache_namespace(subcommand: &str, opts: &Options) -> String {
    format!("{subcommand}:{}", opts.format.key_tag())
}

/// Read-through. On a cache hit, replay the cached stdout bytes verbatim and
/// return the resulting [`ExitCode`]; on a miss (or when caching is disabled, or
/// on any I/O hiccup), return `None` so the caller computes. Honors the debug
/// marker. Caching disabled => always a "miss" with no lookup.
fn serve_from_cache(subcommand: &str, raw: &str, opts: &Options) -> Option<ExitCode> {
    if opts.no_cache {
        cache_debug("miss");
        return None;
    }
    let cache = Cache::new(env!("CARGO_PKG_VERSION"));
    let namespace = cache_namespace(subcommand, opts);
    match cache.lookup(&namespace, raw.as_bytes()) {
        Some(bytes) => {
            cache_debug("hit");
            Some(write_stdout_bytes(&bytes))
        }
        None => {
            cache_debug("miss");
            None
        }
    }
}

/// Render a [`MergeResult`] in the chosen format, write it to stdout, and —
/// on success and unless `--no-cache` — write it through to the cache so the
/// next identical call is a hit. Only exit-0 results are cached.
fn emit_and_cache(
    result: MergeResult,
    label_a: &str,
    label_b: &str,
    subcommand: &str,
    raw: &str,
    opts: &Options,
) -> ExitCode {
    // The exact bytes that go to stdout are the exact bytes we cache, so a hit
    // is byte-identical to a fresh compute.
    let bytes = match render_output(&result, label_a, label_b, opts.format) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };

    let code = write_stdout_bytes(&bytes);
    if code == ExitCode::SUCCESS && !opts.no_cache {
        // Best-effort write-through; a failed cache write never fails the call.
        let cache = Cache::new(env!("CARGO_PKG_VERSION"));
        let namespace = cache_namespace(subcommand, opts);
        cache.store(&namespace, raw.as_bytes(), &bytes);
    }
    code
}

/// Serialize a [`MergeResult`] in the chosen format into the exact byte buffer
/// (including the trailing newline) that should be written to stdout.
fn render_output(
    result: &MergeResult,
    label_a: &str,
    label_b: &str,
    format: Format,
) -> Result<Vec<u8>, ExitCode> {
    let mut body = match format {
        Format::Json => match serde_json::to_string(result) {
            Ok(out) => out,
            Err(e) => {
                eprintln!("error: failed to serialize MergeResult: {e}");
                return Err(ExitCode::from(EXIT_INTERNAL_ERROR));
            }
        },
        // `debate_markdown` already ends with a newline; trim the trailing one
        // so the uniform newline appended below does not double it.
        Format::DebateMarkdown => {
            let md = debate_markdown(result, label_a, label_b);
            md.strip_suffix('\n').map(str::to_string).unwrap_or(md)
        }
    };
    body.push('\n');
    Ok(body.into_bytes())
}

/// Write a fully-rendered output buffer to stdout, mapping a write/flush failure
/// to the internal-error exit code.
fn write_stdout_bytes(bytes: &[u8]) -> ExitCode {
    let mut stdout = io::stdout().lock();
    if stdout.write_all(bytes).is_err() || stdout.flush().is_err() {
        eprintln!("error: failed to write result to stdout");
        return ExitCode::from(EXIT_INTERNAL_ERROR);
    }
    ExitCode::SUCCESS
}

/// Emit a `cache: <outcome>` line to stderr when [`CACHE_DEBUG_ENV`] is set to a
/// non-empty value. Goes to stderr only — stdout stays a pure JSON/Markdown ABI.
fn cache_debug(outcome: &str) {
    if std::env::var(CACHE_DEBUG_ENV)
        .ok()
        .is_some_and(|v| !v.is_empty())
    {
        eprintln!("cache: {outcome}");
    }
}

/// Parse the optional `--format json|debate-md` and `--no-cache` flags.
/// Defaults to JSON with caching enabled. On a bad flag, prints the diagnostic +
/// usage and yields the usage exit code so callers can `return` it directly.
fn parse_options(args: &[String]) -> Result<Options, ExitCode> {
    let mut format = Format::Json;
    let mut no_cache = false;
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
            "--no-cache" => no_cache = true,
            other => return Err(usage_error(&format!("unexpected argument '{other}'"))),
        }
    }
    Ok(Options { format, no_cache })
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
