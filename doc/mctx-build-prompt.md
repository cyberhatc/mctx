# BUILD PROMPT — Implement the `.mctx` Memory Context Format

Paste this whole file to a coding agent (Claude Code, Cursor, etc.) as the task
spec. It is self-contained — everything needed to build `.mctx` from scratch is
below, no other context required.

---

## What you're building

`.mctx` is a token-optimized file format for AI persistent memory — a coding
agent's long-term memory across sessions and context-window resets. It must be:

1. **Token-efficient to read** — tabular headers declared once, no repeated keys,
   minimal punctuation (inspired by TOON / YAML-style compactness, not JSON).
2. **Indexed for fast lookup** — a small header block mapping each section to its
   exact byte offset, so a reader can seek straight to one section without
   scanning or parsing the whole file.
3. **Memory-aware, not just data-compact** — every section is tagged with a
   durability tier so an agent knows what's permanent vs. what's safe to drop.

Build native libraries for **C**, **C++**, and **Rust**, plus a test/demo for each.

---

## 1. File format spec

```
#mctx v1.1 | updated:<ISO date>
%%INDEX
<name>:<tier>:v<N>:<byte-offset>
<name>:<tier>:v<N>:<byte-offset>
%%END-INDEX
%%@<name> <tier> v:<N>
<body>
%%END
%%@<name2> <tier> v:<N>
<body>
%%END
```

Rules:
- `<tier>` is exactly one of `!fixed`, `!durable`, `!volatile`.
  - `!fixed` — identity-level, permanent, never auto-overwritten.
  - `!durable` — current-state facts, updated in place when superseded.
  - `!volatile` — session/short-lived (task checkpoints, scratch logs); may
    carry an optional `ttl:<Nd>` for "safe to prune after N days."
- `<byte-offset>` in the index is the exact byte position of that section's
  `%%@name` marker line — must be byte-exact so a reader can `fseek`/`seek`
  directly there.
- `v:<N>` starts at 1, increments by 1 every time that section's body changes.
- Section bodies use tabular arrays for repeated records — declare the schema
  once, then one row per line, no repeated field names:
  ```
  projects[2]{id,title,status}:
    p1,"SmartTodo","in progress"
    p2,"Zoya","active"
  ```
  Use plain `key: value` lines for scalar/non-repeating facts.
- No braces, no trailing commas. Quote only strings containing a comma or colon.

## 2. The hard part — fixed-width offsets

The index block's own byte length depends on the offsets it contains (their
digit-width), but the offsets themselves depend on where the index block ends.
This is circular. **Solve it by zero-padding every offset to a fixed width**
(e.g. 10 digits) so the index block's length never changes based on the values
inside it — a dummy pass (offsets = 0, same padding) and the real pass are
always identical length, so "where does the body start" is computable in one
pass. Get this wrong and every seek silently lands a few bytes into the wrong
place — validate against it explicitly in tests (see §5).

## 3. Required API (same shape in all three languages)

- **Load index only** — parse just the `%%INDEX`...`%%END-INDEX` block. Must
  not read section bodies. This is the cheap, frequent operation.
- **Read section by name** — look up its offset in the loaded index, seek
  directly there, read until `%%END`, return the body. Must not read the rest
  of the file.
- **Write/update section** — given name + tier + new body: if the section
  exists, replace its body and increment `v:`; if not, append a new section.
  Then rebuild the index (offsets shift after any edit). Full-file rewrite is
  acceptable for this operation.
- **Rebuild index** — rescan the file for `%%@name` markers, regenerate the
  `%%INDEX` block with correct fixed-width offsets. Callable standalone (e.g.
  after an external tool hand-edited the file).
- **Checkpoint convenience** — a one-call helper equivalent to
  `write("checkpoint", "!volatile", body)`, for the "about to run out of
  context, save state now" pattern.

Language-specific shape:
- **C**: `mctx.h` / `mctx.c`. Plain structs (`MctxIndexEntry`, `MctxIndex`),
  functions take a `path` and output buffers, caller-managed memory.
- **C++**: header-only wrapper (`mctx.hpp`) around the C core (or a clean
  native reimplementation) — a `Store` class with `reload()`, `index()`,
  `read(name)`, `write(name, tier, body)`, `checkpoint(body)`, throwing on error.
- **Rust**: `mctx.rs`, no external crates — `Store` struct with equivalent
  methods returning `io::Result<T>`.

## 4. Deliverables checklist

- [ ] `mctx.h` + `mctx.c` — compiles clean with `gcc -Wall`
- [ ] `mctx.hpp` — compiles clean with `g++ -Wall -std=c++17`, links against the C core
- [ ] `mctx.rs` — compiles clean with `rustc`, no warnings, no external deps
- [ ] One test/demo file per language that: creates a fresh `.mctx` file,
  appends 2-3 sections across different tiers, reads one back via seek,
  updates a section and confirms `v:` incremented, and checkpoints.
- [ ] A short `mctx-agent-guide.md` documenting: the tier decision rule, the
  read/write protocol (index-first, then targeted seek), and the
  context-exhaustion checkpoint protocol (what fields a checkpoint should
  contain: `task`, `done`, `next`, `files_touched`, `blockers`).

## 5. Validation — do this before calling it done

For each language's test: after writing/updating sections, independently
verify every offset in the index is byte-exact — read the raw file, jump to
each stored offset, and confirm the bytes there are literally `%%@<name>`, not
`%%@` a few bytes off in either direction. This is the single most likely bug
(see §2) and it fails silently (reads still "work" by accident if the reader
skips to the next newline) unless you check byte-exactness directly.

## 6. Anti-patterns to avoid while building this

- Don't reach for JSON or add braces/quotes "for safety" — the whole point is
  token-minimal syntax a model reads cheaply.
- Don't make the index optional or best-effort — a reader must be able to
  trust it enough to seek blind, or the whole design point (skip full-file
  scans) is lost.
- Don't invent a 4th durability tier or extra metadata fields unless asked —
  keep the schema exactly as specified so files stay portable across the three
  implementations.
