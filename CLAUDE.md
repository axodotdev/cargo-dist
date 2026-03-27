# cargo-dist (dist)

Shippable application packaging for Rust (and other languages). dist handles the full release pipeline: planning releases, building binaries and installers, hosting artifacts, publishing packages, and announcing releases. It generates its own CI scripts (GitHub Actions) to automate the entire flow.

## Primary Language

Rust (edition 2021, MSRV 1.74, toolchain 1.93)

## Workspace Structure

This is a Cargo workspace with three crates:

| Crate | Purpose |
|-------|---------|
| `cargo-dist/` | Main CLI and library. Binary is named `dist`. Contains all core logic: planning, building, installer generation, CI generation, signing, and config handling. |
| `cargo-dist-schema/` | JSON schema definitions for `dist-manifest.json`, the machine-readable release manifest. Kept separate so downstream tools can depend on just the schema. |
| `axoproject/` | Workspace/project discovery. Understands Cargo (Rust), npm (JavaScript), and generic project layouts. Extracts metadata like binaries, versions, and repository info. |

### First-Party Dependencies (axodotdev ecosystem)

- **axoasset** — File I/O and remote asset fetching (JSON, TOML, YAML, compression)
- **axocli** — CLI boilerplate (error reporting, logging setup)
- **axoprocess** — Process spawning with better error messages
- **axotag** — Git tag parsing and version extraction
- **axoupdater** — Self-update functionality

## Key Commands

```sh
cargo build                    # Build the workspace
cargo test                     # Run all tests (do NOT use nextest — tests cannot run in parallel)
cargo insta review             # Review snapshot test changes after running tests
cargo clippy --workspace       # Lint all crates
cargo fmt --all                # Format all crates
typos                          # Check for typos (install: cargo install typos-cli)
```

**Important:** The test suite uses [insta](https://insta.rs) for snapshot testing. After `cargo test`, run `cargo insta review` to inspect and approve changed snapshots.

**Important:** Tests are stateful and must run serially. Do not use `cargo-nextest` or any parallel test runner.

## Architecture

### Core Pipeline (in `cargo-dist/src/`)

The heart of dist is `tasks.rs` which computes the full dependency graph of work via `gather_work()`:

1. **Config** (`config/`) — Reads `dist-workspace.toml` and `[workspace.metadata.dist]` from Cargo.toml. Supports v0 and v1 config formats with migration path (`v0_to_v1.rs`).
2. **Planning** (`tasks.rs`) — Builds a `DistGraph` describing all releases, variants, artifacts, and build steps. This is the central data structure.
3. **Building** (`build/`) — Executes build steps: `cargo.rs` (Cargo builds), `generic.rs` (generic builds), `fake.rs` (mock builds for testing).
4. **Backends** (`backend/`) — Generates output artifacts:
   - `ci/github.rs` — GitHub Actions release.yml generation
   - `installer/` — Shell, PowerShell, Homebrew, npm, MSI, macOS .pkg installer generation
   - `templates.rs` — Minijinja template rendering (templates live in `cargo-dist/templates/`)
5. **Hosting** (`host.rs`) — Uploads artifacts to GitHub Releases
6. **Announcing** (`announce.rs`) — Release notes, changelogs, tag parsing
7. **Signing** (`sign/`) — macOS codesigning, SSL.com signing
8. **Manifest** (`manifest.rs`) — Generates `dist-manifest.json` (schema defined in `cargo-dist-schema`)

### Config System

dist is transitioning from v0 config (embedded in `Cargo.toml` under `[workspace.metadata.dist]`) to v1 config (`dist-workspace.toml` and `dist.toml` files). Both are supported. The `config/v1/` directory contains the new config format.

### Template System

Installer scripts and CI configs are generated via [minijinja](https://docs.rs/minijinja) templates in `cargo-dist/templates/`. The `include_dir` crate embeds these at compile time.

## Code Style

- `#![deny(missing_docs)]` is enforced on `cargo-dist` — all public items need doc comments
- Clippy is enabled; `single_match` and `result_large_err` are explicitly allowed
- Use `camino::Utf8Path`/`Utf8PathBuf` instead of `std::path::Path` throughout
- Use `miette` for error reporting (not `anyhow`)
- Use `thiserror` for error type definitions
- Use `tracing` for logging (not `log` or `println!`)
- Prefer `axoprocess::Cmd` over `std::process::Command` for subprocess calls
- Prefer `axoasset::LocalAsset` for file I/O operations

## Error Handling

Errors are defined in `cargo-dist/src/errors.rs` using `thiserror` and reported via `miette`. Use `DistResult<T>` as the return type. Attach diagnostic context with miette's `#[diagnostic]` and `#[help]` attributes.

## Testing

- Snapshot tests use `insta` with the `filters` feature
- Test fixtures for project configurations are in `axoproject/tests/projects/` (excluded from the workspace build)
- `cargo-dist/src/tests/` contains integration-style tests (config parsing, hosting, tags, mocking)
- The `build::fake` module provides mock build implementations for testing without real compilation

## Contributing

- File an issue before opening a PR (except for trivial fixes)
- Write tests for new features and bug fixes
- The test suite must pass: `cargo test` followed by `cargo insta review`
- See CONTRIBUTING.md for full guidelines

## License

MIT OR Apache-2.0

---
*Generated by [ai-ready](https://github.com/lunacompsia-oss/ai-ready)*
