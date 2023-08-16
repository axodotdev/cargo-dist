# Config

<!-- toc -->

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

cargo-dist has an internal notion of an "artifact download URL" that is required for things like [installers][] that detect the current platform and fetch binaries. If your CI backend is "github" then we will base the "[artifact download URL][artifact-url]" on the "repository" key. To be safe, we will only do this if your workspace agrees on this value. It's fine if only some packages bother setting "repository", as long as the ones that do use the exact same string. If they don't we will fail to compute an "artifact download URL", emit a warning, and ignore your request for installers that require it. (This might want to be a hard error in the future.)

### readme

cargo-dist defaults to trying to include certain "important" static files in your executable-zips. A README is one of them.

If you specify a path to a README file, cargo-dist will use that for all the packages it affects. If you don't, then cargo-dist will search for a README* file in the package's root directory and the workspace's root directory (preferring the package).

### license-file

cargo-dist defaults to trying to include certain "important" static files in your executable-zips. A LICENSE is one of them.

If you specify a path to a license file, cargo-dist will use that for all packages it affects. Otherwise, cargo-dist will search for LICENSE* or UNLICENSE* files in the package's root directory and the workspace's root directory (preferring the package). If multiple are defined in the same directory, we will grab them all (this is necessary for the extremely common dual MIT/Apache license, which often results in two LICENSE-* files). 

Note that the Cargo license-file flag only accepts one path, so it can't handle the dual-license-file case. This cargo feature largely exists as an escape hatch for weird licenses which can't be described by the SPDX format of the "license" field.



## workspace.metadata.dist

