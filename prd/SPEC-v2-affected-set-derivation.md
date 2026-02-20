# Speck V2 — Affected-Set Derivation and Blacksmith Scheduling Integration

Extends speck's planning and validation capabilities to derive, maintain, and refresh `affected:` metadata that blacksmith's scheduler consumes for conflict-aware parallel scheduling.

Builds on V1 (spec-sandwich.md): plan, validate, map, and sync are prerequisites. This spec adds the derivation engine and the refresh lifecycle that keeps affected sets accurate as the codebase evolves.

---

## Problem

Blacksmith schedules coding agents in parallel using `affected:` declarations on beads — glob patterns listing which files a task will touch. Two tasks with non-overlapping affected sets run concurrently; overlapping tasks are serialized.

Today these declarations are hand-authored, which fails in three ways:

1. **Missing.** Most beads don't have them. Blacksmith schedules optimistically and catches conflicts at integration time, wasting full agent sessions.
2. **Wrong.** Humans guess which files a task will touch before the work starts. The guess is frequently incomplete.
3. **Stale.** When a refactor moves files, pending beads still reference old paths. No mechanism refreshes them.

Speck already does the codebase exploration needed to solve all three. During `spec plan`, it surveys the codebase (Pass 1), does deep dives per requirement (Pass 2), and produces task specs with concrete file references. The codebase map (`spec map`) ties abstract module concepts to concrete files at a specific commit. The missing piece is connecting this knowledge to bead metadata that blacksmith reads.

---

## Design

### Two-Phase Metadata Model

Affected-set derivation uses two layers with different invalidation lifecycles, following the model described in blacksmith's SPEC-v5.

**Layer 1: Intent Analysis (slow, stable)**

When `spec plan` produces a task spec, it records which abstract codebase concepts the task touches. This is a byproduct of the planning exploration — no additional LLM call needed.

```yaml
task:
  id: IMPACT-42
  title: "Add project linkage to impact events"

intent:
  content_hash: a8f3c1...  # hash of task title + description + acceptance criteria
  target_areas:
    - concept: impact_event_model
      reasoning: "Adding FK column to impact_events table"
    - concept: migration_system
      reasoning: "New migration for schema change"
    - concept: impact_event_tests
      reasoning: "Existing tests need updating for new column"
```

Intent analysis is cached against the content hash of the task spec. It survives refactors because concepts like "impact_event_model" are abstract — they don't reference file paths. It only invalidates when the task spec itself is edited (scope change, redefinition).

**Layer 2: File Resolution (fast, volatile)**

A static analysis pass maps abstract concepts from layer 1 onto concrete files at the current HEAD, using the codebase map.

```yaml
resolution:
  task_id: IMPACT-42
  base_commit: abc123
  intent_hash: a8f3c1...

  mappings:
    - concept: impact_event_model
      resolved_files:
        - src/models/impact_event.rs
        - src/schema.rs
      resolved_modules: [models]

    - concept: migration_system
      resolved_files:
        - migrations/
      resolved_modules: [migrations]

    - concept: impact_event_tests
      resolved_files:
        - tests/models/impact_event_test.rs
      resolved_modules: [tests]

  derived:
    affected_files:
      - src/models/impact_event.rs
      - src/schema.rs
      - migrations/**
      - tests/models/impact_event_test.rs
    affected_globs:
      - src/models/impact_event.rs
      - src/schema.rs
      - migrations/**
      - tests/models/impact_event_test.rs
```

File resolution is keyed to `base_commit`. When main advances, the resolution is stale and must be regenerated. Regeneration is cheap — it's static analysis (codebase map lookup, import graph traversal), not an LLM call.

---

## New Commands and Flags

### `spec plan` — intent capture (automatic)

No new flags. During planning, the existing codebase exploration already identifies which areas each task touches. The change is that `spec plan` now persists the intent analysis (layer 1) alongside the task spec in the spec store.

For the initial implementation, layer 1 is simply the concrete file list that planning already produces, stored as the canonical intent. The abstract concept layer can be added later when the codebase map is mature enough to support concept-level resolution.

### `spec sync beads` — affected-set emission

Existing command, new behavior. When syncing task specs to the issue tracker (beads), `spec sync beads` now also writes the `affected:` line into each bead's description, derived from the layer 2 file resolution.

```
$ spec sync beads
Syncing 12 task specs to beads...
  IMPACT-40: affected: src/test_infra/**, package.json  (4 globs)
  IMPACT-41: affected: src/models/impact_event.rs, src/schema.rs  (2 globs)
  IMPACT-42: affected: src/models/impact_event.rs, src/schema.rs, migrations/**  (3 globs)
  ...
✓ 12 beads synced with affected-set metadata
```

**Glob strategy:** File resolution produces concrete file paths. The sync command converts these to globs using simple rules:
- Individual files remain as literal paths: `src/models/impact_event.rs`
- Directories where multiple files are affected become wildcards: `migrations/**`
- When >3 files share a parent directory, collapse to a directory glob: `src/models/**`

### `spec sync beads --refresh`

Re-resolves layer 2 (file resolution) for all pending beads against the current HEAD of main, then updates their `affected:` lines in the issue tracker.

This is the command blacksmith calls from its post-integration hook:

```
$ spec sync beads --refresh
Refreshing affected sets against HEAD (abc123f)...
  IMPACT-43: affected set unchanged
  IMPACT-44: src/utils/format.rs → src/formatting/format.rs (path moved)
  IMPACT-44: affected: src/formatting/format.rs, src/formatting/mod.rs  (updated)
  IMPACT-45: affected set unchanged
✓ 1 bead updated, 2 unchanged
```

