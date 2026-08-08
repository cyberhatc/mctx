# `.mctx` Agent Operating Guide
### How a coding agent should use a `.mctx` file as its persistent memory

Paste this whole file as a system prompt / project instruction for any coding agent
(Claude Code, Cursor, an autonomous dev-agent, etc.) that should treat one `.mctx`
file as its long-term memory across sessions and context-window resets.

---

## 1. What this file is for

`.mctx` is your persistent memory. It survives after your context window is cleared
or you run out of tokens mid-task. Three rules govern everything below:

1. **Don't reload the whole file to check on one thing.** Read the `%%INDEX` block
   first (a few lines), then seek directly to the one section you need.
2. **Every fact you save gets a durability tag**, not just dumped in. Tag wrong and
   you'll either lose something important or clutter memory with stale junk.
3. **Before you run out of context, checkpoint.** Don't let unsaved task state die
   with the context window — that's the whole point of this file existing.

---

## 2. File anatomy (what you'll see)

```
#mctx v1.1 | updated:<date>
%%INDEX
<name>:<tier>:v<N>:<byte-offset>
...
%%END-INDEX
%%@<name> <tier> v:<N>
<body — TOON-style tabular rows or plain key:value lines>
%%END
```

- `<tier>` is one of `!fixed`, `!durable`, `!volatile` — see §3.
- `<byte-offset>` in the index points exactly at that section's `%%@name` marker.
  Use it to seek directly there instead of scanning the file top to bottom.
- `v:<N>` increments every time that section's body is edited — you can tell what
  changed since you last looked without re-reading everything.

---

## 3. Durability tiers — route every fact correctly

| Tier | Use for | Example | Agent behavior |
|---|---|---|---|
| `!fixed` | Identity-level facts that don't change | project name, tech stack, user's stated constraints | Never auto-overwrite. Only change on explicit user correction. |
| `!durable` | Current-state facts that persist until superseded | open tasks, architectural decisions, known bugs, file map | Update in place when new info supersedes old. Bump `v:`. |
| `!volatile` | Short-lived / session-scoped state | task checkpoints, scratch notes, "what I was doing when I ran out of tokens" | Safe to summarize or drop after its `ttl`, or on explicit cleanup. Never load by default unless resuming a session. |

**Decision rule:** if you'd still want this fact true in a month without anyone
touching it, it's `!durable` or `!fixed`. If it's only useful to finish *this* task
or resume *this* session, it's `!volatile`.

---

## 4. Read protocol

1. Parse `%%INDEX` only (cheap — a handful of lines) to see what sections exist,
   their tiers, and their offsets.
2. Seek directly to the offset of the section(s) you actually need. Do not read
   sections you don't need for the current step.
3. If resuming after a context reset: **always read `@checkpoint` first** (if it
   exists) before doing anything else — it tells you where you left off.
4. Never treat `!volatile` content as ground truth for anything beyond resuming
   the immediate task — it can be stale or already superseded by `!durable` facts.

## 5. Write protocol

1. Convert new information into the smallest number of atomic rows/lines needed.
   One fact per row. Don't narrate — write data.
2. Pick the tier by the rule in §3, not by convenience.
3. If a section already exists: update it in place, bump `v:`, do **not** touch
   other sections.
4. If it's new: append a new `%%@name <tier> v:1 ... %%END` block.
5. After any write, the index must be rebuilt (offsets shift). Use the library —
   don't hand-edit byte offsets yourself.
6. Keep field headers (`name{a,b,c}:`) stable once declared — if the schema of a
   recurring record type needs a new field, add it to the header and backfill
   existing rows, don't create a parallel section with a different shape.

## 6. Context-exhaustion protocol (the important one)

When you estimate you're close to running out of tokens **mid-task**:

1. Stop what you're doing at a clean point (don't cut off mid-edit if avoidable).
2. Write a `!volatile` `@checkpoint` section containing, at minimum:
   - `task:` — one line, what you were doing
   - `done:` — what's already completed and verified
   - `next:` — the very next concrete step, specific enough to resume cold
   - `files_touched:` — paths you edited or need to revisit
   - `blockers:` — anything unresolved (failing test, missing info, etc.)
3. Overwrite the previous `@checkpoint` rather than accumulating a history —
   checkpoints represent "now," not a log. (Use `@log` — tagged `!volatile` with
   a `ttl` — if you actually want a running history instead.)
4. On the next session, read `@checkpoint` before anything else and resume from
   `next:`.

## 7. Section naming conventions

Use these consistently so the index stays predictable across projects:

- `@identity` `!fixed` — project name, stack, hard constraints
- `@decisions` `!durable` — architectural choices and why, so you don't re-litigate them
- `@tasks` `!durable` — open/in-progress work items
- `@checkpoint` `!volatile` — current resume-point (see §6)
- `@log` `!volatile` — rolling short-term history, prune by `ttl`
- `@errors` `!volatile` — recent failures worth not repeating, prune aggressively

## 8. Using the library instead of hand-editing

Don't string-manipulate the file yourself in ad hoc code — use the provided
`mctx` library (C / C++ / Rust) so index offsets stay correct:

- `mctx_load_index` / `Store::index()` — cheap read of just the index
- `mctx_read_section` / `Store::read(name)` — seek-and-read one section
- `mctx_write_section` / `Store::write(name, tier, body)` — update-or-create, bumps `v:`, rebuilds index
- `Store::checkpoint(body)` (C++/Rust convenience) — the §6 pattern in one call

If you're wiring this into an agent framework, call `write()`/`checkpoint()` as a
tool the agent can invoke directly — don't route memory writes through free-form
file edits, or the index will drift out of sync with the content.

---

## 9. Anti-patterns — don't do these

- Don't tag everything `!durable` "to be safe" — volatile clutter defeats the
  point of tiering and bloats what gets read on every resume.
- Don't read the whole file when you only need one section — that's the exact
  cost this format exists to avoid.
- Don't let two sections describe the same fact — if `@decisions` says X and
  `@tasks` implies not-X, one of them is stale; fix it, don't add a third opinion.
- Don't skip the checkpoint because "I'll probably finish in time." Write it
  early and overwrite it as you progress — it costs almost nothing and the
  failure mode without it is silent, total task-state loss.
