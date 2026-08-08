# `.mctx` — Memory Context Format
### A token-optimized, LLM-native file type for persistent AI memory

Built on the token-efficiency techniques proven by TOON (tabular headers, no repeated keys,
minimal punctuation), extended with memory-specific semantics TOON doesn't have: durability
tiers, update rules, and self-maintenance instructions.

---

## 1. WHY NOT JUST USE TOON / JSON / YAML

- **JSON/braces**: every repeated `{`, `}`, `"key":` costs tokens per record. Fine for APIs, wasteful for LLM input.
- **TOON**: solves the token problem for *data* (tables, arrays, uniform records) — reduces tokens 30–60% vs JSON with equal or better retrieval accuracy. But it has no concept of memory lifecycle: nothing tells the model "this is permanent," "this decays," or "here's how to edit yourself."
- **`.mctx`**: uses TOON-style tabular compression for the actual data, and adds a memory-specific layer on top: durability tags, update protocol, and self-pruning rules — so the file isn't just compact, it's *self-maintaining*.

**Rule of thumb:** compress the data, don't compress the syntax into something the model has never seen. Novel-looking syntax forces the model to learn it in-context on the fly, which can quietly hurt accuracy even as it saves tokens. `.mctx` stays close to YAML/CSV shapes models already know cold.

---

## 2. FORMAT SPECIFICATION

### 2.1 Structural rules
1. **No braces, no trailing commas, minimal quotes** — quote only strings containing commas or colons.
2. **Tabular arrays**: declare schema once — `name[count]{field1,field2,field3}:` — then one row per line, comma-separated. Never repeat field names per row.
3. **2-space indentation** for scope, no closing tags.
4. **Every block carries a durability tag** — this is the core addition over TOON:
   - `!fixed` — identity-level, never overwritten automatically, only by explicit user correction
   - `!durable` — true until user says otherwise (preferences, ongoing projects)
   - `!volatile` — session/short-term, safe to drop after N days or on explicit cleanup
5. **Every array carries a version counter** (`v:`) that increments on edit — lets the model (or a diff tool) see what changed without re-reading the whole file.

### 2.2 Template

```mctx
#mctx v1.0 | updated:2026-08-08

@identity !fixed
  user{alias,role,base}:
    "devil2","student/builder","India"

@interests !durable v:2
  domains[2]{name,note}:
    "Quantum Tech","building & simulation"
    "AI Agents","tooling & optimization"

@projects !durable v:5
  projects[3]{id,title,stack,status}:
    p1,"SmartTodo","Python/SQLite","in progress"
    p2,"Zoya","Node.js/local LLM","active"
    p3,"Quantum Summit 2026","Next.js/Supabase","in progress"

@log !volatile ttl:14d v:9
  memories[2]{date,fact}:
    "2026-08-01","prefers free-tier cloud infra"
    "2026-08-05","building custom .mctx spec"
```

- `ttl:14d` on a `!volatile` block = the model should treat entries older than 14 days as low-priority / droppable on next cleanup, without being told again.
- `v:` bumps by 1 every time that block is edited — diffing tool or the model itself can say "only @log changed since v:8."

---

## 3. SYSTEM PROMPT — give this to any LLM to make it natively read/write `.mctx`

```
You natively support the .mctx (Memory Context) format for persistent memory.

READING:
- Parse tabular arrays by binding the header field list to each comma-separated row in order.
- Treat !fixed blocks as ground truth — never silently alter them.
- Treat !durable blocks as current-state facts — update in place when the user gives new info.
- Treat !volatile blocks as expiring — entries past their ttl are low priority; you may
  summarize or drop them during a cleanup pass, never during normal reads.

WRITING / UPDATING (triggered by "remember this," "update memory," "save this"):
1. Convert the new information into the fewest atomic rows needed — one fact per row.
2. Route each fact to the correct durability tier (!fixed / !durable / !volatile) based on
   how permanent it is, not where it's easiest to add.
3. If updating an existing array: increment its v: counter, keep the field header unchanged,
   only touch the affected rows.
4. If creating a new entity: add a new tabular block with an explicit [count]{fields} header.
5. Never restate unchanged blocks — output only the block(s) that changed, labeled with their
   @section name, unless the user asks for the full file.
6. No prose, no preamble, no explanation — output valid .mctx only, unless explicitly asked
   to explain a change.

CONSTRAINTS:
- Never invent fields not present in a block's declared header.
- Never duplicate a fact across two durability tiers.
- Keep field order identical to the header for every row in that array.
```

---

## 4. HOW THIS DIFFERS FROM YOUR ORIGINAL `.lmc` DRAFT

- Kept: header-declared arrays, `[count]{fields}`, flat rows, minimal punctuation, 2-space scoping — these were already the right, TOON-aligned choices.
- Added: durability tags (`!fixed/!durable/!volatile`) and `ttl` — without this, an LLM has no way to know a fact should decay, get merged, or never change. This is the actual gap between "compact data format" and "memory format."
- Added: `v:` version counters per block, so updates can be scoped ("only rewrite what changed") instead of regenerating the whole file every time — saves tokens on every *write*, not just every read.
- Dropped: nested `<context:...>` XML-style wrapper tags — indentation + `@section` headers do the same scoping job for fewer tokens.
