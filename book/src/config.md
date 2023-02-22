# Config

cargo-dist accepts configuration from the following sources, in order of increasing preference (that is, CLI flags generally replace things specified in your Cargo.toml):

* Relevant [Cargo.toml fields][cargo-manifest] like "repository" and "readme"
* `[workspace.metadata.dist]`
* `[package.metadata.dist]`
* CLI flags

As discussed in [concepts][], all of your config should be persistently stored in the first 3 locations so that every run of cargo-dist agrees on what "build everything" should look like. CLI flags should primarily be used to select *subsets* of that "everything" for an individual run of cargo-dist to care about.

## Relevant Cargo.toml Fields

The [builtin Cargo.toml fields][cargo-manifest] define a lot of things that cargo-dist cares about. Here's the ones that matter:

### name

The name of your package will become the name cargo-dist uses to refer to your package. There is currently no notion of a "prettier display name" (if you have a use for that, let us know!).

### version

The version of your package is used pervasively, and cargo-dist will generally error out if you ask it to build "my-app-1.0.0" when the actual "my-app" package is set to version "1.1.0".

### publish

If you set `publish = false` in your Cargo.toml we will treat this as a hint that cargo-dist should ignore all the affected packages completely. You can override this with dist's own `dist = true` config.

### repository

cargo-dist has an internal notion of an "artifact download URL" that is required for things like installers that detect the current platform and fetch binaries. If your CI backend is "github" then we will base the "artifact download URL" on the "repository" key. To be safe, we will only do this if your workspace agrees on this value. It's fine if only some packages bother setting "repository", as long as the ones that do use the exact same string. If they don't we will fail to compute an "artifact download URL", emit a warning, and ignore your request for installers that require it. (This might want to be a hard error in the future.)

### readme

cargo-dist defaults to trying to include certain "important" static files in your executable-zips. A README is one of them.

If you specify a path to a README file, cargo-dist will use that for all the packages it affects. If you don't, then cargo-dist will search for a README* file in the package's root directory and the workspace's root directory (preferring the package).

### license-file

cargo-dist defaults to trying to include certain "important" static files in your executable-zips. A LICENSE is one of them.

If you specify a path to a license file, cargo-dist will use that for all packages it affects. Otherwise, cargo-dist will search for LICENSE* or UNLICENSE* files in the package's root directory and the workspace's root directory (preferring the package). If multiple are defined in the same directory, we will grab them all (this is necessary for the extremely common dual MIT/Apache license, which often results in two LICENSE-* files). 

Note that the Cargo license-file flag only accepts one path, so it can't handle the dual-license-file case. This cargo feature largely exists as an escape hatch for weird licenses which can't be described by the SPDX format of the "license" field.



## metadata.dist

