# Contributing

Thanks for wanting to contribute to mctx!

## Reporting bugs

Open an issue on GitHub. Please include:

- the version you're using (`mctx --version`, or the package name)
- your platform (OS + architecture)
- the `.mctx` file (or a minimal repro) and the exact command you ran
- the output you got vs. the output you expected

## Building

```bash
cargo build --release          # binaries under target/release/
cargo test --release           # unit + library tests
cargo clippy --release --all-targets
```

## Code style

- The format library (`src/mctx.rs`) is intentionally **zero-dependency**.
  Keep it that way: no `serde`, no `serde_json`, no build scripts.
- Keep commits small and focused. Prefix commit messages with the area you
  touched (`doc:`, `gui:`, `cli:`, `android:`, `pkg:`).
- Run the tests and `clippy` before opening a PR.

## Format spec changes

The `.mctx` format is stable at **v1.1**. Any change to the on-disk format must
update `doc/mctx-spec.md`, bump the format version, and keep the library able to
read files written by older versions.
