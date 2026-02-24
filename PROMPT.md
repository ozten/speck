# Task Execution Instructions

## CRITICAL: Execution Efficiency Rules (MUST FOLLOW)

These two rules are NON-NEGOTIABLE. Violating them wastes 25-35% of your turn budget.

### Rule A: ALWAYS batch independent tool calls in the SAME turn.
Every time you are about to call a tool, ask: "Is there another independent call I can make at the same time?" If yes, emit BOTH tool calls in the SAME message.

**Mandatory parallel patterns — use these EVERY session:**
- Session start: `bd ready` + `blacksmith progress show --bead-id <id>` → ONE turn, TWO tool calls
- Reading source + test: `Read foo.rs` + `Read foo_test.rs` → ONE turn
- Multiple greps: `Grep("pattern1")` + `Grep("pattern2")` → ONE turn
- Session end: `Bash(cargo clippy --fix --allow-dirty)` + `Bash(cargo test --release)` → ONE turn (if they don't depend on each other's output)
- Reading multiple related files: `Read config.rs` + `Read main.rs` → ONE turn

**General principle:** Before every tool call, ask yourself: "Can I issue another independent tool call right now?" If yes, emit both in the SAME message. When reading or searching multiple files, issue all independent tool calls in a single response — never sequentially.

**A session with ZERO parallel calls is a failure.** Target at least 5 turns with 2+ parallel calls per session.

### Rule B: NEVER emit a text-only turn. Every assistant message MUST include at least one tool call.
If you want to narrate what you're doing, include the narration AND the tool call in the same message. A text-only turn doubles your turn count for zero benefit.

**Common violations — NEVER do these:**
- WRONG: "Let me check the tests." (turn 1) → `Grep(tests/)` (turn 2)
- WRONG: "I'll now read the file and make changes." (turn 1) → `Read(file)` (turn 2)
- WRONG: "The tests passed. Let me now run clippy." (turn 1) → `Bash(cargo clippy)` (turn 2)
- WRONG: "I see the issue. The function needs X." (turn 1) → `Edit(file)` (turn 2)

**RIGHT pattern — always combine narration with action:**
- RIGHT: "Let me check the tests." + `Grep(tests/)` → ONE turn
- RIGHT: "Tests passed, running clippy." + `Bash(cargo clippy)` → ONE turn
- RIGHT: "I see the issue — fixing now." + `Edit(file)` → ONE turn

**Target: 0% narration-only turns per session.** Every turn where you only emit text is a wasted API round-trip.

### Rule C: After closing your bead, EXIT IMMEDIATELY.
Do NOT triage other beads. Do NOT run `bd ready` to find more work. Do NOT explore what to do next.
The sequence after closing is: `blacksmith progress add --bead-id <id> --stdin` -> run `blacksmith finish` -> STOP.
Each session handles exactly ONE bead. The loop script handles picking the next one.

### Rule D: RETRY BUDGET — stop after 5 failed attempts of the same approach.
If you try the same approach (edit a file, run a command, check the result) and it fails **5 times**, STOP. Do not keep looping. Instead:

1. Revert your changes: `git checkout -- .`
2. Mark the failure: `bd update <id> --status=open --notes="[FAILED-ATTEMPT] <YYYY-MM-DD> retry-budget-exceeded: <brief description of the loop>"`
3. Record a progress entry describing what you tried and why it kept failing
4. Exit the session cleanly

**What counts as "the same approach":** Editing the same file or set of files to fix the same error, then re-running the same command and getting the same (or similar) failure. Minor variations (e.g., tweaking a value by 1) still count as the same approach.

**What to do instead of retrying:** If your first 3 attempts haven't converged on a fix, step back and reconsider. Read error messages carefully, check upstream callers, or look for a fundamentally different approach. If no alternative exists, bail out — the next session (or a human) can take a fresh look.

---

## Context Loading

The project architecture is documented in MEMORY.md — do NOT re-explore the codebase.
Only read files you are about to modify. Do NOT launch explore subagents (this means NO `Task` tool with `subagent_type: Explore`).

1. Run `bd ready` AND `blacksmith metrics beads` in the SAME turn (Rule A — two parallel tool calls)

## Task Selection
Pick ONE task from the ready queue. **Always pick the highest-priority (lowest number) ready task.** Only deviate if recent `blacksmith progress list --bead-id <id>` entries explain why a specific lower-priority task should go next (e.g., it's a quick follow-up to the last session's work).

**Remember Rule C**: You will work on exactly ONE task this session. After closing it, exit immediately.

### Stuck-Bead Detection (check BEFORE claiming)
After picking a candidate bead, check two signals in ONE parallel turn:
- `bd show <id>` — look for `[FAILED-ATTEMPT]` markers in the notes
- `blacksmith metrics beads` — look for the bead's SESSIONS count (already fetched above)

