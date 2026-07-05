//! CLI integration tests: the stdin -> JSON-stdout contract and exit codes.
//!
//! Cargo builds the binary before running these and exposes its path via the
//! `CARGO_BIN_EXE_<name>` environment variable.
//!
//! # Hermetic caching
//!
//! `merge`/`merge-text` cache exit-0 output on disk by default. To keep
//! `cargo test` from ever touching the user's real `~/.cache/xavier-tool` (and
//! to keep tests from serving each other stale results), every spawned binary
//! is pointed at an isolated `XAVIER_TOOL_CACHE_DIR` under the OS temp dir:
//!
//! - [`run`] gives each invocation its own unique temp cache dir, so the
//!   pre-existing contract tests are unaffected by — and never pollute — the
//!   cache. (Equivalently they could pass `--no-cache`; isolation keeps them
//!   closer to real invocations.)
//! - [`run_in`] pins an explicit cache dir (and optional extra env) so the
//!   cache-behavior tests can share state across calls to prove hit / miss /
//!   version-invalidation.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

/// Path to the freshly-built `xavier-tool` binary.
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_xavier-tool")
}

/// A process-unique temp directory path for an isolated cache (not created on
/// disk — the binary makes it on first write). Unique across calls and test
/// processes via PID + an atomic counter.
fn unique_cache_dir() -> std::path::PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "xavier-tool-test-cache-{}-{}",
        std::process::id(),
        n
    ))
}

/// Run `xavier-tool <args...>` feeding `stdin`, returning (exit code, stdout,
/// stderr). Each call gets a fresh, isolated cache dir so it neither reads nor
/// writes the real user cache and cannot be served another test's result.
fn run(args: &[&str], stdin: &str) -> (i32, String, String) {
    run_in(&unique_cache_dir(), &[], args, stdin)
}

/// Like [`run`], but pins `XAVIER_TOOL_CACHE_DIR` to `cache_dir` and applies
/// `extra_env` (e.g. `XAVIER_TOOL_CACHE_DEBUG`). Use this when a test needs
/// several invocations to share one cache (hit/miss/version tests).
fn run_in(
    cache_dir: &Path,
    extra_env: &[(&str, &str)],
    args: &[&str],
    stdin: &str,
) -> (i32, String, String) {
    let mut cmd = Command::new(bin());
    cmd.args(args)
        .env("XAVIER_TOOL_CACHE_DIR", cache_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in extra_env {
        cmd.env(k, v);
    }

    let mut child = cmd.spawn().expect("spawn xavier-tool");

    // Write the whole stdin, then drop the handle to close the pipe.
    //
    // A `BrokenPipe` here is expected, not a failure: for usage/input-error
    // args (e.g. an unknown `--format` value) the binary validates and exits
    // *before* draining stdin, so on Linux the child can close the read end
    // mid-write. These tests assert on the exit code and stderr — both still
    // captured by `wait_with_output` below — so a closed pipe is benign. Any
    // *other* write error is a real problem and still panics.
    {
        let mut stdin_pipe = child.stdin.take().expect("stdin piped");
        match stdin_pipe.write_all(stdin.as_bytes()) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
            Err(e) => panic!("write stdin: {e}"),
        }
    }

    let out = child.wait_with_output().expect("wait for xavier-tool");
    (
        out.status.code().expect("process exited via code"),
        String::from_utf8(out.stdout).expect("utf8 stdout"),
        String::from_utf8(out.stderr).expect("utf8 stderr"),
    )
}

#[test]
fn merge_reads_stdin_and_writes_json_stdout() {
    let input = r#"{
        "a": [{"severity":"high","reference":{"file":"src/main.rs","line":42},"description":"off by one"}],
        "b": [{"severity":"medium","reference":{"file":"src/main.rs","line":42},"description":"loop bound"}],
        "label_a": "GPT",
        "label_b": "Gemini"
    }"#;

    let (code, stdout, _stderr) = run(&["merge"], input);
    assert_eq!(code, 0, "successful merge exits 0");

    // stdout must be a single JSON object with the four stable buckets.
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is valid JSON");
    assert_eq!(parsed["consensus"].as_array().unwrap().len(), 1);
    assert!(parsed["blindspot"].as_array().unwrap().is_empty());
    assert!(parsed["dispute"].as_array().unwrap().is_empty());
    // `unmatched` is ALWAYS present (schema stability), even when empty.
    assert!(parsed["unmatched"].is_array());
}

