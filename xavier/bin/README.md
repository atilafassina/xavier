# Bundled native binaries

This directory holds prebuilt `xavier-tool` binaries, one per platform triple:

```
bin/
  <target-triple>/
    xavier-tool
```

For example `bin/aarch64-apple-darwin/xavier-tool` or
`bin/x86_64-unknown-linux-gnu/xavier-tool`.

## Contract

- Users **never compile anything**. These binaries are built in CI by the
  release workflow (build-time Rust toolchain only) and shipped inside the
  release tarball.
- `install.sh` detects the host triple, and if a matching
  `bin/<triple>/xavier-tool` exists it `chmod +x`'s it. If no triple matches,
  the installer **no-ops** — it never writes a stub. Skills then fall back to
  the pure-shell `parse.sh` merge.
- The source for these binaries is the Rust workspace in the top-level `tool/`
  directory.

The release workflow cross-compiles and ships one binary per supported triple:

- `aarch64-apple-darwin` (macOS Apple Silicon)
- `x86_64-apple-darwin` (macOS Intel)
- `x86_64-unknown-linux-gnu` (Linux x86_64)
- `aarch64-unknown-linux-gnu` (Linux aarch64)

This set is kept in lockstep across `.github/workflows/release.yml` (the build
matrix), `install.sh` `detect_host_triple()` (install-time selection),
`deps/multi-model-dispatch/merge.sh` `resolve_tool()` (runtime selection), and
`validate-install-triples.sh` (the offline guard).

This directory is intentionally empty in the source tree except for this
README; binaries are populated only in release tarballs.