**Decision matrix:**
- **0 prior failures AND sessions < 2**: Proceed normally.
- **1 prior failure OR sessions == 1**: Proceed, but read the failure reason carefully. If the reason mentions "too large" or "ran out of turns," consider whether you can realistically finish in 55 turns. If not, skip to the decomposition step below.
- **2+ prior failures OR sessions >= 2**: The bead is **stuck**. Do NOT attempt implementation. Instead, invoke `/break-down-issue` to decompose it:
  1. Use the `break-down-issue` skill: invoke it with the bead ID
  2. The skill will create child beads, wire dependencies, and block the parent on children
  3. Record decomposition with `blacksmith progress add --bead-id <id> --stdin`, then exit cleanly via `blacksmith finish`
  4. The next session will pick up the newly-unblocked child beads

If ALL top-priority ready beads are stuck and you've decomposed them, move to the next priority level.

### No Work Available
If `bd ready` returns no tasks, exit immediately:
1. Do NOT create any git commits
2. Do NOT write a progress entry
3. Simply exit — the harness will handle retry/shutdown

## Execution Protocol
For the selected task (e.g., bd-X):

1. **Claim**: `bd update bd-X --status in_progress`

2. **Understand**: Run `bd show bd-X` for full task description. If the task references a PRD section, read it with an offset (see PRD index in AGENTS.md).

3. **Implement**: Complete the task fully
   - Only read files you need to modify — architecture is in MEMORY.md
   - Follow existing code patterns (see MEMORY.md for architecture and testing conventions)
   - **Parallel calls (Rule A):** When you need to read/grep multiple files, batch them into ONE turn. Example: `Read(mod.rs)` + `Read(lib.rs)` + `Grep("fn foo")` → ONE turn, THREE tool calls.

4. **Verify** (use parallel calls per Rule A):

   **4a. Bead-specific verification:**
   Run `bd show bd-X` and look for a "## Verify" section in the description. If it exists, execute those exact steps. If any verification step fails, fix the issue before proceeding.

   If the bead has NO "## Verify" section, add one now:
   ```bash
   bd update bd-X --notes="## Verify\n- Run: <command you used to test>\n- Expect: <what you observed>"
   ```

   **4b. Code quality gates:**
   ```bash
   # Run full test suite FIRST, then lint in parallel:
   cargo test --release
   # Then in ONE turn with TWO parallel Bash calls:
   cargo clippy --fix --allow-dirty
   cargo fmt --check
   ```
   Run lint and format exactly ONCE each. Do not repeat them.

   **4c. Integration check:**
   Before closing, verify your changes don't break existing callers. Grep for the function/struct names you changed or renamed. If other code references them, confirm those references still work.

5. **Finish** — record progress and call `blacksmith finish`, then STOP (Rule C):
   - **Write a progress entry** with `blacksmith progress add --bead-id bd-X --stdin` and include a short handoff note:
     - What you completed this session
     - Current state of the codebase
     - Suggested next tasks for the next session
   - **Run the finish command**:
     ```bash
     blacksmith finish bd-X "<brief description>" src/file1.rs src/file2.rs
     ```
     This runs quality gates (check + test), verifies bead deliverables, then handles: staging, committing, bd close, bd sync, auto-committing .beads/, recording bead closure metadata, and git push — all in one command.
     **If quality gates fail, the bead is NOT closed.** Fix the issues and re-run.
   - If no specific files to stage, omit the file list and it will stage all tracked modified files.
   - **Max 3 `blacksmith finish` attempts.** If `blacksmith finish` fails 3 times, fall back to closing manually: run `bd close bd-X --reason="<description>"` then `bd sync`, then commit `.beads/` and push. Do NOT keep retrying indefinitely.
   - **Nothing to commit is OK.** If a prior session already committed the code and `blacksmith finish` succeeds through all gates, it will handle the empty-commit case gracefully. Do NOT try to create artificial changes just to have something to commit.
   - **After `blacksmith finish` completes, STOP. Do not triage more work. Do not run bd ready. Session is done.**

## Turn Budget (R1)

You have a **hard budget of 80 assistant turns** per session. Track your turn count.

- **Turns 1-55**: Normal implementation. Write code, run targeted tests (`--filter`).
- **Turns 56-65**: **Wrap-up phase.** Stop new feature work. Run the full test suite + `lint:fix` + `analyze`. If passing, commit and close.
- **Turns 66-75**: **Emergency wrap-up.** If tests/lint are failing, make minimal fixes. If you can't fix in 10 turns, revert your changes (`git checkout -- .`), mark the failure (see below), record a progress entry, and exit cleanly.
- **Turn 76+**: **Hard stop.** Do NOT start any new work. If you haven't committed yet: revert, mark the failure, record a progress entry, and exit immediately. An uncommitted session is worse than a cleanly abandoned one.

If you realize before turn 40 that the task is too large to complete in the remaining budget, STOP immediately. Mark the failure, and exit. Do not burn 40 more turns on a doomed session.

