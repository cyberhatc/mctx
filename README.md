# mctx — Memory Context format

A token-optimized, seek-indexed file format for **AI agent persistent memory**,
plus notepad apps — terminal and desktop GUI — to view and edit those files.

- **`src/mctx.rs`** — the format library: zero dependencies, compiles with
  plain `rustc`. Load only the `%%INDEX`, `seek` straight to one section,
  write/update with a `v:` version bump, rebuild the index. No JSON, no braces.
- **`apps/mctx-notepad`** — `mctx`, a two-panel terminal editor (section list
  + body editor, Ctrl+S to save, `c` for checkpoint). Use it like a notepad for
  your agent's memory file.
- **`apps/mctx-gui`** — `mctx-gui`, a desktop notepad with **two views**:
  a **Human** tab that renders the memory as readable Markdown, and an **AI**
  tab showing the raw `.mctx` source plus a structured JSON breakdown. Open a
  `.mctx` file and both you and your agents see the same text.
- **`mctx/`** — Cargo packaging wrapper so the single-file library can be used
  as a normal crate dependency.

## File format (v1.1)

```
#mctx v1.1 | updated:2026-08-08
%%INDEX
identity:!fixed:v1:0000000058
projects:!durable:v2:0000000134
%%END-INDEX
%%@identity !fixed v:1
user{alias,role}:
  "devil2","student/builder"
%%END
```

- Every byte offset is zero-padded to 10 digits so the index block's length
  never depends on its own values (that dependency would be circular).
- Tiers: `!fixed` (never auto-overwritten), `!durable` (superseded in place),
  `!volatile` (short-lived, checkpoint/scratch).
- Bodies are TOON-style tabular arrays or plain `key: value` lines — minimal
  punctuation, easy for an LLM to read cheaply.

## Build & test

```
cargo build --release          # builds target/release/mctx
cargo test --release           # unit + library tests
cargo clippy --release --all-targets
rustc --edition 2021 -D warnings -O -o /tmp/mctx_test src/test_mctx.rs && /tmp/mctx_test
```

## Install

Every install path is documented below; prebuilt binaries for Linux, Windows,
macOS and FreeBSD (plus a `.deb` and an Android APK) are attached to each
[GitHub Release](https://github.com/cyberhatc/mctx/releases).

- **One-liner (any OS)**: `curl -sSL https://raw.githubusercontent.com/cyberhatc/mctx/main/scripts/install.sh | bash`
- **Debian / Ubuntu**: `scripts/build-deb.sh` → `target/mctx_1.1.0_amd64.deb`,
  then `sudo apt install ./mctx_1.1.0_amd64.deb`
  (the package uses gzip-compressed archives, so any apt/dpkg version accepts
  it; it installs both `mctx` and `mctx-gui` and registers `.mctx` files so
  they open in the mctx app from your file manager)
- **Homebrew (macOS/Linux)**: `brew install cyberhatc/mctx/mctx`
- **Windows**: `mctx-windows-x86_64.exe` and `mctx-gui-windows-x86_64.exe`
  from the release
- **FreeBSD**: port skeleton in `pkg/freebsd/` (`make package`, `pkg add`);
  submit it to the ports tree for `pkg install mctx`
- **Android**: download `mctx-android.apk` from the release and side-load it,
  or use Termux: `pkg install rust && cargo install --path apps/mctx-notepad`
- **As an agent skill**: `bash scripts/install-skill.sh` installs
  `skills/mctx` into `~/.config/opencode/skills/` and `~/.claude/skills/`
  so agents know the `.mctx` read/write/checkpoint protocol globally.

## Usage

Terminal notepad:

```
mctx [memory.mctx]      # default: ./memory.mctx (created if missing)
```

Keys: `Tab` switch panel · `a` add section · `c` checkpoint · `Enter` edit ·
`Ctrl+S` save · `Esc` back · `q` quit (safe while unsaved).

Desktop notepad (human + AI views):

```
mctx-gui [memory.mctx]   # open a file, or use Open…/Save As…
```

`mctx-gui` opens `.mctx` files with a **Human** tab (readable Markdown) and
an **AI** tab (raw `.mctx` source + JSON structure). Ctrl+S saves, Ctrl+O
opens, Ctrl+Shift+S saves as. On Debian/Ubuntu the `.deb` wires up the
`application/x-mctx` MIME type, so double-clicking a `.mctx` file opens it in
the app like a notepad.

See `man/mctx.1` and `apps/mctx-notepad/src/main.rs` for details, and
`doc/mctx-spec.md` for the format rationale.
