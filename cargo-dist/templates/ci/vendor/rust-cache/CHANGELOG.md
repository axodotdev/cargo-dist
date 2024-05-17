# Changelog

## 2.7.3

- Work around upstream problem that causes cache saving to hang for minutes.

## 2.7.2

- Only key by `Cargo.toml` and `Cargo.lock` files of workspace members.

## 2.7.1

- Update toml parser to fix parsing errors.

## 2.7.0

- Properly cache `trybuild` tests.

## 2.6.2

- Fix `toml` parsing.

## 2.6.1

- Fix hash contributions of `Cargo.lock`/`Cargo.toml` files.

## 2.6.0

- Add "buildjet" as a second `cache-provider` backend.
- Clean up sparse registry index.
- Do not clean up src of `-sys` crates.
- Remove `.cargo/credentials.toml` before saving.

## 2.5.1

- Fix hash contribution of `Cargo.lock`.

## 2.5.0

- feat: Rm workspace crates version before caching.
- feat: Add hash of `.cargo/config.toml` to key.

## 2.4.0

- Fix cache key stability.
- Use 8 character hash components to reduce the key length, making it more readable.

## 2.3.0

- Add `cache-all-crates` option, which enables caching of crates installed by workflows.
- Add installed packages to cache key, so changes to workflows that install rust tools are detected and cached properly.
- Fix cache restore failures due to upstream bug.
- Fix `EISDIR` error due to globed directories.
- Update runtime `@actions/cache`, `@actions/io` and dev `typescript` dependencies.
- Update `npm run prepare` so it creates distribution files with the right line endings.

## 2.2.1

- Update `@actions/cache` dependency to fix usage of `zstd` compression.

## 2.2.0

- Add new `save-if` option to always restore, but only conditionally save the cache.

## 2.1.0

- Only hash `Cargo.{lock,toml}` files in the configured workspace directories.

## 2.0.2

- Avoid calling `cargo metadata` on pre-cleanup.
- Added `prefix-key`, `cache-directories` and `cache-targets` options.

## 2.0.1

- Primarily just updating dependencies to fix GitHub deprecation notices.

## 2.0.0

- The action code was refactored to allow for caching multiple workspaces and
  different `target` directory layouts.
- The `working-directory` and `target-dir` input options were replaced by a
  single `workspaces` option that has the form of `$workspace -> $target`.
- Support for considering `env-vars` as part of the cache key.
- The `sharedKey` input option was renamed to `shared-key` for consistency.

## 1.4.0

- Clean both `debug` and `release` target directories.

## 1.3.0

- Use Rust toolchain file as additional cache key.
- Allow for a configurable target-dir.

## 1.2.0

- Cache `~/.cargo/bin`.
- Support for custom `$CARGO_HOME`.
- Add a `cache-hit` output.
- Add a new `sharedKey` option that overrides the automatic job-name based key.

## 1.1.0

- Add a new `working-directory` input.
- Support caching git dependencies.
- Lots of other improvements.

## 1.0.2

- Don’t prune targets that have a different name from the crate, but do prune targets from the workspace.

## 1.0.1

- Improved logging output.
- Make sure to consider `all-features` dependencies when pruning.
- Work around macOS cache corruption.
- Remove git-db cache for now.