#[test]
fn merge_debate_md_format_emits_markdown() {
    let input = r#"{
        "a": [{"severity":"high","reference":{"file":"src/main.rs","line":42},"description":"off by one","source":"GPT"}],
        "b": [{"severity":"high","reference":{"file":"src/main.rs","line":42},"description":"loop bound","source":"Gemini"}],
        "label_a": "GPT",
        "label_b": "Gemini"
    }"#;

    let (code, stdout, _) = run(&["merge", "--format", "debate-md"], input);
    assert_eq!(code, 0);
    // Markdown, not JSON — the three debate sections must be present.
    assert!(stdout.contains("## Consensus"));
    assert!(stdout.contains("## Disputes"));
    assert!(stdout.contains("## Blindspots"));
    assert!(stdout.contains("### [high] off by one"));
    assert!(!stdout.trim_start().starts_with('{'), "should not be JSON");
}

#[test]
fn merge_text_parses_markdown_and_merges() {
    // The binary parses each side's raw assistant text itself. Two paraphrases
    // of the same issue at the same file must collapse into one consensus.
    // Not a raw string: the JSON values embed `###` and backticks, so escape
    // the inner quotes and newlines explicitly.
    let input = "{\
        \"text_a\": \"### [high] The response omits the `id` field\\n**File**: `src/api.rs:42`\\n\",\
        \"text_b\": \"### [medium] `id` is absent from the response\\n**File**: `src/api.rs:42`\\n\",\
        \"label_a\": \"GPT\",\
        \"label_b\": \"Gemini\"\
    }";

    let (code, stdout, _stderr) = run(&["merge-text"], input);
    assert_eq!(code, 0, "successful merge-text exits 0");

    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is valid JSON");
    assert_eq!(
        parsed["consensus"].as_array().unwrap().len(),
        1,
        "paraphrased same-location findings collapse to one consensus"
    );
    assert!(parsed["unmatched"].is_array());
}

#[test]
fn merge_text_debate_md_has_unmatched_section() {
    // A reference-less finding becomes unmatched and must render under the new
    // `## Unmatched` section, while the three pilot-fish headings remain.
    let input = "{\
        \"text_a\": \"### [low] general advice with no file attached\\n\",\
        \"text_b\": \"\",\
        \"label_a\": \"GPT\",\
        \"label_b\": \"Gemini\"\
    }";

    let (code, stdout, _) = run(&["merge-text", "--format", "debate-md"], input);
    assert_eq!(code, 0);
    assert!(stdout.contains("## Consensus"));
    assert!(stdout.contains("## Disputes"));
    assert!(stdout.contains("## Blindspots"));
    assert!(stdout.contains("## Unmatched"));
    assert!(stdout.contains("### [low] general advice with no file attached"));
}

#[test]
fn merge_text_invalid_json_exits_input_error() {
    let (code, stdout, stderr) = run(&["merge-text"], "}{ not json");
    assert_eq!(code, 1, "bad stdin JSON -> exit 1");
    assert!(stdout.is_empty());
    assert!(stderr.contains("invalid MergeTextInput JSON"));
}

#[test]
fn merge_text_accepts_minimal_input_via_serde_defaults() {
    // Empty texts and missing labels should still succeed (serde defaults).
    let (code, stdout, _) = run(&["merge-text"], r#"{}"#);
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed["consensus"].as_array().unwrap().is_empty());
    assert!(parsed["unmatched"].is_array());
}

#[test]
fn merge_json_abi_is_pinned() {
    // Guard the original `merge` JSON ABI: the four-bucket schema and the
    // pre-parsed Finding input shape must remain stable across the version bump.
    let input = r#"{
        "a": [{"severity":"high","reference":{"file":"src/main.rs","line":42},"description":"x"}],
        "b": [{"severity":"high","reference":{"file":"src/main.rs","line":42},"description":"y"}],
        "label_a": "GPT",
        "label_b": "Gemini"
    }"#;
    let (code, stdout, _) = run(&["merge"], input);
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    // Exactly these four keys, all arrays.
    for key in ["consensus", "blindspot", "dispute", "unmatched"] {
        assert!(parsed[key].is_array(), "bucket `{key}` must be an array");
    }
    assert_eq!(parsed["consensus"].as_array().unwrap().len(), 1);
}

