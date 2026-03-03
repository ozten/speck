# speck

> **Status: Not Ready for Use**
> This project is still being actively designed and iterated on. APIs, commands, and behavior may change significantly. Do not depend on it for production workflows.

Speck sandwiches requirements gathering and verification around the meat of product development.

`speck` is a Rust CLI for **spec-driven development**: transform requirements into verifiable task specs, then prove they're done with automated checks.

## Quick Start

```bash
# Build
cargo build

# Configure API key (required for speck plan)
cp env.example.txt .env   # then edit with your Anthropic API key

# Plan → Inspect → Validate
speck plan "your requirement here"
speck status
speck show
speck validate --all
```

See the [Getting Started](docs/getting-started.md) guide for full setup instructions.

## Commands

| Command | Description |
|---|---|
| `speck plan` | Generate task specs from requirements via multi-pass LLM analysis |
| `speck validate` | Run verification checks against specs |
| `speck map` | Generate codebase structure maps and detect drift |
| `speck status` | List all specs with signal type and strategy |
| `speck show` | Inspect spec details |
| `speck deps` | Visualize dependency graph between specs |
| `speck sync` | Push specs to external issue trackers (beads/bd) |

## Documentation

| Document | Description |
|---|---|
| [Overview](docs/overview.md) | What Speck is, philosophy, and how features connect |
| [Getting Started](docs/getting-started.md) | Installation, configuration, and first run |
| [CLI Reference](docs/cli-reference.md) | Full command documentation with all options |
| [Planning](docs/planning.md) | Multi-pass planning pipeline deep dive |
| [Validation](docs/validation.md) | Verification checks, strategies, and the feedback loop |
| [Spec Format](docs/spec-format.md) | TaskSpec YAML structure reference |
| [Codebase Mapping](docs/codebase-mapping.md) | Map generation, caching, and drift detection |
| [Sync](docs/sync.md) | External tracker integration (beads/bd) |
| [Architecture](docs/architecture.md) | Port/adapter design, project layout, request flow |
| [Record/Replay](docs/record-replay.md) | Cassette system for deterministic testing |
| [Development](docs/development.md) | Building, testing, and contributing |
| [Walkthrough](docs/walkthrough.md) | End-to-end tutorial: plan a calculator, file issues, validate |

## Development

```bash
git config core.hooksPath .githooks
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

See [Development](docs/development.md) for full details.
