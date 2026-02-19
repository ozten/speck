# speck

Speck sandwhiches requirements gathering and verification around the meat of product development.

`speck` is a minimal Rust CLI scaffold focused on two workflows:

- `speck plan`: requirements, risks, and milestone planning.
- `speck verify`: test/lint/acceptance verification.

## Quick start

```bash
cargo run -- plan
cargo run -- verify
```

## Development

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

## Project layout

- `src/cli.rs`: argument parsing and subcommands.
- `src/commands/plan.rs`: `plan` command handler (stub).
- `src/commands/verify.rs`: `verify` command handler (stub).
- `tests/cli.rs`: integration tests for CLI behavior.
- `.github/workflows/ci.yml`: CI checks for fmt, lint, and tests.

## Next implementation steps

1. Add structured plan input/output (`--from`, `--format`).
2. Add real verification runners (tests, lint, policy checks).
3. Add machine-readable output and exit codes for CI integrations.
