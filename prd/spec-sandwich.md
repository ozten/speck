# Spec Sandwich

*Like the Spanish jamón — the build is the meat, but it doesn't go anywhere without bread on both sides.*

## The Problem

Current agent-driven development is all meat, no bread. An agent gets a vague prompt, makes a bunch of changes, and declares victory. Nobody checked the sandwich.

Specifically, three things go wrong:

1. **Requirements enter the system under-specified.** A "simple" requirement like "add a CSV export button" could be trivial or massive depending on what already exists in the codebase. Without grounding in the code, you can't know.

2. **Work items lack verifiable exit criteria.** The agent stops editing files and reports success, but nobody defined what "done" means in a way that can be mechanically checked.

3. **Verification is an afterthought.** Testing is bolted on after implementation rather than being a constraint that shapes how work is specified and structured.

## The Core Idea

```
spec plan     # top slice — define the work, verify it's verifiable
agent build   # the meat — execute the task
spec validate # bottom slice — prove it's done
```

**Plan** is the gate that refuses to let under-specified work enter the system. It interviews you, explores the codebase, pushes back when verification is unclear, and won't produce a task spec until it can answer "how will we know this is done?"

**Validate** is the gate that refuses to let unfinished work exit the system. It runs the verification strategy that plan defined, checks that references still resolve, confirms assertions pass, and won't mark a task complete until the evidence matches the spec.

They share the same specification. Plan writes the contract. Validate enforces it. If plan can't write a contract that validate could enforce, the work doesn't enter the system. That's the quality ratchet.

## Two-Pass Planning

Requirements decomposition is not a pure planning exercise — it's an exploration of the codebase. Spec Sandwich uses two passes to balance efficiency against depth.

### Pass 1: Broad Survey (Spec-Level)

Read the full spec. Do a single shallow codebase traversal — directory structure, module boundaries, key interfaces, schema, test infrastructure. The goal is to build a **routing table**: "auth-related work lives here, reporting lives here, the data pipeline works like this."

This pass also identifies **cross-cutting concerns** — "three different requirements all assume we can send notifications, but there's no notification system." Those become foundational tasks that must be sequenced first.

**Output:** A codebase map, a dependency graph between requirements, and a list of foundational gaps.

### Pass 2: Deep Dive (Requirement-Level)

Take each requirement (or cluster of related requirements) and do a focused, deep exploration of the specific files and modules it touches. This is where you determine actual changes, discover existing patterns to follow, and produce task specs with real acceptance criteria.

Pass 2 references the pass 1 summary instead of re-exploring the whole codebase. You pay the broad exploration cost once and the deep exploration cost per-requirement.

### Pass 2.5: Reconciliation

After all deep dives, review all proposed task specs together. Look for duplicated efforts, shared abstractions that should be extracted, and dependency ordering issues. This is cheap — you're reading task specs, not re-reading code.

## The Signal Taxonomy

Not all verification is equal. The difficulty of proving "done" varies by what kind of signal the task produces.

### Clear Signal

The output is directly observable and assertable. "The CLI `--help` lists the new subcommand." "The API returns the expected JSON shape." "The migration adds column X." You write a test, it passes or fails.

### Fuzzy-but-Constrainable Signal

The outcome is observable but the correctness criteria are soft. "The timeline component shows events in chronological order with reasonable spacing." You can't screenshot-compare this, but you can decompose the fuzzy requirement into clear structural assertions: events are in date order, each event renders its title and date, the container has proper overflow handling.

### Internal Logic Signal

The hard case. "At point Z in the processing pipeline, we should branch to Y instead of C." Z might not be a function boundary — it's a conditional inside a method, and correctness depends on context.

**The rule:** If a requirement's verification depends on internal logic correctness at a specific code point, the task spec must mandate that the logic be structured to make that decision point observable.

Two approaches:

1. **Refactor to expose the decision.** Extract the logic into a named, testable function. If a decision point is important enough to verify, it's important enough to be an explicit abstraction rather than a buried conditional.

