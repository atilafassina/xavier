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