Cargo allows other tools to include their own project-wide settings in [metadata tables][workspace-metadata]. The one cargo-dist uses is `[workspace.metadata.dist]`, which must appear in your root Cargo.toml (whether or not it's [virtual][workspace]). All settings specified here apply to all packages in your project. You can override them on a per-package basis with `[package.metadata.dist]`, which accepts all the same fields (except for those which must be specified once globally, see the docs for each individual option).

### cargo-dist-version

Example: `cargo-dist-version = "0.0.3"` 

**This can only be set globally**

This is added automatically by `cargo dist init`, and is a recording of its own version for the sake of reproducibility and documentation. When you run [generate-ci][] the resulting CI scripts will use that version of cargo-dist to build your applications.

The syntax must be a valid [Cargo-style SemVer Version][semver-version] (not a VersionReq!).

If you delete the key, generate-ci will just use the version of cargo-dist that's currently running.

### rust-toolchain-version

Example: `rust-toolchain-version = "1.67.1"` 

**This can only be set globally**

This is added automatically by `cargo dist init`, recorded for the sake of reproducibility and documentation. It represents the "ideal" Rust toolchain to build your project with. This is in contrast to the builtin Cargo [rust-version][] which is used to specify the *minimum* supported Rust version. When you run [generate-ci][] the resulting CI scripts will install that version of the Rust toolchain with [rustup][]. There's nothing special about the chosen value, it's just a hardcoded "recent stable version".

The syntax must be a valid rustup toolchain like "1.60.0" or "stable" (should not specify the platform, we want to install this toolchain on all platforms).

If you delete the key, generate-ci will just use "stable" which will drift over time as new stable releases occur.

### ci

Example: `ci = ["github"]`

**This can only be set globally**

This is a list of CI backends you want to support, allowing subsequent runs of [generate-ci][] to know what CI scripts to generate. Its presence also enables certain CI-specific features. For instance if "github" is included we'll try to generate the body for a Github Release and tell installers to fetch binaries from a Github Release.  Once we introduce more CI backends we'll need to more completely rationalize what that means. In all likelihood each set of CI scripts will need to explicitly select just its own CI by passing `--ci=...` for every invocation.

"github" is currently the only supported CI backend.

`cargo dist init` can set this if you pass `--ci=...`

### targets

Example: `targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc"]`

This is a list of [target platforms][platforms] you want your application(s) to be built for. In principle this can be overriden on a per-package basis but that is not well tested.

In v0.0.3 the only properly supported choices are:

* x64 macOS: "x86_64-apple-darwin"
* x64 Windows: "x86_64-pc-windows-msvc"
* x64 Linux: "x86_64-unknown-linux-gnu"

v0.0.4 should hopefully introduce proper support for important targets like "arm64 macos (apple silicon)" and "musl linux".

By default all runs of `cargo-dist` will be trying to handle all platforms specified here at once. If you specify `--target=...` on the CLI this will focus the run to only those platforms. As discussed in [concepts][], this cannot be used to specify platforms that are not listed in `metadata.dist`, to ensure different runs agree on the maximum set of platforms.

### installers

Example: `installers = ["shell", "powershell"]`

This is a list of installers you want to be made for your application(s). In principle this can be overriden on a per-package basis but that is not well tested.

The currently supported values are:

* "shell" (global installer, one per app): a shell script that detects the current platform and fetches and installs binaries from the release's "Artifact Download URL". Ideal for `curl | sh` installation. Currently this always tries to install in `~/.cargo/bin/`.

* "powershell" (global installer, one per app): a powershall script that detects the current platform and fetches and installs binaries from the release's "Artifact Download URL". Ideal for `irm | iex` installation (that's Windows' version of `curl | sh`). Currently this always tries to install in `~/.cargo/bin/`.

See "repository" for some discussion on the "Artifact Download URL".


### include

Example: `include = ["my-cool-file.txt", "../other-cool-file.txt"]`

This is a list of additional *files* (directory support TBD) to copy into the root of all [executable-zips][] that this setting affects. The paths are relative to the directory of the Cargo.toml that you placed this setting in. Globs are not supported.

### auto-includes

Example: `auto-includes = false`

Allows you to specify whether cargo-dist should auto-include README, (UN)LICENSE, and CHANGELOG/RELEASES files in [executable-zips][]. Defaults to true.

### dist

Example: `dist = false`

Specifies whether cargo-dist should ignore this package. It primarily exists as an alternative for `publish=false` or an override for `publish=false`.



## Subsetting CI Flags

Several `metadata.dist` configs have globally available CLI equivalents. These can be used to select a subset of `metadata.dist` list for that run. If you don't pass any, it will be as-if you passed all the values in `metadata.dist`. You can pass these flags multiple times to provide a list. This includes:

* `--target`
* `--installer`
* `--ci`

See [Artifact Modes][artifact-modes] for how you might use this kind of subsetting.

Caveat: the default "host" Artifact Mode does something fuzzier with `--target` to allow you to build binaries that are usable on the current platform. Again see [Artifact Modes][artifact-modes].




[workspace-metadata]: https://doc.rust-lang.org/cargo/reference/workspaces.html#the-metadata-table
[cargo-manifest]: https://doc.rust-lang.org/cargo/reference/manifest.html
[concepts]: ./concepts.md
[workspace]: https://doc.rust-lang.org/cargo/reference/workspaces.html
[semver-version]: https://docs.rs/semver/latest/semver/struct.Version.html
[platforms]: https://doc.rust-lang.org/nightly/rustc/platform-support.html
[executable-zips]: TODO://executable-zips
[artifacts-modes]: TODO://link-concepts-artifact-modes-section