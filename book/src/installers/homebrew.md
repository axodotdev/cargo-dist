# Homebrew Installer

> since 0.2.0

dist can automatically build and publish [Homebrew](https://brew.sh) formulae (packages) for your application. Users can install your application with an expression like `brew install axodotdev/tap/axolotlsay` and automatically get updates whenever they update their Homebrew packages.

The homebrew package will [fetch](../reference/artifact-url.md) your prebuilt [archives](../artifacts/archives.md), and install the contents in the traditional homebrew directory structure.

*Building* a formula is pretty straight-forward, but publishing it requires you to create a your own [Homebrew tap](https://docs.brew.sh/Taps) (package repository), because [the core Homebrew tap](https://github.com/Homebrew/homebrew-core) does not accept prebuilt binaries from third parties. This sounds hard, but surprisingly it's not: you need to make a repository named "homebrew-tap" under your GitHub org or user, and get a GitHub API token to push to it. dist will manage the contents of the repo for you.


## Quickstart

To setup your homebrew installer you need to create a custom tap and enable the installer. This is broken up into parts because a project administrator may need to be involved in part 1, while part 2 can be done by anyone.


### Part 1: Creating A Custom Homebrew Tap

1. Create a GitHub repository called "homebrew-tap" (`axodotdev/homebrew-tap`)
2. Create a GitHub [personal access token](https://github.com/settings/tokens/new?scopes=repo) with the `repo` scope
3. Add the token as a [GitHub Secret](https://docs.github.com/en/actions/security-guides/encrypted-secrets) called `HOMEBREW_TAP_TOKEN` to the repository you want to publish **from** (`axodotdev/axolotlsay`)

We recommend initializing the repository with a README, but otherwise the directory structure will be managed by dist, and many separate repos can publish to the same tap without issue.

A Homebrew Tap is just a GitHub repository that starts with `homebrew-`. Many homebrew features allow that prefix to be elided, so the package `axolotlsay` published in `axodotdev/homebrew-tap`, can be installed as `axodotdev/tap/axolotlsay`. Your users don't need to "register" anything to use it, custom taps are just that builtin to Homebrew.


### Part 2: Enabling The Homebrew Installer

1. run `dist init` on your project
2. when prompted to pick installers, enable "homebrew"
3. this should trigger a prompt for your tap (`axodotdev/homebrew-tap`)

...that's it! Assuming you already setup your custom tap, as described in the previous section. If this worked, your config should now contain the following entries:

```toml
[workspace.metadata.dist]
# "..." indicates other installers you may have selected
installers = ["...", "homebrew", "..."]
tap = "axodotdev/homebrew-tap"
publish-jobs = ["homebrew"]
```

Next make sure that `description` and `homepage` are set in your Cargo.toml. These
fields are optional but make for better formula definitions.

```toml
[package]
description = "a CLI for learning to distribute CLIs in rust"
homepage = "https://github.com/axodotdev/axolotlsay"
```

## Renaming Formulae

> since 0.11.0

By default, your formula will be named using the app name (in Rust, this is the crate
name). If you are overriding the bin name, you may want to make your Homebrew formula
match [with the `formula` setting](../reference/config.md#formula):

```toml
[package]
name = "legacyname"

[[bin]]
name = "coolname"
path = "src/main.rs"

[package.metadata.dist]
formula = "coolname"
```


## Adding Binary Aliases

> since 0.14.0

If you want to install symlinked aliases for your binaries, you can do so with the [bin-aliases setting](../reference/config.md#bin-aliases).


## Linuxbrew

> since 0.6.0

The formulae dist builds automatically support Linux and macOS, as long as you release your application for the relevant targets.


## Limitations / Caveats

### There Is Only One Version

**Homebrew fundamentally does not support the notion of a package having multiple published versions.** There is *only* the latest version. **If you publish a new version of a package, it will always replace the current one.** This is why [the `publish-prereleases` setting is disabled by default](../reference/config.md#publish-prereleases): otherwise publishing 2.0.0-prerelease.1 would completely obliterate 1.0.0, which presumably you'd prefer users installing.

Unfortunately if you have any kind of non-linear version history (such as doing a patch release for 1.0 after already releasing 2.0), the published Homebrew package will randomly contain whichever one you released last. The releases are just git commits though, so you can manually revert a release if you want.

### Unsupported Formats

* Does not support creating a formula which builds from source
* Does not support [Cask][issue-cask] for more convenient GUI app installation



[issue-cask]: https://github.com/axodotdev/cargo-dist/issues/309
