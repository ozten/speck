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

## Configuration

```bash
cp env.example.txt .env
```

Edit `.env` and set your Anthropic API key:

```
ANTHROPIC_API_KEY=sk-ant-...
```

The `.env` file is gitignored. You can also export the variable directly in your shell.

## Quick start

```bash
cargo run -- plan "your requirement here"
cargo run -- status
cargo run -- show
cargo run -- validate --all
cargo run -- map
```

For a full end-to-end walkthrough (plan a calculator, file issues with bd, build, validate), see [docs/walkthrough.md](docs/walkthrough.md).

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
