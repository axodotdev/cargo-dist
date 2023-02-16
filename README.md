# cargo-dist

[![crates.io](https://img.shields.io/crates/v/cargo-dist.svg)](https://crates.io/crates/cargo-dist) [![docs](https://docs.rs/cargo-dist/badge.svg)](https://docs.rs/cargo-dist)
![Rust CI](https://github.com/axodotdev/cargo-dist/workflows/Rust%20CI/badge.svg?branch=main)

`cargo build` but For Building Final Distributable Artifacts and uploading them to an archive.

The Big Idea of cargo-dist is that we want to streamline all the steps of providing prebuilt binaries
for a rust project. This includes:

1. Generating your "Cut A Release" Github CI for you
2. Picking good build flags for a "production" release
3. Making zips and installers for the resulting binaries
4. Generating a machine-readable manifest so other tools can understand the results
5. Uploading all the resulting artifacts to a Github Release™️

Even though cargo-dist is primarily a tool for building and packaging applications (steps 2-4), we put a fair amount of effort into Generating Your CI Scripts For You because we want to be able to run things locally and know what the CI *should* do without actually running it. It also helps avoid needless vendor lock-in -- in an ideal world, migrating from Github to Gitlab or your own personal infra would be just one invocation of cargo-dist away!

That said, the current version is Very Very Unstable And Experimental and the extra conveniences currently only work with Github CI and Github Releases™️!

* [Way-Too-Quick Start](#way-too-quick-start)
* [Installation](#installation)
  * [Install Prebuilt Binaries With cargo-binstall](#install-prebuilt-binaries-with-cargo-binstall)
  * [Build From Source With Cargo](#build-from-source-with-cargo)
  * [Download Prebuilt Binaries From Github Releases](#download-prebuilt-binaries-from-github-releases)
  * [Use The Installer Scripts](#use-the-installer-scripts)
* [Setup](#setup)
  * [Configuring Installers](#configuring-installers)
  * [Configuring Targets](#configuring-targets)
* [Usage (CI)](#usage-ci)
* [Usage (Local Builds)](#usage-local-builds)
* [Concepts](#concepts)
* [Build Flags](#build-flags)
* [Compatibility With Other Tools](#compatibility-with-other-tools)
* [Contributing](#contributing)
  * [Updating Snapshots](#updating-snapshots)
  * [Cutting Releases](#cutting-releases)



# Way-Too-Quick Start


```sh
# install tools
cargo install cargo-dist

# one-time setup
cargo dist init --ci=github
git add .
git commit -am "wow shiny new cargo-dist CI!"

# cut a release like you normally would

# <manually update the version of your crate, run tests, etc>
# then:
git commit -am "chore: Release version 0.1.0"
git tag v0.1.0
cargo publish
git push
git push --tags
```

That's gonna do a whole bunch of stuff you might not have expected, but if it all works you'll get a Github Release™️ with built and zipped artifacts uploaded to it! Read the rest of the docs to learn more!

You may have noticed "cut a release" still has a lot of tedious work. That's because we recommend using [cargo-release](https://github.com/crate-ci/cargo-release) to streamline the last step:

```sh
# install tools
cargo install cargo-dist
cargo install cargo-release

# one-time setup
cargo dist init --ci=github
git add .
git commit -am "wow shiny new cargo-dist CI!"

# cut a release
cargo release 0.1.0
```

(I left off the --execute flag from `cargo-release` so you won't actually break anything if you really did just copy paste that 😇)




# Installation


## Install Prebuilt Binaries With cargo-binstall

```sh
cargo binstall cargo-dist --no-symlinks
```

(Without `--no-symlinks` [this may fail on Windows](https://github.com/cargo-bins/cargo-binstall/issues/728))


## Build From Source With Cargo

```sh
cargo install cargo-dist --profile=dist
```

(`--profile=dist` may get you a slightly more optimized binary.)


## Install From The AUR

Arch Linux users can install `cargo-dist` from the [AUR](https://aur.archlinux.org/packages?O=0&SeB=nd&K=cargo-dist&outdated=&SB=p&SO=d&PP=50&submit=Go) using an [AUR helper](https://wiki.archlinux.org/title/AUR_helpers). For example:

```sh
paru -S cargo-dist
```

## Download Prebuilt Binaries From Github Releases

[See The Latest Release](https://github.com/axodotdev/cargo-dist/releases/latest)!

## Use The Installer Scripts

**NOTE: these installer scripts are currently under-developed and will place binaries in `$HOME/.cargo/bin/` without properly informing Cargo of the change, resulting in `cargo uninstall cargo-dist` and some other things not working. They are however suitable for quickly bootstrapping cargo-dist in temporary environments (like CI) without any other binaries being installed.**

Linux and macOS:

```sh
curl --proto '=https' --tlsv1.2 -L -sSf https://github.com/axodotdev/cargo-dist/releases/download/v0.0.2/installer.sh | sh
```

Windows PowerShell:

```sh
irm 'https://github.com/axodotdev/cargo-dist/releases/download/v0.0.2/installer.ps1' | iex
```

# Setup

Once cargo-dist is installed, you can set it up in your cargo project by running

```sh
cargo dist init --ci=github
```

This will:

* Add a `dist` build profile to your Cargo.toml (with recommended default build flags)
* Add a `.github/workflows/release.yml` file to your project (only if you pass `--ci=...`)

These changes should be checked in to your repo for whenever you want to cut a release.

If you don't want ci scripting generated, but just want the `dist` profile you can do:

```sh
cargo dist init
```

If you want to just (re)generate the ci scripts, you can do:

```sh
cargo dist generate-ci
```

(This assumes you have set `ci = ["github"]` in your Cargo.toml, which you should do so that things like installers understand that Github Releases are a place to fetch artifacts from. You *can* pass "github" to generate-ci to test it out, but it won't persist. Maybe that UX should be reworked.)

See the next section ("Usage (CI)") for how the github workflow is triggered and what it does.



## Configuring Installers

If you would like to generate (still under development) installer scripts, you can pass `--installer` flags
to either `init` or `generate-ci`:

```sh
cargo dist init --ci=github --installer=shell --installer=powershell
```

This will result in `installer.sh` and `installer.ps1` being generated which fetch from a Github Release™️ and copy the binaries to `$HOME/.cargo/bin/` on the assumption that this is on your PATH. The scripts are currently brittle and won't properly tell Cargo about the installation (making `cargo uninstall` and some other commands behave incorrectly). As such they're currently only really appropriate for setting up temporary environments like CI without any other binaries. This will be improved in the future.



## Configuring Targets

By default, `init` and `generate-ci` will assume you want to build for a "standard desktop suite of targets". This is currently:

* x86_64-pc-windows-msvc
* x86_64-unknown-linux-gnu
* x86_64-apple-darwin

(In The future arm64 counterparts and linux-musl will probably join this, but unfortunately we currently don't support cross-compilation.)

If you would like to manually specify the targets, you can do this with `--target=...` which can be passed any number of times. If this flag is passed then the defaults will be cleared. 

Other commands like `cargo dist build` (bare `cargo dist`) will always default to only using the current host target, and may need more manual target specification. This is handled automatically if you're using dist's generated CI scripts.

**cargo-dist does not currently support specifying additional targets based on different `--features` or anything else, this will change in the future. See [issue #22](https://github.com/axodotdev/cargo-dist/issues/22) for discussion.**




# Usage (CI)

Once you've completed setup (run `cargo dist init --ci=...`), you're ready to start cutting releases!

The github workflow will trigger whenever you push a [git tag](https://git-scm.com/book/en/v2/Git-Basics-Tagging) to the main branch of your repository that looks like a version number (`v1`, `v1.2.0`, `v0.1.0-prerelease2`, etc.).

You might do that with something like this:

```sh
# <first manually update the version of your crate, run tests, etc>
# then:
git commit -am "chore: Release version 0.1.0"
git tag v0.1.0
cargo publish
git push
git push --tags
```

That's a bunch of junk to remember to do, so we recommend using [cargo-release](https://github.com/crate-ci/cargo-release) to streamline all of that:

```sh
cargo release 0.1.0
```

> NOTE: this will do nothing unless you also pass `--execute`, this is omitted intentionally!

> ALSO NOTE: if your application is part of a larger workspace, you may want to configure cargo-release with things like `shared-version` and `tag-name` to get the desired result. In the future the CI scripts we generate may be smarter and able to detect things like "partial publishes of the workspace". For now we assume you're always publishing the entire workspace!

cargo-release will then automatically:

1. Bump all your version numbers
2. Make a git commit
3. Make a git tag
4. Publish to crates.io (disable this with `--no-publish`)
4. Push to your repo's main branch

When you *do* push a tag (and the commit it points to) the CI will take over and do the following:

1. Create a *draft* Github Release™️
2. Build your application for all the target platforms, wrap them in zips/tars, and upload them to the Github Release™️
3. (Optional, see setup) Build installer scripts that fetch from the Github Release™️
4. Generate a [dist-manifest.json](https://github.com/axodotdev/cargo-dist/tree/main/cargo-dist-schema) describing all the artifacts and upload it to the Github Release™️
5. On success of all the previous tasks, mark the Github Release™️ as a non-draft

The reason we do this extra dance with drafts is that we don't want to notify anyone of the release until it's Complete, but also don't want to lose anything if some only some of the build tasks failed.




# Usage (Local Builds)

> When you run bare `cargo dist` this is actually a synonym for `cargo dist build`. For the sake of clarity these docs will prefer this longer form.

The happy path of cargo-dist is to just have its generated CI scripts handle all the details for you, so you never *really* need to run `cargo dist build` if you're happy to leave it to the CI. But there's plenty of reasons to want to do a local build, or to just want to understand what the builds do, so here's the docs for that!

At a high level, `cargo dist build` will:

* create a `target/distrib/` folder
* run `cargo build --profile=dist` on your workspace
* copy built-assets reported by `cargo` into `target/distrib/`
* copy static-assets like README.md
* bundle things up into zips/tarballs ("Artifacts")
* give you paths to all the final Artifacts for you to do whatever with

If you pass `--output-format=json` it will also produce a machine-readable dist-manifest.json describing all of this.

If you pass `--installer=...` it will also produce that installer artifact (see [Configuring Installers](#configuring-installers)).

If you pass `--target=...` it will build for that target instead of the host one (see [Configuring Targets](#configuring-targets)).

If you pass `--no-builds` you can make it skip cargo builds and just focus on generating artifacts that don't require a build (like install scripts).

If you run `cargo dist manifest --output-format=json` it will skip generating artifacts and just produce `dist-manifest.json`. Notably, if you pass every `--installer` and `--target` flag at once to this command you will get a unified manifest for everything you should produce on every platform. `--no-local-paths` will strip away the actual paths pointing into `target`, which would otherwise become giberish if the artifacts get moved to another system.

For further details, see [Concepts](#concepts) and [Build Flags](#build-flags).



# Concepts

cargo-dist views the world as follows:

* You are trying to publish *Applications* (e.g. "ripgrep" or "cargo-binstall")
* An Application has *Releases*, which are a specific version of an Application (e.g. "ripgrep 1.0.0" or "cargo-binstall 0.1.0-prerelease")
* A Release has *Artifacts* that should be built and uploaded:
    * platform-specific "executable-zips" (`ripgrep-1.0.0-x86_64-apple-darwin.tar.xz`)
    * "symbols" (debuginfo/sourcemaps) for those executables (`ripgrep-1.0.0-x86_64-pc-windows-msvc.pdb`)
    * installers (`installer.sh`, `installer.ps1`)
    * machine-readable manifests (`dist-manifest.json`)
* Artifacts may have *Assets* stored inside them:
    * built-assets (`ripgrep.exe`, `ripgrep.pdb`)
    * static-assets (`README.md`, `LICENSE.md`)
* Artifacts may also have a list of *Targets* (triples) that they are intended for (multi-arch binaries/installers are possible)

We'll eventually make this more properly configurable, but currently cargo-dist computes this from a combination of CLI flags and your Cargo workspace:

* Every [binary target](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#binaries) your workspace can build is given its own Application. Properties like "github repository", "README", "version", and so on are inherited from the parent Cargo package (Cargo packages can have multiple binaries they produce, whether that's a good idea is up to you).
* Each Application will get one Release -- for its current version
* Each Release will get the following artifacts:
    * An executable-zip for each target platform (see [Configuring Targets](#configuring-targets))
    * Symbols for each target platform (if the platform supports it, currently only `windows-msvc => pdb` is enabled)
    * Installers if requested (see [Configuring Installers](#configuring-installers))
    * dist-manifest.json describing all of this (emitted on stdout if `--output-format=json` is passed)
* Each executable-zip will automatically include local files with special names like README.md (eventually this will be configurable...)

In the future we might support things like "hey this application actually wants to bundle up several binaries" or "ignore this binary". Similarly we might allow you to specify that multiple versions of an application should be published with different feature flags. This is all up in the air for now, we're just trying to get the simple happy path working right now.

A current key property of cargo-dist's design is that it can compute all of these facts on *any* host platform before running *any* builds. `cargo dist manifest --output-format=json` does exactly this.

(Applications only really exist implicitly -- in practice cargo-dist on really ever talks about Releases, since that's just An Application With A Version, and we always have *some* version.)




# Build Flags

cargo-dist changes a bunch of the default build flags you would get with `cargo build --release`, so here's what we change and why!

Most of the settings we change are baked into your Cargo.toml when you run `cargo dist init` in the form of a `dist` profile. This lets you see them and change them if you disagree with them! Here's the current default:

```toml
[profile.dist]
inherits = "release"
debug = true
split-debuginfo = "packed"
```

* `inherits = "release"` -- release generally has the right idea, so we start with its flags!
* `debug = true` -- enables full debuginfo, which release builds normally disable (because it would bloat the binary)
* `split-debuginfo = "packed"` -- tells the compiler to rip all of the debuginfo it can out of the final binary and put it into a single debuginfo file (aka "symbols", aka "sourcemap")

We also secretly modify RUSTFLAGS as follows (unfortunately not yet configurable):

* on `*-windows-msvc` targets we append `-Ctarget-feature=+crt-static"` to RUSTFLAGS. Unlike other platforms, Microsoft doesn't consider libc ("crt", the C RunTime) to be a fundamental part of the platform. There are more fundamental DLLs on the OS that libc is implemented on top of. As such, libc isn't actually guaranteed to exist on the system, and Microsoft actually *wants* you to statically link it! (Or have an installer wizard which downloads the version you need, which you may have seen a game do for C++ when it says "Installing Visual C++ Redistributable".) Really Rust should have defaulted to this setting but Mistakes Happen so we're fixing it for you. [See The RFC for more details](https://rust-lang.github.io/rfcs/1721-crt-static.html).

In the future we'll probably also turn on these settings:

* `profile.dist.lto="fat"` -- further optimize the binary in a way that's only practical for shippable releases
* `RUSTFLAGS="-Csymbol-mangling-version=v0"` -- use the Fancier symbol mangling that preserves more info for debuggers
* `RUSTFLAGS="-Cforce-frame-pointers=yes"` -- enable frame pointers, making debuggers and profilers more reliable and efficient in exchange for usually-negligible perf losses
* `RUSTFLAGS="--remap-path-prefix=..."` -- try to strip local paths from the debuginfo/binary 

In a similar vein to the `crt-static` change, we may also one day prefer `linux-musl` over `linux-gnu` to produce more portable binaries. Currently the only mechanism we have to do this is "try to run builds on Github's older linux images so the minimum glibc version isn't too high". This is a place where we lack expertese and welcome recommendations! (This is blocked on supporting cross-compilation.)


# Compatibility With Other Tools

cargo-dist can used totally standalone (well, you need Cargo), but is intended to be a cog in various machines. Here's some things that work well with it:

* CI Scripts should be automatically triggered by simple uses of [cargo-release](https://github.com/crate-ci/cargo-release)
* If you set `repository` in your Cargo.toml, then [cargo-binstall](https://github.com/cargo-bins/cargo-binstall) should automagically find, download, and install binaries from the Github Releases™️ we produce without any further configuration
* FUTURE AXODOTDEV TOOL will be able to consume dist-manifest.json and DO COOL THINGS






# Contributing

## Updating Snapshots

cargo-dist's tests rely on [cargo-insta](https://crates.io/crates/cargo-insta) for snapshot testing various
outputs. This allows us to both catch regressions and also more easily review UI/output changes. If a snapshot
test fails, you will need to use the `cargo insta` CLI tool to update them:

```sh
cargo install cargo-insta
```

One installed, you can review and accept the changes with:

```sh
cargo insta review
```

If you know you like the changes, just use `cargo insta accept` to auto-apply all changes.

(If you introduced brand-new snapshot tests you will also have to `git add` them!)

> NOTE: when it succeeds, cargo-dist-schema's `emit` test will actually commit the results back to disk to `cargo-dist-schema/cargo-dist-schema.json` as a side-effect. This is a janky hack to make sure we have that stored and up to date at all times (the test also uses an insta snapshot but insta snapshots include an extra gunk header so it's not something we'd want to link end users). The file isn't even used for anything yet, I just want it to Exist because it seems useful and important. In the future we might properly host it and have our outputs link it via a `$schema` field.



## Cutting Releases

cargo-dist is self-hosting, so just follow the usual [usage instructions](#usage-ci) and publish with `cargo release 1.0.0`! (Or whatever cooler version you prefer.)

The CI is (re)generated with:

```
cargo dist generate-ci
```

**NOTE: if you want to update the version of cargo-dist used in the CI, you need to update cargo-dist-version in the root Cargo.toml! I keep forgetting this and should add a feature to help this this! Yeah! Looking at you, Future Me Who Still Hasn't Done This!**

Note that as a consequence of the way we self-host, cargo-dist's published artifacts will always be built/generated by a previous version of itself. This can be problematic if you make breaking changes to cargo-dist-schema's format... so don't! Many things in the schema are intentionally optional to enable forward and backward compatibility, so this should hopefully work well!

(Future work: mark `cargo release` do more magic like cutting RELEASES.md and whatnot?)