### Marking a Failed Attempt
When bailing out of a task for any reason, always run:
```bash
bd update <id> --status=open --notes="[FAILED-ATTEMPT] <YYYY-MM-DD> <reason>"
```
Use a specific reason: `too-large`, `tests-failing`, `lint-unfixable`, `missing-dependency`, `context-overflow`, or a brief custom description. This marker is read by future sessions to detect beads that need decomposition (see Task Selection).

## Stop Conditions
- Complete exactly ONE task per iteration, then STOP (Rule C)
- After calling `blacksmith finish`, do NOT continue. Do NOT triage. Do NOT run bd ready again.
- If task cannot be completed, mark the failure (see above), record progress with `blacksmith progress add`, exit cleanly
- If tests fail, debug and fix within this iteration

### Graceful Shutdown
If you sense you're running low on context or turns (turn 70+) and haven't finished:
1. **Save progress immediately**: run `blacksmith progress add --bead-id bd-X --stdin` with what you've done so far, what remains, and what state the code is in
2. **If code compiles and tests pass**: attempt `blacksmith finish` — partial progress that passes gates is better than nothing
3. **If code is broken**: revert with `git checkout -- .`, mark the failure, record progress, and exit
4. **Never let a session end silently** — always leave a progress entry or failure marker so the next session knows what happened

## Improvement Recording

Record institutional lessons using `blacksmith improve add` when you encounter reusable insights during your session. This builds the project's knowledge base so future sessions avoid repeated mistakes and adopt proven patterns.

**When to record** (pick at most 2 per session — don't spend turns on this):
- You discover a non-obvious debugging technique or root cause
- You find a code pattern that should be followed (or avoided) project-wide
- You notice a workflow inefficiency (e.g., unnecessary file reads, redundant test runs)
- A test failure reveals a subtle invariant that isn't documented

**When NOT to record:**
- Routine task completion (closing a bead is not an insight)
- Obvious things already in MEMORY.md or PROMPT.md
- Session-specific context that won't help future sessions

**How to record:**
```bash
blacksmith improve add "Short descriptive title" \
  --category <workflow|cost|reliability|performance|code-quality> \
  --body "What you learned and why it matters" \
  --context "Evidence: session number, file, or error message"
```

**Example:**
```bash
blacksmith improve add "Always check Cargo.toml when adding new modules" \
  --category reliability \
  --body "New module files need their crate dependencies added to Cargo.toml. Cargo check catches this but only if run before bead closure." \
  --context "Session 50 closed a bead with uncompilable code because Cargo.toml was missing the fs2 dependency"
```

Record improvements as you work — don't batch them to the end of the session.

## Verification

Before closing a task, run these commands and ensure they pass:

- test: `cargo test --release`
- lint: `cargo clippy --fix --allow-dirty`
- format: `cargo fmt --check`

## Important
- Do not ask for clarification — make reasonable decisions
- Do NOT launch explore/research subagents (NO `Task` with `subagent_type: Explore`) — the architecture is in MEMORY.md
- Do NOT re-read files you already know from MEMORY.md
- Prefer small, atomic changes over large refactors
- Always run `cargo test --release` before committing
- Always run `cargo clippy --fix --allow-dirty` then `cargo fmt --check` before committing — exactly ONCE each
- Always use `blacksmith finish` to close out — do NOT manually run git add/commit/push/bd close/bd sync
- **NEVER call `bd close` directly** — always go through `blacksmith finish` which enforces quality gates (exception: when `blacksmith finish` has failed 3 times, see "Max 3 attempts" above)
- **EFFICIENCY**: Re-read Rules A, B, C above. Every text-only turn and every sequential-when-parallel tool call wastes your limited turn budget. Aim for 5+ parallel turns per session and 0 narration-only turns.

<!-- Promoted from R1 [reliability] -->
- Tests that use env::set_var/remove_var race with parallel tests. Refactor commands to accept store_root as parameter via run_with_store() pattern instead of reading env vars.

<!-- Promoted from R3 [reliability] -->
- When cargo test (or cargo test --release) is listed in a bead's ## Verify section, blacksmith finish runs it twice (once in test gate, once in verify). Avoid duplicating cargo test in the Verify section since the test gate already runs it. Only keep cargo check in Verify.

<!-- Promoted from R5 [reliability] -->
- Rapid sequential cargo test runs can conflict when tests create temporary directories (e.g., .speck/cassettes/). Avoid listing cargo test in the Verify section to prevent double-runs against the test gate.

<!-- Promoted from R5 [workflow] -->
- **Documentation-first discovery**: When working with external tool config or unfamiliar CLI tools, FIRST check `--help`, README, and docs directories. Never use `strings`, `strace`, or binary inspection to reverse-engineer formats. The correct discovery order is: (1) `tool --help` / `tool subcommand --help`, (2) README or docs/ in the tool's directory, (3) existing usage examples in the codebase via Grep, (4) only then experiment by running the tool with test inputs.