**Environment variables consumed from blacksmith:**
- `BLACKSMITH_INTEGRATED_BEAD` — the bead that just integrated (can be skipped since it's now closed)
- `BLACKSMITH_MAIN_COMMIT` — the new HEAD of main to resolve against

**Selective refresh:** If the integrated bead was flagged as a refactor (structural changes), refresh all pending beads. Otherwise, only refresh beads whose layer 2 resolution references files that were modified by the integration. This keeps refresh cost proportional to impact.

### `spec map --diff`

Existing command concept from V1. In the context of affected-set derivation, `--diff` becomes the mechanism for determining which file resolutions are stale.

```
$ spec map --diff
Codebase map diff (abc122 → abc123f):
  renamed: src/utils/format.rs → src/formatting/format.rs
  added:   src/formatting/mod.rs
  deleted: src/utils/mod.rs
  modified: src/lib.rs (re-exports changed)

Affected pending beads:
  IMPACT-44: references src/utils/format.rs (renamed)
  IMPACT-47: references src/utils/mod.rs (deleted)
```

### `spec validate` — expansion detection feedback

After blacksmith integrates a task, it records which files the agent actually modified vs. what was declared in the affected set (expansion events — see blacksmith SPEC-v7). Speck consumes this data to improve future derivation.

```
$ spec validate --expansion-report
Expansion history (last 20 tasks):
  Tasks about "auth" expanded to include "config" in 3/5 cases
  Tasks about "models" expanded to include "tests/models" in 4/6 cases

Recommendation: when deriving affected sets for auth-related tasks,
  include src/config/** as a likely dependency
```

This is a reporting and learning mechanism, not an automated change. It informs the planning LLM (via context injection into `spec plan`) so future task specs produce more accurate affected sets.

---

## Storage

Layer 1 and layer 2 data live in the spec store (not in project git, per V1 architecture):

```
spec store
  ├── requirements/
  ├── task_specs/
  │   └── IMPACT-42.yaml          # includes intent analysis (layer 1)
  ├── resolutions/
  │   └── IMPACT-42.yaml          # layer 2, keyed to base_commit
  ├── expansion_history/
  │   └── events.yaml             # imported from blacksmith DB
  └── planning_history/
```

The spec store is the authoritative source for affected-set derivation. The `affected:` line in the bead description is a materialized projection — it's always re-derivable from the spec store.

---

## Lifecycle

The full lifecycle, showing how speck and blacksmith interact:

```
1. Human writes PRD
       │
2. spec plan (explores codebase, decomposes, captures intent)
       │
3. spec sync beads (writes task specs + affected: lines to beads)
       │
4. blacksmith run (scheduler reads affected: lines, assigns non-conflicting tasks)
       │
5. Agent works in worktree, integrates to main
       │
6. blacksmith post-integration hook:
   │  a. Records expansion event (actual files vs. declared)
   │  b. Runs: spec sync beads --refresh
   │         │
   │         ├── spec map --diff (what changed in main?)
   │         ├── Re-resolve layer 2 for affected pending beads
   │         └── Update affected: lines on beads
       │
7. blacksmith continues scheduling with updated metadata
       │
8. After all tasks: spec validate --expansion-report
       │
9. Expansion insights fed back into spec plan context for next PRD
```

---

## Verification

### Intent capture during planning
- Integration test: `spec plan` on a recorded cassette produces a task spec with intent analysis
- Layer 1 intent is cached: re-running plan with same task content reuses cached intent
- Editing task description invalidates intent (content hash changes)

### Affected-set sync
- Integration test: `spec sync beads` writes `affected:` lines that match blacksmith's expected format
- Unit test: glob collapse rules (>3 files in same dir → directory glob)
- Round-trip test: blacksmith's `parse_affected_set()` can parse what speck writes

### Refresh lifecycle
- Integration test: after a file rename in main, `--refresh` updates the affected line on pending beads
- Integration test: non-refactor integration only refreshes beads that reference modified files
- Integration test: refactor integration refreshes all pending beads

### Expansion feedback
- Unit test: given blacksmith expansion event data, report correctly identifies patterns
- The expansion report is informational — no automated behavior changes to verify

---

## Sequencing

1. **Intent capture in `spec plan`** — store concrete file lists as layer 1 (no abstract concepts yet)
2. **`spec sync beads` with affected-set emission** — write `affected:` lines from stored file lists
3. **`spec sync beads --refresh`** — re-resolve against new HEAD after integration
4. **`spec map --diff`** — selective refresh based on what actually changed
5. **Expansion feedback** — import blacksmith expansion events and report patterns

Items 1–2 are useful immediately and unblock smarter scheduling for any project using both tools. Items 3–4 close the staleness loop. Item 5 improves derivation accuracy over time.

---

## Open Questions

1. **Glob granularity.** Should speck emit fine-grained file lists (`src/auth/handler.rs, src/auth/types.rs`) or coarse directory globs (`src/auth/**`)? Fine-grained is more precise but more likely to be incomplete. Coarse is safer but reduces parallelism. The right answer may depend on codebase maturity — coarse early, fine-grained once expansion history gives confidence.

2. **Layer 1 abstraction level.** The initial implementation uses concrete file lists as layer 1 intent. When should we introduce abstract concepts? Probably when `spec map` is mature enough to reliably resolve concept → files, and when we have enough expansion history to know the concrete approach is insufficient.

3. **Refresh trigger.** Blacksmith's post-integration hook is pull-based (blacksmith triggers speck). An alternative is push-based: speck watches for main advancing and proactively refreshes. Pull is simpler and sufficient initially.