2. **Trace-based verification.** When refactoring isn't practical, instrument the code path to emit a trace at point Z, and assert on the trace. The test runs the job with known inputs and asserts that the trace contains the expected decision.

### The Pushback Rule

If `spec plan` cannot classify the signal type and propose a verification strategy, the requirement is under-specified. This is not a failure — it's the system working. The requirement gets kicked back for clarification before it enters the system.

During decomposition, for each task, plan must:

1. Classify the signal type (clear / fuzzy-but-constrainable / internal logic)
2. For clear signals: propose the assertion directly
3. For fuzzy signals: propose the structural decomposition into clear sub-assertions
4. For internal logic signals: propose either a refactoring that makes the decision point testable, or a trace-based verification strategy
5. If it can't do any of these: refuse to produce a task spec

## The Interactive Pushback Loop

`spec plan` isn't a one-shot generator. It's an interactive conversation that argues with you until the spec is sound.

```
You: "Grant funders should see a timeline of impact events per project"

Plan: "I found ImpactEvent and Project models. ImpactEvent isn't
       linked to Project — only to Article. I'm proposing 4 tasks.

       Task 3 (Timeline UI component) — I can't propose a verification
       strategy. The codebase has no component tests and no Playwright
       setup. Options:

       a) Add a foundational task for component test infrastructure first
       b) Use structural assertions only (renders without error, correct
          DOM structure)
       c) Add Playwright e2e test infrastructure as a foundational task

       Which approach, or do you want to refine the requirement?"

You: "Option a, and the timeline should also support filtering"

Plan: "Adding foundational task IMPACT-40: component test infrastructure.
       Updating Task 3 to depend on it. For filtering — the filter
       criteria aren't specified. What can users filter by? And should
       filters be URL-persisted or component-local state?"
```

## Storage Architecture

Specs, the codebase map, and the project code have **three different lifecycles** and must not be version-coupled.

### Why Specs Don't Belong in Git

Specs represent intent. Rolling back the codebase doesn't change what you want to build. Deleting a feature branch doesn't mean you've abandoned the plan. A spec might outlive multiple implementation attempts.

If specs live in git and you roll back, file references might now point to the right line again... or might not, because the spec was written against a different version. It's a false sense of coherence.

### Three Separate Lifecycles

**Specs** (durable plans — database, separate repo, or API — not in project git):
- Requirements and task decompositions
- Verification contracts
- Planning history and conversation logs
- Abstract references to codebase concepts ("the MetricsService"), not concrete file paths

**Codebase map** (versioned snapshot — derived, disposable, regenerated):
- Module boundaries, key interfaces, schema summary
- Existing patterns and conventions
- Test infrastructure inventory
- Tied to a commit hash, regenerated when code changes

**Linkage** (resolved at execution time by validate):
- Abstract spec references resolved against the current codebase map
- Drift detection: "MetricsService has changed since this spec was authored"
- Re-resolution after refactors: same spec, different file paths

### Storage Model

```
spec store (not in project git)
  ├── requirements/
  ├── task specs (with abstract references)
  ├── verification contracts
  └── planning history

project repo (git-versioned)
  └── .spec-cache/
      ├── codebase_map.yaml (regenerated, tied to commit hash)
      └── treated as derived artifact
```

This enables useful operations like planning against one branch and validating against another: "Here's what we planned for the feature — does it still make sense against main after last week's refactor?"

## Integration with Issue Tracking

Spec Sandwich does not replace your issue tracker (e.g., Beads). The issue tracker remains the **coordination layer** — assignment, priority, status, review. Spec Sandwich is the **specification layer** — what needs doing and how to verify it.

The issue tracker is a projection of the spec store into a workflow system:

```
Spec Store (rich, validated, codebase-aware)
    │
    ├── creates/updates issues in tracker (lightweight summaries
    │   with links back to the full spec)
    │
    └── agent reads full spec from spec store when executing
        then updates status in tracker when done
```