Cargo allows other tools to include their own project-wide settings in [metadata tables][workspace-metadata]. The one cargo-dist uses is `[workspace.metadata.dist]`, which must appear in your root Cargo.toml (whether or not it's [virtual][workspace]). You can override them on a per-package basis with `[package.metadata.dist]`, which accepts all the same fields (except for those which must be specified once globally, see the docs for each individual option).

### cargo-dist-version

> since 0.0.3

Example: `cargo-dist-version = "0.0.3"` 

**This can only be set globally**

This is added automatically by `cargo dist init`, and is a recording of its own version for the sake of reproducibility and documentation. When you run [generate-ci][] the resulting CI scripts will use that version of cargo-dist to build your applications.

The syntax must be a valid [Cargo-style SemVer Version][semver-version] (not a VersionReq!).

If you delete the key, generate-ci will just use the version of cargo-dist that's currently running.

### rust-toolchain-version

> since 0.0.3 (deprecated in 0.1.0)

Example: `rust-toolchain-version = "1.67.1"` 

> Deprecation reason: [rust-toolchain.toml](https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file) is a more standard/universal mechanism for pinning toolchain versions for reproducibility. Teams without dedicated release engineers will likely benefit from unpinning their toolchain and letting the underlying CI vendor silently update them to "some recent stable toolchain", as they will get updates/improvements and are unlikely to have regressions.

**This can only be set globally**

This is added automatically by `cargo dist init`, recorded for the sake of reproducibility and documentation. It represents the "ideal" Rust toolchain to build your project with. This is in contrast to the builtin Cargo [rust-version][] which is used to specify the *minimum* supported Rust version. When you run [generate-ci][] the resulting CI scripts will install that version of the Rust toolchain with [rustup][]. There's nothing special about the chosen value, it's just a hardcoded "recent stable version".

The syntax must be a valid rustup toolchain like "1.60.0" or "stable" (should not specify the platform, we want to install this toolchain on all platforms).

If you delete the key, generate-ci won't explicitly setup a toolchain, so whatever's on the machine will be used (with things like rust-toolchain.toml behaving as normal). Before being deprecated the default was to `rustup update stable`, but this is no longer the case.

### ci

> since 0.0.3

Example: `ci = ["github"]`

**This can only be set globally**

This is a list of CI backends you want to support, allowing subsequent runs of [generate-ci][] to know what CI scripts to generate. Its presence also enables certain CI-specific features. For instance if "github" is included we'll try to generate the body for a Github Release and tell [installers][] to fetch binaries from a Github Release.  Once we introduce more CI backends we'll need to more completely rationalize what that means. In all likelihood each set of CI scripts will need to explicitly select just its own CI by passing `--ci=...` for every invocation.

"github" is currently the only supported CI backend.

`cargo dist init` can set this if you pass `--ci=...`

### targets

> since 0.0.3

Example: `targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc"]`

This is a list of [target platforms][platforms] you want your application(s) to be built for. In principle this can be overridden on a per-package basis but that is not well tested.

In v0.0.5 the only properly supported choices are:

* x64 macOS: "x86_64-apple-darwin"
* x64 Windows: "x86_64-pc-windows-msvc"
* x64 Linux: "x86_64-unknown-linux-gnu"
* arm64 macOS (Apple silicon): "aarch64-apple-darwin" (supported added in v0.0.4)

Future versions should hopefully introduce proper support for important targets like "musl linux".

By default all runs of `cargo-dist` will be trying to handle all platforms specified here at once. If you specify `--target=...` on the CLI this will focus the run to only those platforms. As discussed in [concepts][], this cannot be used to specify platforms that are not listed in `metadata.dist`, to ensure different runs agree on the maximum set of platforms.

### installers

> since 0.0.3

Example: `installers = ["shell", "powershell"]`

This is a list of installers you want to be made for your application(s). In principle this can be overridden on a per-package basis but that is not well tested. See [the full docs on installers for the full list of values][installers].

See "repository" for some discussion on the "Artifact Download URL".


### include

> since 0.0.3

Example: `include = ["my-cool-file.txt", "../other-cool-file.txt", "./some/dir/"]`

This is a list of additional *files* or *directories* to copy into the root of all [executable-zips][] that this setting affects. The paths are relative to the directory of the Cargo.toml that you placed this setting in. Globs are not supported.

### auto-includes

> since 0.0.3

Example: `auto-includes = false`

Allows you to specify whether cargo-dist should auto-include README, (UN)LICENSE, and CHANGELOG/RELEASES files in [executable-zips][]. Defaults to true.

### windows-archive

> since 0.0.5

Example: `windows-archive = ".tar.gz"`

Allows you to specify the file format to use for [executable-zips][] that target windows. The default is
".zip". Supported values:

* ".zip"
* ".tar.gz"
* ".tar.xz"
* ".tar.zstd"

See also unix-archive below.

### unix-archive

> since 0.0.5

Example: `unix-archive = ".tar.gz"`

Allows you to specify the file format to use for [executable-zips][] that target not-windows. The default is
".tar.xz". See "windows-archive" above for a complete list of supported values.



### dist

> since 0.0.3

Example: `dist = false`

Specifies whether cargo-dist should ignore this package. It primarily exists as an alternative for `publish=false` or an override for `publish=false`.


### npm-scope

> since 0.0.6

Example `npm-scope = "@axodotdev"`

Specifies that [npm installers][] should be published under the given [scope][]. The leading `@` is mandatory. If you newly enable the npm installer in `cargo dist init`'s interactive UI, then it will give you an opportunity to add the scope.

If no scope is specified the package will be global.


### checksum

> since 0.1.0

Example: `checksum = "sha512"`

Specifies how to checksum [executable-zips][]. Supported values:

* "sha256" (default) - generate a .sha256 file for each archive
* "sha512" - generate a .sha512 file for each archive
* "false" - do not generate any checksums

The hashes should match the result that sha256sum and sha512sum generate. The current format is just a file containing the hash of that file and nothing else.

Future work is planned to [support more robust signed checksums][issue-sigstore].


### precise-builds

> since 0.1.0

Example: `precise-builds = true`

**This can only be set globally**

Build only the required packages, and individually.

[See "inferring precise-builds" for the default behaviour.](#inferring-precise-builds)

By default when we need to build anything in your workspace, we try to build your entire workspace with `--workspace`. This setting tells cargo-dist to instead build each app individually.

On balance, the Rust experts we've consulted with find building with --workspace to be a safer/better default, as it provides some of the benefits of a more manual [workspace-hack][], without the user needing to be aware that this is a thing.

TL;DR: cargo prefers building one copy of each dependency in a build, so if two apps in your workspace depend on e.g. serde with different features, building with --workspace, will build serde once with the features unioned together. However if you build each package individually it will more precisely build two copies of serde with different feature sets.

The downside of using --workspace is that if your workspace has lots of example/test crates, or if you release only parts of your workspace at a time, we build a lot of gunk that's not needed, and potentially bloat up your app with unnecessary features.

If that downside is big enough for you, this setting is a good idea.

[workspace-hack]: https://docs.rs/cargo-hakari/latest/cargo_hakari/about/index.html


#### inferring precise-builds

Although cargo-dist prefers `--workspace` builds ([precise-builds](#precise-builds) = `false`) for the reasons stated above, it *will* attempt to check if that's possible, and use `--package` builds if necessary (`precise-builds = true`).

If you explicitly set `precise-builds = false` and we determine `--package` builds are required, cargo-dist will produce an error. `precise-builds = true` will never produce an error.

Precise-builds are considered required when you use any of [features](#features), [all-features](#all-features), or [default-features](#default-features) *and* not all of the packages in your workspace have the same values set.

So for instance if you have several packages in your workspace and only one sets:

```toml
[package.metadata.dist] 
all-features = true
```

Then we will require precise-builds, and will pass `--all-features` to only the `cargo build` for that package. This setting, on the other hand:

```toml
[workspace.metadata.dist] 
all-features = true
```

Will just make us pass `--all-features` to `cargo build --workspace`.


### merge-tasks

> since 0.1.0

Example: `merge-tasks = true`

**This can only be set globally**

Whether we should try to merge otherwise-parallelizable tasks onto the same machine, sacrificing latency and fault-isolation for more the sake of minor effeciency gains.

For example, if you build for x64 macos and arm64 macos, by default we will generate ci which builds those independently on separate logical machines. With this enabled we will build both of those platforms together on the same machine, making it take twice as long as any other build and making it impossible for only one of them to succeed.

The default is `false`. Before 0.1.0 it was always `true` and couldn't be changed, making releases annoyingly slow (and technically less fault-isolated). This config was added to allow you to restore the old behaviour, if you really want.


### fail-fast

> since 0.1.0

Example: `fail-fast = true`

**This can only be set globally**

Whether failing tasks should make us give up on all other tasks. (defaults to false)

When building a release you might discover that an obscure platform's build is broken. When this happens you have two options: give up on the release entirely (`fail-fast = true`), or keep trying to build all the other platforms anyway (`fail-fast = false`).

cargo-dist was designed around the "keep trying" approach, as we create a draft Release
and upload results to it over time, undrafting the release only if all tasks succeeded.
The idea is that even if a platform fails to build, you can decide that's acceptable
and manually undraft the release with some missing platforms.

(Note that the dist-manifest.json is produced before anything else, and so it will assume
that all tasks succeeded when listing out supported platforms/artifacts. This may make
you sad if you do this kind of undrafting and also trust the dist-manifest to be correct.)

Prior to 0.1.0 we didn't set the correct flags in our CI scripts to do this, but now we do.
This flag was introduced to allow you to restore the old behaviour if you prefer.


### install-path

> since 0.1.0

Example: `install-path = "~/.my-app/"`

The strategy that script installers ([shell][shell-installer], [powershell][powershell-installer]) should use for selecting a path to install things at, with 3 possible syntaxes:

* `CARGO_HOME`: (default) installs as if `cargo install` did it (tries `$CARGO_HOME/bin/`, but if `$CARGO_HOME` isn't set uses `$HOME/.cargo/bin/`). Note that we do not (yet) properly update some of the extra metadata files Cargo maintains, so Cargo may be confused if you ask it to manage the binary.

* `~/some/subdir/`: installs to the given subdir of the user's `$HOME`

* `$SOME_VAR/some/subdir`: installs to the given subdir of the dir defined by `$SOME_VAR`

> NOTE: `$HOME/some/subdir` is technically valid syntax but it won't behave the way you want on Windows, because `$HOME` isn't a proper environment variable. Let us handle those details for you and just use `~/subdir/`.

All of these error out if none of the required env-vars are set to a non-empty value. 

We do not currently sanitize/escape the path components (it's not really a security concern when the user is about to download+run an opaque binary anyway). In the future validation/escaping of this input will become more strict. We do appear to correctly handle spaces in paths on both windows and unix (i.e. `~/My cargo-dist Documents/bin/` works), but we won't be surprised if things misbehave on Interesting Inputs.

Future Improvements:

* In the future [we may expand this setting to allow you to pass an array of options that are tried in sequence until one succeeds](https://github.com/axodotdev/cargo-dist/issues/286).
* In the future [we may support XDG dirs](https://github.com/axodotdev/cargo-dist/issues/287)
* In the future [we may support %windows dirs%](https://github.com/axodotdev/cargo-dist/issues/288)

(Please file an issue if you have other requirements!)


### features

> since 0.2.0

Example: `features = ["serde-support", "fancy-output"]`

Specifies feature-flags that should be passed to a package when building it. This lets you enable features that should be on "in production" but for whatever reason shouldn't be on by default.

For instance for packages that are a library and a CLI binary, some developers prefer to make the library the default and the CLI opt-in. In such a case you would want to add `features = ["cli"]` to your `[package.metadata.dist]`.

If you use this you *probably* want to set it on `[package.metadata.dist]` and
not `[workspace.metadata.dist]`. See ["inferring precise-builds"](#inferring-precise-builds) for details.


### default-features

> since 0.2.0

Example: `default-features = false`

Specifies that default features for a package should be enabled when building it (when set to false, this tells us to pass `--no-default-features` to Cargo).

Defaults true.

If you use this you *probably* want to set it on `[package.metadata.dist]` and not `[workspace.metadata.dist]`. See ["inferring precise-builds"](#inferring-precise-builds) for details.


### all-features

> since 0.2.0

Example: `all-features = true`

Specifies that all features for a package should be enabled when building it (when set to true this tells us to pass `--all-features` to Cargo).

Defaults false.

If you use this you *probably* want to set it on `[package.metadata.dist]` and
not `[workspace.metadata.dist]`. See ["inferring precise-builds"](#inferring-precise-builds) for details.


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
[generate-ci]: ./cli.md#cargo-dist-generate-ci
[semver-version]: https://docs.rs/semver/latest/semver/struct.Version.html
[rust-version]: https://doc.rust-lang.org/cargo/reference/manifest.html#the-rust-version-field
[rustup]: https://rust-lang.github.io/rustup/
[platforms]: https://doc.rust-lang.org/nightly/rustc/platform-support.html
[executable-zips]: ./artifacts.md#executable-zip
[artifact-modes]: ./concepts.md#artifact-modes-selecting-artifacts
[installers]: ./installers.md
[shell-installer]: ./installers.md#shell
[powershell-installer]: ./installers.md#powershell
[artifact-url]: ./installers.md#artifact-download-url
[scope]: https://docs.npmjs.com/cli/v9/using-npm/scope
[npm installers]: ./installers.md#npm
[issue-sigstore]: https://github.com/axodotdev/cargo-dist/issues/120