#[test]
fn merge_unknown_format_exits_usage_error() {
    let (code, _stdout, stderr) = run(&["merge", "--format", "yaml"], r#"{"a":[],"b":[]}"#);
    assert_eq!(code, 2, "bad --format value -> exit 2");
    assert!(stderr.contains("unknown --format value"));
}

#[test]
fn merge_accepts_minimal_input_via_serde_defaults() {
    // Missing labels and one empty side should still succeed (serde defaults).
    let (code, stdout, _) = run(&["merge"], r#"{"a":[],"b":[]}"#);
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed["unmatched"].is_array());
}

#[test]
fn invalid_json_exits_input_error() {
    let (code, stdout, stderr) = run(&["merge"], "not json at all");
    assert_eq!(code, 1, "bad stdin JSON -> exit 1");
    assert!(stdout.is_empty(), "no stdout on input error");
    assert!(stderr.contains("invalid MergeInput JSON"));
}

#[test]
fn unknown_subcommand_exits_usage_error() {
    let (code, _stdout, stderr) = run(&["frobnicate"], "");
    assert_eq!(code, 2, "unknown subcommand -> exit 2");
    assert!(stderr.contains("unknown subcommand"));
}

#[test]
fn missing_subcommand_exits_usage_error() {
    let (code, _stdout, stderr) = run(&[], "");
    assert_eq!(code, 2, "missing subcommand -> exit 2");
    assert!(stderr.contains("missing subcommand"));
}

#[test]
fn version_flag_exits_zero() {
    let (code, stdout, _) = run(&["--version"], "");
    assert_eq!(code, 0);
    assert!(stdout.contains("xavier-tool"));
}

// --- Caching ---------------------------------------------------------------
//
// These tests share an explicit cache dir across invocations (via `run_in`) so
// they can observe hit/miss/version-invalidation. The dir lives under the OS
// temp dir, so they never touch the user's real cache.

/// The binary's own version, parsed from `--version` (`xavier-tool <ver>`), so
/// the cache-layout tests survive a version bump without hardcoding.
fn tool_version() -> String {
    let (code, stdout, _) = run(&["--version"], "");
    assert_eq!(code, 0);
    stdout
        .trim()
        .rsplit(' ')
        .next()
        .expect("version token")
        .to_string()
}

const MERGE_INPUT: &str = r#"{
    "a": [{"severity":"high","reference":{"file":"src/main.rs","line":42},"description":"off by one"}],
    "b": [{"severity":"medium","reference":{"file":"src/main.rs","line":42},"description":"loop bound"}],
    "label_a": "GPT",
    "label_b": "Gemini"
}"#;

#[test]
fn cache_miss_then_hit_returns_identical_output() {
    let dir = unique_cache_dir();

    // First call: cold cache -> miss, computes, writes through.
    let (code1, out1, err1) = run_in(
        &dir,
        &[("XAVIER_TOOL_CACHE_DEBUG", "1")],
        &["merge"],
        MERGE_INPUT,
    );
    assert_eq!(code1, 0);
    assert!(err1.contains("cache: miss"), "first call misses: {err1:?}");

    // A cache file must now exist under the version-scoped subdir.
    let version_dir = dir.join(format!("v{}", tool_version()));
    let files: Vec<_> = std::fs::read_dir(&version_dir)
        .expect("version cache dir created")
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(
        files.len(),
        1,
        "exactly one cache entry, no temp files: {files:?}"
    );
    assert!(
        files[0].ends_with(".json"),
        "entry is <hash>.json: {files:?}"
    );

    // Second identical call: served from cache, byte-identical stdout.
    let (code2, out2, err2) = run_in(
        &dir,
        &[("XAVIER_TOOL_CACHE_DEBUG", "1")],
        &["merge"],
        MERGE_INPUT,
    );
    assert_eq!(code2, 0);
    assert!(err2.contains("cache: hit"), "second call hits: {err2:?}");
    assert_eq!(out1, out2, "a hit is byte-identical to a fresh compute");
    // stdout must remain pure JSON even with the debug marker on stderr.
    assert!(serde_json::from_str::<serde_json::Value>(&out2).is_ok());
}

#[test]
fn cache_hit_matches_no_cache_output_byte_for_byte() {
    let dir = unique_cache_dir();

    // Bypass path: never touches the cache.
    let (c_nc, out_nc, err_nc) = run_in(
        &dir,
        &[("XAVIER_TOOL_CACHE_DEBUG", "1")],
        &["merge", "--no-cache"],
        MERGE_INPUT,
    );
    assert_eq!(c_nc, 0);
    assert!(err_nc.contains("cache: miss"), "no-cache reports miss");
    // --no-cache must not write anything.
    assert!(
        !dir.exists()
            || std::fs::read_dir(&dir)
                .map(|mut d| d.next().is_none())
                .unwrap_or(true),
        "--no-cache leaves the cache empty"
    );

    // Populate then read back through the cache.
    let (_, _, _) = run_in(&dir, &[], &["merge"], MERGE_INPUT);
    let (c_hit, out_hit, _) = run_in(&dir, &[], &["merge"], MERGE_INPUT);
    assert_eq!(c_hit, 0);
    assert_eq!(
        out_nc, out_hit,
        "cached output == uncached output, byte for byte"
    );
}