Why this separation matters:

- **Validation.** The spec tool can say "you referenced a file that doesn't exist." The issue tracker can't.
- **Pushback.** The spec tool refuses under-specified work. The issue tracker accepts what you give it.
- **Structured relationships.** Task A's verification depends on infrastructure from Task B. Task C and D modify the same file and need sequencing. These are typed, semantic relationships that issue trackers can't represent.
- **Liveness.** A file reference in the spec store can warn you when the referenced code has changed. A string in an issue description can't.

## The Recursive Feedback Property

When validate fails — the agent thinks it's done but verification doesn't pass — that failure is itself a signal that flows back to plan. Either:

- The implementation is wrong → agent iterates
- The spec was flawed → plan revises the contract

The system learns what "well-specified" means by discovering what fails at the exit gate.

## Task Spec Format (Draft)

Each task that `plan` produces and `validate` consumes should include:

```yaml
task:
  id: IMPACT-42
  title: "Add project linkage to impact events"
  requirement: timeline-view  # parent requirement reference

context:
  modules: [MetricsService, ImpactEvent]  # abstract references
  patterns: "Follow existing migration conventions"
  dependencies: [IMPACT-40, IMPACT-41]

acceptance_criteria:
  - impact_events table has project_id column (nullable, FK to projects)
  - existing data is unaffected
  - migration is reversible

signal_type: clear  # clear | fuzzy | internal_logic

verification:
  strategy: direct_assertion
  checks:
    - type: sql_assertion
      query: "SELECT column_name FROM information_schema.columns WHERE table_name = 'impact_events' AND column_name = 'project_id'"
      expected: "one row returned"
    - type: test_suite
      command: "npm run test:db"
      expected: "all pass"
    - type: migration_rollback
      description: "Run down migration, verify column removed"
```

For an internal logic signal, the spec would additionally include:

```yaml
signal_type: internal_logic

verification:
  strategy: refactor_to_expose
  decision_point: "metric inclusion logic in report generation"
  required_structure: "Extract to named function shouldIncludeMetrics(event): boolean"
  cases:
    - input: { event: { audienceData: null } }
      expected: false
    - input: { event: { audienceData: { reach: 1000 } } }
      expected: true
```

Or for trace-based:

```yaml
signal_type: internal_logic

verification:
  strategy: trace_assertion
  trace_point: "metric_inclusion_decision"
  test_input: "fixtures/report_with_missing_metrics.json"
  expected_trace:
    - { eventId: "evt_1", decision: "skip", reason: "missing_audience_data" }
    - { eventId: "evt_2", decision: "include", reason: "complete_data" }
```

## CLI Design

```bash
# Planning (entry gate)
spec plan "grant funders should see impact timeline"
spec plan --from requirements.md

# Validation (exit gate)
spec validate IMPACT-42
spec validate --all

# Codebase map management
spec map                  # regenerate codebase map
spec map --diff           # show what's changed since last map

# Issue tracker sync
spec sync beads
spec sync beads --dry-run

# Inspection
spec show IMPACT-42       # what an agent sees when picking up a task
spec status               # overview of all tasks and their states
spec deps                 # dependency graph
```

## Build Path

1. **Define the task spec schema.** Nail down the YAML format for task specs with signal classification, verification strategies, and abstract codebase references. Manually author a few specs to validate the format.

2. **Build `spec validate`.** A CLI that reads task specs and runs their verification checks against the current codebase. Start simple — check file references resolve, run specified test commands, assert on outputs.

3. **Build `spec plan`.** The interactive planning command backed by an LLM. Does codebase exploration, proposes decompositions, classifies signals, and refuses to produce specs without verification strategies.

4. **Build `spec map`.** The codebase cartography tool that generates the structural snapshot. Used by both plan and validate.

5. **Add issue tracker sync.** `spec sync` pushes task summaries to Beads, keeps status bidirectional.

6. **Build the agent execution loop.** Reads from the spec store, implements, runs `spec validate`, reports results.
