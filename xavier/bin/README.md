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

Phase 1 ships a single triple (the maintainer's release host). The full
cross-compiled matrix is a later phase.

This directory is intentionally empty in the source tree except for this
README; binaries are populated only in release tarballs.
