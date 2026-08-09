<div align="center">

# mctx — Memory Context

**A token-optimized, seek-indexed memory format for AI agents — with a
beautiful desktop app, a terminal notepad, and an Android app to read & edit
it.**

`v2.1.5` · MIT License · [GitHub Releases](https://github.com/cyberhatc/mctx/releases) · [Install](#install)

</div>

---

## Why mctx?

When an AI agent needs to remember things across sessions, it shouldn't burn
tokens on verbose JSON or fragile free-text. `.mctx` files are:

- **Token-optimized** — a seek-indexed flat format with minimal punctuation.
  One section = one `%%@name tier v:N` block. No braces, no quoting noise.
- **Fast** — the `%%INDEX` block tells a reader exactly where each section
  lives, so an agent can `seek` straight to what it needs instead of parsing
  a whole file.
- **Human + AI readable** — the same file opens in a **desktop GUI** with a
  friendly Markdown view *and* a raw/JSON AI view, so you and your agents see
  the same text.
- **Zero dependencies** — the core library compiles with plain `rustc`. No
  `serde`, no `serde_json`, no build script.

## Screenshots

The desktop notepad (`mctx-gui`) shows **two views** of the same file.

| Human tab — rendered Markdown, easy on the eyes | AI tab — raw `.mctx` source + JSON breakdown |
|:---:|:---:|
| ![mctx-gui human view](images/gui-human-tab.png) | ![mctx-gui AI view](images/gui-ai-tab.png) |

---

## Components

| Component | What it is |
|---|---|
| **`src/mctx.rs`** | The format library: zero dependencies, `rustc`-only. Load the `%%INDEX`, seek to one section, write with a `v:` version bump, rebuild the index. |
| **`apps/mctx-gui`** → `mctx-gui` | Desktop notepad (egui). **Human** tab (Markdown) + **AI** tab (raw + JSON). Auto-reloads when the file changes on disk. Native open/save dialogs. |
| **`apps/mctx-notepad`** → `mctx` | Two-panel terminal editor (section list + body), plus a full **agent/script CLI** (`show`, `md`, `json`, `list`, `get`, `set`, `checkpoint`, `index`, `new`). |
| **`android/`** → `mctx-android.apk` | Storage Access Framework notepad for Android. |
| **`mctx/`** | Cargo wrapper so the single-file library is a normal crate dependency. |

---

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

- Every byte offset is zero-padded to 10 digits, so the index block's length
  never depends on its own values.
- **Tiers**: `!fixed` (identity, never auto-overwritten), `!durable` (current
  state, superseded in place), `!volatile` (checkpoint/scratch, safe to drop).
- **Bodies** are TOON-style tabular arrays or plain `key: value` lines —
  minimal punctuation, cheap for an LLM to read.

See `doc/mctx-spec.md` for the full rationale.

---

## Install

Prebuilt binaries for **Linux, Windows, macOS and FreeBSD** (plus a `.deb` and
an Android APK) are attached to each [GitHub Release](https://github.com/cyberhatc/mctx/releases).

- **One-liner (any OS)**:
  `curl -sSL https://raw.githubusercontent.com/cyberhatc/mctx/main/scripts/install.sh | bash`
- **Debian / Ubuntu**: `scripts/build-deb.sh` → `target/mctx_2.1.5_amd64.deb`,
  then `sudo apt install ./mctx_2.1.5_amd64.deb` (installs `mctx` *and*
  `mctx-gui`, registers the `application/x-mctx` MIME type so `.mctx` files
  open in the app from your file manager).
- **Homebrew (macOS/Linux)**: `brew install cyberhatc/mctx/mctx`
- **Windows**: `mctx-windows-x86_64.exe` and `mctx-gui-windows-x86_64.exe`
  from the release.
- **FreeBSD**: port skeleton in `pkg/freebsd/`.
- **Android**: side-load `mctx-android.apk` from the release, or use Termux:
  `pkg install rust && cargo install --path apps/mctx-notepad`.
- **As an agent skill**: `bash scripts/install-skill.sh` installs
  `skills/mctx` into `~/.config/opencode/skills/` and `~/.claude/skills/`.

---

## Usage

### Desktop app

```
mctx-gui [memory.mctx]   # open a file, or use Open… / Save As…
```

- **Human** tab: the memory rendered as readable Markdown.
- **AI** tab: the raw `.mctx` source plus a structured JSON breakdown.
- `Ctrl+S` save · `Ctrl+O` open · `Ctrl+Shift+S` save as · `Ctrl+R` reload.
- The app **watches the file and reloads automatically** when it changes on
  disk (e.g. an agent writes new memory while you're looking at it), so the
  view stays live.

### Terminal notepad

```
mctx [memory.mctx]      # default: ./memory.mctx (created if missing)
```

Keys: `Tab` switch panel · `a` add section · `c` checkpoint · `Enter` edit ·
`Ctrl+S` save · `Esc` back · `q` quit (safe while unsaved).

### Agent / script mode (what an AI uses)

```
mctx show FILE                  raw .mctx
mctx md FILE                    human-readable Markdown
mctx json FILE                  AI view: structured JSON (sections/tiers/v/offsets)
mctx list FILE                  index rows
mctx get FILE SECTION           one section's body
mctx set FILE SECTION TIER BODY write a section (bumps v:); BODY '-' = stdin
mctx checkpoint FILE BODY       write the !volatile checkpoint; '-' = stdin
mctx index FILE                 rebuild the %%INDEX after hand edits
mctx new FILE                   create a fresh file
```

Example:

```bash
printf 'next: fix the bug\n' | mctx checkpoint memory.mctx -
```

---

## Build & test

```
cargo build --release          # builds target/release/mctx
cargo test --release           # unit + library tests
cargo clippy --release --all-targets
rustc --edition 2021 -D warnings -O -o /tmp/mctx_test src/test_mctx.rs && /tmp/mctx_test
```

See `man/mctx.1` and `apps/mctx-notepad/src/main.rs` for details.

---

<div align="center">

Built for humans and AI agents to share the same memory.

[Report a bug](https://github.com/cyberhatc/mctx/issues) · MIT License

</div>
