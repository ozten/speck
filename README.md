# speck

Speck sandwiches requirements gathering and verification around the meat of product development.

`speck` is a Rust CLI for spec-driven development workflows:

- `speck plan`: requirements, risks, and milestone planning.
- `speck validate`: test/lint/acceptance verification against specs.
- `speck map`: generate a codebase dependency map.
- `speck show`: inspect spec details.
- `speck status`: display project spec status.
- `speck deps`: list dependency relationships between specs.
- `speck sync`: sync specs to an external tracker (e.g., beads).

## Quick start

```bash
cargo run -- plan
cargo run -- validate --all
cargo run -- status
cargo run -- map
```

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

## Project layout

- `src/cli.rs`: argument parsing and subcommands.
- `src/commands/`: command handlers (`plan`, `validate`, `map`, `show`, `status`, `deps`, `sync`).
- `src/ports/`: trait-based port interfaces (clock, filesystem, git, llm, shell, issues, id_gen).
- `src/adapters/`: live, recording, and replaying adapter implementations.
- `src/cassette/`: record/replay infrastructure for deterministic testing.
- `src/context.rs`: `ServiceContext` wiring for live, recording, and replaying modes.
- `src/spec/`: spec model, checks, signals, and verification logic.
- `src/store/`: spec persistence (YAML-based).
- `src/plan/`: planning subsystem (conversation, survey, reconcile, feedback, signals).
- `src/map/`: codebase map generation and diffing.
- `src/linkage/`: spec-to-code linkage resolution and drift detection.
- `src/sync/`: external tracker sync (beads integration).
- `src/validate/`: validation orchestration.
- `tests/cli.rs`: integration tests for CLI behavior.
- `tests/record_replay.rs`: record/replay round-trip tests.
- `.github/workflows/ci.yml`: CI checks for fmt, lint, and tests.
