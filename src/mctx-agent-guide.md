# `.mctx` Agent Operating Guide

How a coding agent should use one `.mctx` file as its persistent memory across
sessions and context-window resets. Use with the library in `src/mctx.rs`.

## 1. File anatomy

```
#mctx v1.1 | updated:<ISO date>
%%INDEX
identity:!fixed:v1:0000000058
projects:!durable:v2:0000000134
%%END-INDEX
%%@identity !fixed v:1
user{alias,role}:
  "devil2","student/builder"
%%END
%%@projects !durable v:2
projects[2]{id,title,status}:
  p1,"SmartTodo","shipped"
%%END
```

- `<byte-offset>` is the exact byte position of that section's `%%@name` line.
  Seek there directly — never scan the whole file.
- `v:<N>` increments by 1 every time a section body is edited.
- Bodies use TOON-style tabular arrays (`name[count]{f1,f2}:` header, then one
  flat row per line) or plain `key: value` lines. No braces, no repeated keys,
  minimal quotes.

## 2. Durability tiers — route every fact correctly

| Tier | Use for | Agent behavior |
|---|---|---|
| `!fixed` | Identity: project name, stack, hard constraints | Never auto-overwrite. Change only on explicit user correction. |
| `!durable` | Current-state: open tasks, decisions, file map | Update in place when superseded; bump `v:`. |
| `!volatile` | Short-lived: checkpoints, scratch, session state | Safe to drop on cleanup; never treated as ground truth on resume. |

**Decision rule:** still true a month from now without anyone touching it →
`!fixed`/`!durable`. Only useful for *this* task or *this* session → `!volatile`.

## 3. Read protocol

1. Load the `%%INDEX` only (a few lines) to see sections, tiers, versions, offsets.
2. `Store::read(name)` seeks straight to one section and reads until `%%END`.
   Read only what you need.
3. On resume after a context reset: read `@checkpoint` first, before anything
   else, and continue from its `next:` field.

## 4. Write protocol

1. Reduce new info to the fewest atomic rows — one fact per row, no narration.
2. Pick the tier by the rule in §2, not by convenience.
3. If the section exists: update it in place, bump `v:`, don't touch others.
4. If it's new: `Store::write(name, tier, body)` appends it at `v:1`.
5. Always write through the library — it rebuilds the index so offsets stay
   byte-exact. Never hand-edit offsets.

## 5. Context-exhaustion checkpoint protocol

When you're close to running out of tokens **mid-task**, stop at a clean point
and `Store::checkpoint(body)` a `!volatile` section containing:

- `task:` — one line, what you were doing
- `done:` — what's completed and verified
- `next:` — the very next concrete step, specific enough to resume cold
- `files_touched:` — paths you edited or need to revisit
- `blockers:` — anything unresolved

Overwrite the previous checkpoint rather than accumulating history. On the
next session, read it first and resume from `next:`.

## 6. Naming conventions

`identity` (`!fixed`), `decisions` (`!durable`), `tasks` (`!durable`),
`checkpoint` (`!volatile`), `log` (`!volatile`), `errors` (`!volatile`).

## 7. Anti-patterns

- Don't tag everything `!durable` "to be safe" — volatile clutter bloats resumes.
- Don't read the whole file for one section — that defeats the seek-index design.
- Don't let two sections state the same fact — fix the stale one, don't add a third opinion.
- Don't skip the checkpoint — write it early and overwrite it as you progress.
