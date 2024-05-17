# Rust Cache Action

A GitHub Action that implements smart caching for rust/cargo projects with
sensible defaults.

## Example usage

```yaml
- uses: actions/checkout@v3

# selecting a toolchain either by action or manual `rustup` calls should happen
# before the plugin, as the cache uses the current rustc version as its cache key
- run: rustup toolchain install stable --profile minimal

- uses: Swatinem/rust-cache@v2
  with:
    # The prefix cache key, this can be changed to start a new cache manually.
    # default: "v0-rust"
    prefix-key: ""

    # A cache key that is used instead of the automatic `job`-based key,
    # and is stable over multiple jobs.
    # default: empty
    shared-key: ""

    # An additional cache key that is added alongside the automatic `job`-based
    # cache key and can be used to further differentiate jobs.
    # default: empty
    key: ""

    # A whitespace separated list of env-var *prefixes* who's value contributes
    # to the environment cache key.
    # The env-vars are matched by *prefix*, so the default `RUST` var will
    # match all of `RUSTC`, `RUSTUP_*`, `RUSTFLAGS`, `RUSTDOC_*`, etc.
    # default: "CARGO CC CFLAGS CXX CMAKE RUST"
    env-vars: ""

    # The cargo workspaces and target directory configuration.
    # These entries are separated by newlines and have the form
    # `$workspace -> $target`. The `$target` part is treated as a directory
    # relative to the `$workspace` and defaults to "target" if not explicitly given.
    # default: ". -> target"
    workspaces: ""

    # Additional non workspace directories to be cached, separated by newlines.
    cache-directories: ""

    # Determines whether workspace `target` directories are cached.
    # If `false`, only the cargo registry will be cached.
    # default: "true"
    cache-targets: ""

    # Determines if the cache should be saved even when the workflow has failed.
    # default: "false"
    cache-on-failure: ""

    # Determines which crates are cached.
    # If `true` all crates will be cached, otherwise only dependent crates will be cached.
    # Useful if additional crates are used for CI tooling.
    # default: "false"
    cache-all-crates: ""

    # Determiners whether the cache should be saved.
    # If `false`, the cache is only restored.
    # Useful for jobs where the matrix is additive e.g. additional Cargo features,
    # or when only runs from `master` should be saved to the cache.
    # default: "true"
    save-if: ""
    # To only cache runs from `master`:
    save-if: ${{ github.ref == 'refs/heads/master' }}

    # Specifies what to use as the backend providing cache
    # Can be set to either "github" or "buildjet"
    # default: "github"
    cache-provider: ""
```

Further examples are available in the [.github/workflows](./.github/workflows/) directory.

## Outputs

**`cache-hit`**

This is a boolean flag that will be set to `true` when there was an exact cache hit.

## Cache Effectiveness

This action only caches the _dependencies_ of a crate, so is more effective if
the dependency / own code ratio is higher.

It is also most effective for repositories with a `Cargo.lock` file. Library
repositories with only a `Cargo.toml` file have limited benefits, as cargo will
_always_ use the most up-to-date dependency versions, which may not be cached.

Usage with Stable Rust is most effective, as a cache is tied to the Rust version.
Using it with Nightly Rust is less effective as it will throw away the cache every day,
unless a specific nightly build is being pinned.

## Cache Details

This action currently caches the following files/directories:

- `~/.cargo` (installed binaries, the cargo registry, cache, and git dependencies)
- `./target` (build artifacts of dependencies)

This cache is automatically keyed by:

- the github [`job_id`](https://docs.github.com/en/actions/using-workflows/workflow-syntax-for-github-actions#jobsjob_id),
- the rustc release / host / hash,
- the value of some compiler-specific environment variables (eg. RUSTFLAGS, etc), and
- a hash of all `Cargo.lock` / `Cargo.toml` files found anywhere in the repository (if present).
- a hash of all `rust-toolchain` / `rust-toolchain.toml` files in the root of the repository (if present).
- a hash of all `.cargo/config.toml` files in the root of the repository (if present).

An additional input `key` can be provided if the builtin keys are not sufficient.

Before being persisted, the cache is cleaned of:

- Any files in `~/.cargo/bin` that were present before the action ran (for example `rustc`).
- Dependencies that are no longer used.
- Anything that is not a dependency.
- Incremental build artifacts.
- Any build artifacts with an `mtime` older than one week.

In particular, the workspace crates themselves are not cached since doing so is
[generally not effective](https://github.com/Swatinem/rust-cache/issues/37#issuecomment-944697938).
For this reason, this action automatically sets `CARGO_INCREMENTAL=0` to disable
incremental compilation, so that the Rust compiler doesn't waste time creating
the additional artifacts required for incremental builds.

The `~/.cargo/registry/src` directory is not cached since it is quicker for Cargo
to recreate it from the compressed crate archives in `~/.cargo/registry/cache`.

The action will try to restore from a previous `Cargo.lock` version as well, so
lockfile updates should only re-build changed dependencies.

The action invokes `cargo metadata` to determine the current set of dependencies.

Additionally, the action automatically works around
[cargo#8603](https://github.com/rust-lang/cargo/issues/8603) /
[actions/cache#403](https://github.com/actions/cache/issues/403) which would
otherwise corrupt the cache on macOS builds.

## Cache Limits and Control

This specialized cache action is built on top of the upstream cache action
maintained by GitHub. The same restrictions and limits apply, which are
documented here:
[Caching dependencies to speed up workflows](https://docs.github.com/en/actions/using-workflows/caching-dependencies-to-speed-up-workflows)

In particular, caches are currently limited to 10 GB in total and exceeding that
limit will cause eviction of older caches.

Caches from base branches are available to PRs, but not across unrelated
branches.

The caches can be controlled using the [Cache API](https://docs.github.com/en/rest/actions/cache)
which allows listing existing caches and manually removing entries.

## Debugging

The action prints detailed information about which information it considers
for its cache key, and it outputs more debug-only information about which
cleanup steps it performs before persisting the cache.

You can read up on how to [enable debug logging](https://docs.github.com/en/actions/monitoring-and-troubleshooting-workflows/enabling-debug-logging)
to see those details as well as further details related to caching operations.

## Known issues

- The cache cleaning process currently removes all the files from `~/.cargo/bin`
  that were present before the action ran (for example `rustc`).
