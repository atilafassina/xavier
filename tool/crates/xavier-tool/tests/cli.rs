//! CLI integration tests: the stdin -> JSON-stdout contract and exit codes.
//!
//! Cargo builds the binary before running these and exposes its path via the
//! `CARGO_BIN_EXE_<name>` environment variable.

use std::io::Write;
use std::process::{Command, Stdio};

/// Path to the freshly-built `xavier-tool` binary.
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_xavier-tool")
}

/// Run `xavier-tool <args...>` feeding `stdin`, returning (exit code, stdout, stderr).
fn run(args: &[&str], stdin: &str) -> (i32, String, String) {
    let mut child = Command::new(bin())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn xavier-tool");

    child
        .stdin
        .take()
        .expect("stdin piped")
        .write_all(stdin.as_bytes())
        .expect("write stdin");

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