#[test]
fn cache_keyed_on_format_no_cross_serving() {
    let dir = unique_cache_dir();

    // Populate the JSON entry.
    let (_, json_out, _) = run_in(&dir, &[], &["merge"], MERGE_INPUT);
    assert!(
        json_out.trim_start().starts_with('{'),
        "json branch is JSON"
    );

    // Same input, different --format: must NOT be served the JSON entry.
    let (code, md_out, err) = run_in(
        &dir,
        &[("XAVIER_TOOL_CACHE_DEBUG", "1")],
        &["merge", "--format", "debate-md"],
        MERGE_INPUT,
    );
    assert_eq!(code, 0);
    assert!(
        err.contains("cache: miss"),
        "different format is a distinct key"
    );
    assert!(
        md_out.contains("## Consensus"),
        "markdown rendering, not cached JSON"
    );
}

#[test]
fn cache_version_invalidation_old_entry_not_served() {
    // The binary's version is fixed at compile time, so we simulate a version
    // change by planting a poisoned entry under a *different* version directory
    // and proving the running binary (current version) ignores it and computes
    // fresh — i.e. a version bump misses the prior version's entries.
    let dir = unique_cache_dir();
    let cur = tool_version();

    // (a) Plant a sentinel under a stale version dir. If the binary keyed
    //     without the version, it might serve this; it must not.
    let stale_dir = dir.join("v0.0.0-stale");
    std::fs::create_dir_all(&stale_dir).unwrap();
    // The cache uses content-addressed file names, so the exact name doesn't
    // matter for proving the *directory* (version) gates reads: any file in the
    // stale dir is unreachable to the current-version binary.
    std::fs::write(stale_dir.join("0000000000000000.json"), b"POISON\n").unwrap();

    // First real call: current-version dir is empty -> miss + compute.
    let (code1, out1, err1) = run_in(
        &dir,
        &[("XAVIER_TOOL_CACHE_DEBUG", "1")],
        &["merge"],
        MERGE_INPUT,
    );
    assert_eq!(code1, 0);
    assert!(
        err1.contains("cache: miss"),
        "stale-version entry is not served"
    );
    assert!(
        !out1.contains("POISON"),
        "did not serve the poisoned stale entry"
    );
    assert!(serde_json::from_str::<serde_json::Value>(&out1).is_ok());

    // The fresh entry landed under the *current* version dir, not the stale one.
    let cur_dir = dir.join(format!("v{cur}"));
    assert!(
        cur_dir.join_exists_one_json(),
        "entry written under current version dir"
    );
    // The stale dir is untouched (still just our poison file).
    let stale_files: Vec<_> = std::fs::read_dir(&stale_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(stale_files, vec!["0000000000000000.json".to_string()]);

    // Second call under the current version: now a hit (proves the current dir
    // is the live one).
    let (code2, _out2, err2) = run_in(
        &dir,
        &[("XAVIER_TOOL_CACHE_DEBUG", "1")],
        &["merge"],
        MERGE_INPUT,
    );
    assert_eq!(code2, 0);
    assert!(
        err2.contains("cache: hit"),
        "current-version entry is served"
    );
}

/// Tiny extension trait to keep the version test readable.
trait DirHasOneJson {
    fn join_exists_one_json(&self) -> bool;
}
impl DirHasOneJson for std::path::PathBuf {
    fn join_exists_one_json(&self) -> bool {
        std::fs::read_dir(self)
            .map(|d| {
                let names: Vec<_> = d
                    .map(|e| e.unwrap().file_name().into_string().unwrap())
                    .collect();
                names.len() == 1 && names[0].ends_with(".json")
            })
            .unwrap_or(false)
    }
}

#[test]
fn invalid_input_is_not_cached() {
    let dir = unique_cache_dir();
    // A parse error (exit 1) must never populate the cache.
    let (code, _out, _err) = run_in(&dir, &[], &["merge"], "not json at all");
    assert_eq!(code, 1);
    assert!(
        !dir.exists()
            || std::fs::read_dir(&dir)
                .map(|mut d| d.next().is_none())
                .unwrap_or(true),
        "errors are never cached"
    );
}
