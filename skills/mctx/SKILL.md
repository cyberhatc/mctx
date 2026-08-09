---
name: mctx
description: Use when reading, writing, or maintaining .mctx files — Memory Context files (persistent AI agent memory), the mctx notepad app, memory checkpoints, durability tiers (!fixed/!durable/!volatile), %%INDEX byte offsets, or anything tagged .mctx. Front-load the tier decision rule, index-first read protocol, and the context-exhaustion checkpoint fields (task/done/next/files_touched/blockers).
---

# .mctx — Memory Context format

`.mctx` is a token-optimized, seek-indexed file format for **persistent AI
agent memory**. An agent keeps one `.mctx` file and treats it as long-term
memory that survives context-window resets. The format is designed to be
cheap to read (index first, then seek) and cheap to write (edit one section,
bump a version counter).

Source of truth: `doc/mctx-build-prompt.md` (spec v1.1) and
`apps/mctx-notepad/` (the notepad). This skill is the operating protocol.

## 1. File anatomy

```
#mctx v1.1 | updated:2026-08-08
%%INDEX
identity:!fixed:v1:0000000058
projects:!durable:v2:0000000134
checkpoint:!volatile:v2:0000000186
%%END-INDEX
%%@identity !fixed v:1
user{alias,role}:
  "devil2","student/builder"
%%END
%%@projects !durable v:2
projects[2]{id,title,status}:
  p1,"SmartTodo","shipped"
  p2,"Zoya","active"
%%END
```

- `<byte-offset>` in the index is the exact byte position of that section's
  `%%@name` line — offsets are zero-padded to 10 digits. Use it to seek
  straight to a section, never scan the whole file.
- `v:<N>` starts at 1 and increments by 1 every time a section body is edited.
- Bodies are TOON-style tabular arrays — schema declared once as
  `name[count]{f1,f2}:`, then one flat, comma-separated row per line — or
  plain `key: value` lines for scalar facts. No braces, no trailing commas,
  quote only strings containing a comma or colon.

## 2. Durability tiers — route every fact correctly

| Tier | Use for | Agent behavior |
|---|---|---|
| `!fixed` | Identity: project name, stack, hard constraints | Never auto-overwrite. Change only on explicit user correction. |
| `!durable` | Current-state: open tasks, decisions, file map | Update in place when superseded; bump `v:`. |
| `!volatile` | Short-lived: checkpoints, scratch, session state | Safe to drop on cleanup; never treated as ground truth on resume. |

**Decision rule:** if a fact would still be true in a month without anyone
touching it, it's `!fixed` or `!durable`. If it's only useful for *this* task
or *this* session, it's `!volatile`.

## 3. Read protocol

1. Load the `%%INDEX` only (a handful of lines) to see sections, tiers,
   versions, and byte offsets.
2. Seek directly to the one section you need and read until `%%END`. Read
   only what you need — never the whole file.
3. On resume after a context reset: read `@checkpoint` **first**, before
   anything else, and continue from its `next:` field.

## 4. Write protocol

1. Reduce new information to the fewest atomic rows — one fact per row, no
   narration.
2. Pick the tier by the rule in §2, not by convenience.
3. Section exists → update it in place, bump `v:`, don't touch other sections.
4. New fact → append a new `%%@name <tier> v:1 ... %%END` block, then rebuild
   the index (offsets shift after any edit).
5. Keep field headers (`name{a,b,c}:`) stable once declared.
6. Never invent fields not in a section's declared header; never duplicate a
   fact across two tiers.

## 5. Context-exhaustion checkpoint protocol

When about to run out of context **mid-task**, stop at a clean point and
write/overwrite a `!volatile` `checkpoint` section containing at minimum:

- `task:` — one line, what you were doing
- `done:` — what's completed and verified
- `next:` — the very next concrete step, specific enough to resume cold
- `files_touched:` — paths you edited or need to revisit
- `blockers:` — anything unresolved

Overwrite the previous checkpoint rather than accumulating history. Read it
first on the next session and resume from `next:`.

## 6. Tools

- **Terminal notepad** (`mctx [file.mctx]`): two-panel editor. `a` add
  section, `c` checkpoint, `Enter` edit, `Ctrl+S` save (bumps `v:`), `q` quit.
- **Desktop notepad** (`mctx-gui [file.mctx]`): two views of the same buffer —
  a **Human** tab rendering the memory as readable Markdown, and an **AI** tab
  with the raw `.mctx` source plus a structured JSON breakdown (sections,
  tiers, versions, byte offsets, bodies). Ctrl+S save, Ctrl+O open,
  Ctrl+Shift+S save as. On Debian/Ubuntu the `.deb` registers the
  `application/x-mctx` MIME type so `.mctx` files open in the app from the
  file manager.
- **Library** (`src/mctx.rs`, or the `mctx` crate): `Store::open(path)`,
  `reload()` (index only), `read(name)` (seek-read), `write(name, tier, body)`
  (update-or-append + reindex), `checkpoint(body)`, `rebuild_index()`,
  `parse_content(str)` (in-memory parse for live previews),
  `render_markdown(str)` (lossless human view), `make_header()`.
- Install: `scripts/install.sh` one-liner, `.deb` for Debian/Ubuntu,
  Homebrew `cyberhatc/mctx/mctx`, FreeBSD port in `pkg/freebsd/`, Windows
  `mctx-*.exe`, Android APK + Termux.

## 7. Anti-patterns

- Don't tag everything `!durable` "to be safe" — volatile clutter defeats tiering.
- Don't read the whole file when you only need one section.
- Don't hand-edit byte offsets — always rebuild the index through the library.
- Don't skip the checkpoint because "I'll probably finish in time."
- Don't let two sections state the same fact — fix the stale one.
