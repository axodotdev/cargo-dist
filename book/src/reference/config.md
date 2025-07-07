# Config

These are the reference docs for configuring dist. [dist init][init] manages the most important of these for you, but if you ever need advanced configuration, this is where to look!

Configuration is currently read from the following sources, in increasing preference:

1. Your language-specific project manifests like [Cargo.toml][rust-guide] or [package.json][js-guide]
2. Your workspace dist config in [dist-workspace.toml][js-guide] or [dist.toml][project-guide]
3. Your package dist config in [dist.toml][project-guide]

We're currently in the middle of [a major config migration](https://github.com/axodotdev/cargo-dist/pull/1247). For [existing Rust users][rust-guide], all references to the `[dist]` section in dist-workspace.toml and dist.toml may also refer to `[workspace.metadata.dist]` or `[package.metadata.dist]` in your Cargo.toml.





[`[dist]`](#the-dist-section)
* [`allow-dirty`](#allow-dirty)
* [`cargo-dist-version`](#cargo-dist-version)
* [`dist`](#dist)
* [`packages`](#packages)
* [`targets`](#targets)
* [`version`](#version)

[artifact settings](#artifact-settings)
* [`checksum`](#checksum)
* [`extra-artifacts`](#extra-artifacts)
* [`source-tarball`](#source-tarball)
* [`ssldotcom-windows-sign`](#ssldotcom-windows-sign)
* [archive settings](#artifact-settings)
    * [`auto-includes`](#auto-includes)
    * [`include`](#include)
    * [`package-libraries`](#package-libraries)
    * [`unix-archive`](#unix-archive)
    * [`windows-archive`](#windows-archive)

[build settings](#build-settings)
* [`dependencies`](#dependencies)
* [cargo build settings](#cargo-build-settings)
    * [`all-features`](#all-features)
    * [`default-features`](#default-features)
    * [`features`](#features)
    * [`min-glibc-version`](#min-glibc-version)
    * [`msvc-crt-static`](#msvc-crt-static)
    * [`precise-builds`](#precise-builds)
    * [`rust-toolchain-version`](#rust-toolchain-version)
    * [`cargo-auditable`](#cargo-auditable)
    * [`cargo-cyclonedx`](#cargo-cyclonedx)
    * [`omnibor`](#omnibor)

[installer settings](#installer-settings)
* [`installers`](#installers)
* [`install-libraries`](#install-libraries)
* [`bin-aliases`](#bin-aliases)
* [`binaries`](#binaries)
* [shell and powershell installer settings](#shell-and-powershell-installer-settings)
    * [`install-success-msg`](#install-success-msg)
    * [`install-path`](#install-path)
    * [`install-updater`](#install-updater)
* [npm installer settings](#npm-installer-settings)
    * [`npm-scope`](#npm-scope)
    * [`npm-package`](#npm-package)
* [homebrew installer settings](#homebrew-installer-settings)
    * [`tap`](#tap)
    * [`formula`](#formula)

[publisher settings](#publisher-settings)
* [`publish-jobs`](#publish-jobs)
* [`publish-prereleases`](#publish-prereleases)

[hosting settings](#hosting-settings)
* [`hosting`](#hosting)
* [`display`](#display)
* [`display-name`](#display-name)
* [`force-latest`](#force-latest)
* [github hosting settings](#github-hosting-settings)
    * [`create-release`](#create-release)
    * [`github-attestations`](#github-attestations)
    * [`github-attestations-phase`](#github-attestations-phase)
    * [`github-attestations-filters`](#github-attestations-filters)
    * [`github-release`](#github-release)
    * [`github-releases-repo`](#github-releases-repo)
    * [`github-releases-submodule-path`](#github-releases-submodule-path)

[ci settings](#ci-settings)
* [`ci`](#ci)
* [`build-local-artifacts`](#build-local-artifacts)
* [`cache-builds`](#cache-builds)
* [`dispatch-releases`](#dispatch-releases)
* [`fail-fast`](#all-features)
* [`merge-tasks`](#merge-tasks)
* [`pr-run-mode`](#pr-run-mode)
* [`tag-namespace`](#tag-namespace)
* [github ci settings](#github-ci-settings)
    * [`github-custom-job-permissions`](#github-custom-job-permissions)
    * [`github-custom-runners`](#github-custom-runners)
    * [`github-build-setup`](#github-build-setup)
    * [`github-action-commits`](#github-action-commits)
* [custom ci jobs](#custom-ci-jobs)
    * [`plan-jobs`](#plan-jobs)
    * [`local-artifacts-jobs`](#local-artifacts-jobs)
    * [`global-artifacts-jobs`](#global-artifacts-jobs)
    * [`host-jobs`](#host-jobs)
    * [`publish-jobs`](#publish-jobs)
    * [`post-announce-jobs`](#post-announce-jobs)

[`[workspace]`](#the-workspace-section)
* [`members`](#workspacemembers)

[`[package]`](#the-package-section)
* [`name`](#packagename)
* [`version`](#packageversion)
* [`description`](#packagedescription)
* [`authors`](#packageauthors)
* [`repository`](#packagerepository)
* [`homepage`](#packagehomepage)
* [`documentation`](#packagedocumentation)
* [`changelog`](#packagechangelog)
* [`readme`](#packagereadme)
* [`license`](#packagelicense)
* [`license-files`](#packagelicense-files)
* [`binaries`](#packagebinaries)
* [`cstaticlibs`](#packagecstaticlibs)
* [`cdylibs`](#packagecdylibs)
* [`build-command`](#packagebuild-command)


-----


# the `[dist]` section

This section represents all the configuration for how dist should build and publish your applications. The `[dist]` section is a temporary placeholder which will soon be replaced (and automatically migrated) to a new hierarchy in [Config 1.0](https://github.com/axodotdev/cargo-dist/pull/1247).


## `allow-dirty`

> <span style="float:right">since 0.3.0<br>[global-only][]</span>
> default = `[]`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> allow-dirty = ["ci", "msi"]
> ```

This is a list of [`generate`][generate] tasks for dist to ignore when checking if generated configuration is up to date.

**We recommend avoiding setting this, as it prevents dist from updating these files for you whenever you update or change your configuration. If you think you need this, please [do file an issue](https://github.com/axodotdev/cargo-dist/issues/new) or ask us about it, so we know what settings we're missing that necessitates this (or ideally, can point you to the existing settings).**

Nevertheless, setting can be necessary for users who customize their own configuration beyond dist's generated defaults and want to avoid dist overwriting it.

Possible values are:

* "ci": don't check/regenerate ci scripts (release.yml)
* "msi": don't check/regenerate msi templates (main.wxs)

## `cargo-dist-version`

> <span style="float:right">since 0.3.0<br>[global-only][]</span>
> default = `<none>` (this is mandatory!)
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> cargo-dist-version = "0.10.0"
> ```

This is added automatically by [`dist init`][init], and is a recording of its own version for the sake of reproducibility and documentation.

Your [release CI][github-ci] will fetch and use the given version of dist to build and publish your project.

The syntax must be a valid [Cargo-style SemVer Version][semver-version] (not a VersionReq!).


## `dist`

> <span style="float:right">since 0.3.0<br>[package-local][]</span>
> [📖 read the guide for this feature!][distribute] \
> default = `<none>` (infer it)
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> dist = true
> ```

Specifies whether dist [should distribute (build and publish) a package][distribute], overriding all other rules for deciding if a package is eligible.

There are 3 major cases where you might use this:

* `dist = false` on a package can be used to force dist to ignore it
* `dist = true` on a package can be used to force dist to distribute it in spite of signals like Cargo's `publish = false` that would suggest otherwise.
* `dist = false` on a whole workspace defaults all packages to do-not-distribute, forcing you to manually allow-list packages with `dist = true` (large monorepos often find this to be a better way of managing project distribution when most developers aren't release engineers).


## `packages`

> <span style="float:right">since 0.29.0<br>[global-only][]</span>
> [📖 read the guide for this feature!][distribute] \
> default = `<none>` (infer it)
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> packages = ["a", "b"]
> ```

`packages` provides a more explicit way of specifying which packages to dist (or not). If `packages` is set, it provides a list of exactly which packages should be distributed within the workspace. It overrides individual package-level `dist = true` or `dist = false` configuration.


## `targets`

> <span style="float:right">since 0.0.3<br>[package-local][]</span>
> [📖 read the guide for this feature!][build-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> targets = [
>    "x86_64-pc-windows-msvc",
>    "x86_64-unknown-linux-gnu",
>    "x86_64-apple-darwin",
>    "aarch64-apple-darwin",
> ]
> ```

This is a list of [target platforms][platforms] you want your packages to be built for.

The supported choices are:

* x64 macOS: "x86_64-apple-darwin"
* x64 Windows: "x86_64-pc-windows-msvc"
* x64 Linux: "x86_64-unknown-linux-gnu"
* arm64 macOS (Apple silicon): "aarch64-apple-darwin"
* arm64 Linux: "aarch64-unknown-linux-gnu"
* x64 Linux (static musl): "x86_64-unknown-linux-musl"
* arm64 Linux (static musl): "aarch64-unknown-linux-musl"

By default all runs of `dist` will be trying to handle all platforms specified here at once. If you specify `--target=...` on the CLI this will focus the run to only those platforms. As discussed in [concepts][], this cannot be used to specify platforms that are not listed in `metadata.dist`, to ensure different runs agree on the maximum set of platforms.


## `version`
> <span style="float:right">since 0.29.0<br>[global-only][]</span>
> default = `<none>` (infer it)
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> version = "0.0.1"
> ```

If set, this value will override the actual version configured for each package. For example, if the workspace contains packages versioned "0.2" and "0.3", and this value is set to "0.1", then dist will consider every package in the workspace to have the version "0.1".


## artifact settings

[Artifacts][artifacts] are the files that will be uploaded to [your hosting][hosting]. These settings affect what files those are, and what they contain. See also [installers](#installer-settings) which are important enough to be separated out from other artifacts.


### `checksum`

> <span style="float:right">since 0.1.0<br>[global-only][]</span>
> [📖 read the checksum guide!](../artifacts/checksums.md) \
> default = `"sha256"`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> checksum = "sha512"
> ```

Specifies how to checksum other [artifacts][artifacts]. Supported values:

* "sha256" - generate a .sha256 file for each archive
* "sha512" - generate a .sha512 file for each archive
* "sha3-256" - generate a .sha3-256 file for each archive
* "sha3-512" - generate a .sha3-512 file for each archive
* "blake2s" - generate a .blake2s file for each archive
* "blake2b" - generate a .blake2b file for each archive
* "false" - do not generate any checksums

The hashes should match the result that sha256sum, sha512sum, etc. generate, and the file should be readable by those sorts of commands.

Future work is planned to [support more robust signed checksums][issue-sigstore].


### `extra-artifacts`

> <span style="float:right">since 0.6.0<br>[package-local][]</span>
> [📖 read the artifacts guide!][artifacts] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [[dist.extra-artifacts]]
> artifacts = ["schema.json"]
> build = ["cargo", "run", "--", "generate-schema"]
>
> [[dist.extra-artifacts]]
> artifacts = ["target/coolsignature.txt", "target/importantfile.xml"]
> build = ["make"]
> ```

(Note the `[[double-square-brackets]]`, you can specify multiple extra-artifacts entries!)

This specifies extra artifacts to build and upload to your [hosting][]. Users can download these directly alongside other [artifacts][] like [archives][] or [installers][].

Each extra-artifacts entry takes the following settings:

* `build`: A command or script to run to produce these artifacts. This is an array of one or more strings; the first string is the command to run, and any subsequent strings are arguments to pass to that command.
* `artifacts`: An array of relative paths to files that dist expects to exist after the `build` command is run. Every file in this list will be uploaded individually to your release as its own artifact.

dist uses this feature to distribute its [`dist-manifest-schema.json`](./schema.md) as part of every release.


### `source-tarball`

> <span style="float:right">since 0.14.0<br>[global-only][]</span>
> [📖 read the artifacts guide!][artifacts] \
> default = `true`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> source-tarball = false
> ```

By default, dist creates and uploads source tarballs from your repository. This setting disables that behaviour. This is especially useful for users who distribute closed-source software to hosts outside their git repos and who would prefer not to distribute source code to their users.


### `recursive-tarball`

> <span style="float:right">since 0.29.0<br>[global-only][]</span>
> [📖 read the artifacts guide!][artifacts] \
> default = `true`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> recursive-tarball = true
> ```

By default, dist's source tarballs only includes the contents of your repository. Setting `recursive-tarball = true` switches to an alternate tarball generation method which includes the content of submodules.


### `ssldotcom-windows-sign`

> <span style="float:right">since 0.14.0<br>[global-only][]</span>
> [📖 read the windows signing guide!](../supplychain-security/signing/windows.md) \
> default = `<none>` (disabled)
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> ssldotcom-windows-sign = "prod"
> ```

If you wish to sign your Windows artifacts ([EXEs][binaries] and [MSIs](../installers/msi.md)) such that Windows SmartScreen won't complain about them, this is the feature for you.

This setting takes one of two values:

* "prod": use the production ssl.com signing service
* "test": use the testing ("sandbox") ssl.com signing service

These strings match the [environment_name setting](https://github.com/SSLcom/esigner-codesign/blob/32825070bd8ca335577862dc735343ae155f2652/README.md#L48) that [SSL.com's code signing action uses](https://github.com/SSLcom/esigner-codesign) uses.


### archive settings

#### `auto-includes`

> <span style="float:right">since 0.0.3<br>[package-local][]</span>
> [📖 read the archives guide!](../artifacts/archives.md) \
> default = `true`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> auto-includes = false
> ```

Specifies whether dist should auto-include README, (UN)LICENSE, and CHANGELOG/RELEASES files in [archives][] and [installers][].

See also: [`include`](#include)


#### `include`

> <span style="float:right">since 0.0.3<br>[package-local][]</span>
> [📖 read the archives guide!](../artifacts/archives.md) \
> default = `[]`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> include = [
>    "my-cool-file.txt",
>    "../other-cool-file.txt",
>    "./some/dir/"
> ]
> ```

This is a list of additional *files* or *directories* to copy into the root of all [archives][] and [installers][]. Paths are relative to the config file. Globs are not supported.

All items specified will be placed in the root of the archive/installer (so in the above example `my-cool-file.txt`, `other-cool-file.txt`, and `dir` would be side-by-side with your binaries in an archive).

See also: [`auto-includes`](#auto-includes)


#### `package-libraries`

> <span style="float:right">since 0.20.0<br>[package-local][]</span>
> 🔧 this is an experimental feature! \
> [📖 read the archives guide!](../artifacts/archives.md) \
> default = `[]`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> package-libraries = ["cdylib", "cstaticlib"]
> ```

Which kinds of [compiled libraries][] to include in [archives][]. By default only [binaries][] will be included in [archives][] and used to decide if a package should be [distributed][distribute]. This feature allows you to opt into bundling static and dynamic libraries that your package builds.

When enabled, libraries will be included in your [archives][] alongside your binaries, but [installers][] will still ignore them. That can be changed using the [`install-libraries`](#install-libraries) setting.


#### `unix-archive`

> <span style="float:right">since 0.0.5<br>[package-local][]</span>
> [📖 read the archives guide!](../artifacts/archives.md) \
> default = `".tar.xz"`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> unix-archive = ".tar.gz"
> ```

Specifies the file format to use for [archives][] that target not-windows.

See [windows-archive](#windows-archive) for a complete list of supported values.


#### `windows-archive`

> <span style="float:right">since 0.0.5<br>[package-local][]</span>
> [📖 read the archives guide!](../artifacts/archives.md) \
> default = `".zip"`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> windows-archive = ".tar.gz"
> ```

Allows you to specify the file format to use for [archives][] that target windows.

Supported values:

* ".zip"
* ".tar.gz"
* ".tar.xz"
* ".tar.zstd" (deprecated for Zstd)
* ".tar.zst" (recommended for Zstd)

See also: [unix-archive](#unix-archive)

## build settings

These settings configure [your builds][build-guide].

### `dependencies`


> <span style="float:right">since 0.4.0<br>[package-local][]</span>
> [📖 read the guide for this feature!][build-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist.dependencies.homebrew]
> cmake = '*'
> libcue = { stage = ["build", "run"] }
>
> [dist.dependencies.apt]
> cmake = '*'
> libcue-dev = { version = "2.2.1-2" }
>
> [dist.dependencies.chocolatey]
> lftp = '*'
> cmake = { version = '3.27.6', targets = ["aarch64-pc-windows-msvc"] }
> ```

Allows specifying dependencies to be installed from a system package manager before the build begins. This is useful if your tool needs certain build tools (say, cmake) or links against C libraries provided by the package manager. This is specified in a Cargo-like format. Dependencies can be specified in two forms:

* A simple form, in which only a version is specified. If any version will do, use `'*'`.
* A complex form, in several extra options can be specified.

Supported options are:

* `version` - A specific version of the package to install. This must be specified in the format that the package manager itself uses. Not used on Homebrew, since Homebrew does not support any method to specify installing specific versions of software.
* `stage` - When exactly dist should make use of this package. Two values are supported: `build`, which specifies that the package should be installed before the build occurs; and `run`, which specifies that the package should be installed alongside your software at the time end users run it. The default is `build`. If `run` is specified for Homebrew dependencies, and you've enabled the Homebrew installer, the Homebrew installer will specify those packages as dependencies.
* `targets` - A set of one or more targets to install the package on, in Rust target-triple format. If not specified, the package is installed on all targets. This is meant as an override to allow a package to be conditionally installed on only certain platforms; for example, a platform may need a build dependency only on Apple Silicon macOS, or have different build dependencies between x86_64 and ARM Windows.

Supported package managers:

* Apt (Linux)
* Chocolatey (Windows)
* Homebrew (macOS)

### cargo build settings

These settings are specific to how we [build your Cargo projects][cargo-build-guide].

#### `all-features`

> <span style="float:right">since 0.2.0<br>[package-local][]</span>
> [📖 read the Cargo project guide!][cargo-build-guide] \
> default = `false`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> all-features = true
> ```

Specifies that all features for a Cargo package should be enabled when building it (when set to true this tells us to pass `--all-features` to Cargo).


#### `default-features`

> <span style="float:right">since 0.2.0<br>[package-local][]</span>
> [📖 read the Cargo project guide!][cargo-build-guide] \
> default = `true`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> default-features = false
> ```

Specifies that default features for a Cargo package should be enabled when building it (when set to false, this tells us to pass `--no-default-features` to Cargo).

#### `features`

> <span style="float:right">since 0.2.0<br>[package-local][]</span>
> [📖 read the Cargo project guide!][cargo-build-guide] \
> default = `[]`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> features = ["serde-support", "fancy-output"]
> ```

Specifies feature-flags that should be passed to a Cargo package when building it. This lets you enable features that should be on "in production" but for whatever reason shouldn't be on by default.

For instance for packages that are a library and a CLI binary, some developers prefer to make the library the default and the CLI opt-in. In such a case you would want to add `features = ["cli"]` to your config.

#### `min-glibc-version`

> <span style="float:right">since 0.26.0<br>[package-local][]</span>
> default = `{}`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist.min-glibc-version]
> # Override glibc version for specific target triplets
> aarch64-unknown-linux-gnu = "2.19"
> x86_64-unknown-linux-gnu = "2.18"
> # Override all remaining glibc versions.
> "*" = "2.17"
> ```

By default, dist will try to auto-detect the glibc version for each build for targets using glibc.

This setting allows you to override the minimum supported glibc version for specific target triplets, in case dist gets it wrong.

The special-cased `"*"` key will allow you to override the minimum supported glibc version for all targets that are not individually overridden.

Note that this setting only affects builds for Linux targets using the GNU libc (glibc). Non-Linux targets, or targets using another libc are not affected.

#### `msvc-crt-static`

> <span style="float:right">since 0.4.0<br>[global-only][]</span>
> [📖 read the Cargo project guide!][cargo-build-guide] \
> default = `true`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> msvc-crt-static = false
> ```

Specifies how The C Runtime (CRT) should be linked when building a Cargo package for Windows. Rust defaults to this being `= false` (dynamically link the CRT), but dist actually defaults to making this `= true` (statically link the CRT). [The Rust default is mostly a historical accident, and it's widely regarded to be an error that should one day be changed][crt-static]. Specifically it's a mistake for the typical Rust application which statically links everything else, because Windows doesn't actually guarantee that the desired things are installed on all machines by default, and statically linking the CRT is a supported solution to this issue.

However when you *do* want a Rust application that dynamically links more things, it then becomes correct to dynamically link the CRT so that your app and the DLLs it uses can agree on things like malloc. However Rust's default is still insufficient for reliably shipping such a binary, because you really should also bundle a "Visual C(++) Redistributable" with your app that installs your required version of the CRT. The only case where it's *probably* fine to not do this is when shipping tools for programmers who probably already have all of that stuff installed (i.e. anyone who installs the Rust toolchain will have that stuff installed).

This config exists as a blunt way to return to the default Rust behaviour of dynamically linking the CRT if you really want it, but more work is needed to handle Redistributables for that usecase.

[See this issue for details and discussion][issue-msvc-crt-static].

#### `precise-builds`

> <span style="float:right">since 0.1.0<br>[global-only][]</span>
> [📖 read the Cargo project guide!][cargo-build-guide] \
> default = `false`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> precise-builds = true
> ```

Build only the required Cargo packages, and individually.

[See "inferring precise-builds" for the default behaviour.](#inferring-precise-builds)

By default when we need to build anything in your workspace, we try to build your entire workspace with `--workspace`. This setting tells dist to instead build each app individually.

On balance, the Rust experts we've consulted with find building with --workspace to be a safer/better default, as it provides some of the benefits of a more manual [workspace-hack][], without the user needing to be aware that this is a thing.

TL;DR: cargo prefers building one copy of each dependency in a build, so if two apps in your workspace depend on e.g. serde with different features, building with --workspace, will build serde once with the features unioned together. However if you build each package individually it will more precisely build two copies of serde with different feature sets.

The downside of using --workspace is that if your workspace has lots of example/test crates, or if you release only parts of your workspace at a time, we build a lot of gunk that's not needed, and potentially bloat up your app with unnecessary features.

If that downside is big enough for you, this setting is a good idea.

[workspace-hack]: https://docs.rs/cargo-hakari/latest/cargo_hakari/about/index.html


##### inferring precise-builds

Although dist prefers `--workspace` builds ([precise-builds](#precise-builds) = `false`) for the reasons stated above, it *will* attempt to check if that's possible, and use `--package` builds if necessary (`precise-builds = true`).

If you explicitly set `precise-builds = false` and we determine `--package` builds are required, dist will produce an error. `precise-builds = true` will never produce an error.

Precise-builds are considered required when you use any of [features](#features), [all-features](#all-features), or [default-features](#default-features) *and* not all of the packages in your workspace have the same values set.

So for instance if you have several packages in your workspace and only one sets `all-features = true`, then we will require precise-builds, and will pass `--all-features` to only the `cargo build` for that package.

If we instead set `all-features = true` on the workspace, then we will just pass `--all-features` to `cargo build --workspace`.


#### `rust-toolchain-version`

> <span style="float:right">since 0.0.3<br>[global-only][]</span>
> ⚠️ deprecated in 0.1.0 \
> [📖 read the Cargo project guide!][cargo-build-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> rust-toolchain-version = "1.67.1"
> ```

> Deprecation reason: [rust-toolchain.toml](https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file) is a more standard/universal mechanism for pinning toolchain versions for reproducibility. Teams without dedicated release engineers will likely benefit from unpinning their toolchain and letting the underlying CI vendor silently update them to "some recent stable toolchain", as they will get updates/improvements and are unlikely to have regressions.

This represents the "ideal" Rust toolchain to build your Cargo packages with. This is in contrast to the builtin Cargo [rust-version][] which is used to specify the *minimum* supported Rust version. Your CI scripts will install that version of the Rust toolchain with [rustup][].

The syntax must be a valid rustup toolchain like "1.60.0" or "stable" (should not specify the platform, we want to install this toolchain on all platforms).

Without this setting, CI won't explicitly setup a toolchain, so whatever's on the machine will be used (with things like rust-toolchain.toml behaving as normal).

#### `cargo-auditable`

> <span style="float:right">since 0.26.0<br>[package-local][]</span>
> default = `false`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> cargo-auditable = true
> ```

Specifies whether to use [`cargo auditable`](https://github.com/rust-secure-code/cargo-auditable) to embed metadata about your dependency tree into the built executables.
When this value is false, dist will run `cargo build`; when it is true, dist will run `cargo auditable build`.

You can then use [`cargo audit`](https://github.com/rustsec/rustsec/blob/main/cargo-audit/README.md) to audit your dependencies for security vulnerabilities that have been reported to the [RustSec Vulnerability Database](https://rustsec.org/).

#### `cargo-cyclonedx`

> <span style="float:right">since 0.26.0<br>[package-local][]</span>
> default = `false`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> cargo-cyclonedx = true
> ```

Specifies whether to use [`cargo cyclonedx`](https://github.com/CycloneDX/cyclonedx-rust-cargo) to generate and upload a Software Bill Of Materials (SBOM) for each project in a workspace.

#### `omnibor`

> <span style="float:right">since 0.26.0<br>[package-local][]</span>
> default = `false`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> omnibor = true
> ```

Specifies whether to use [`omnibor-cli`](https://github.com/omnibor/omnibor-rs/tree/main/omnibor-cli) to generate and upload [OmniBOR Artifact IDs](https://omnibor.io/docs/artifact-ids/) for artifact in a release.

## installer settings

Installers [main installer docs][installers]!

### `installers`

> <span style="float:right">since 0.0.3<br>[package-local][]</span>
> [📖 read the installer guides!][installers] \
> default = `[]`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> installers = [
>   "shell",
>   "powershell",
>   "npm",
>   "homebrew",
>   "msi"
> ]
> ```

This is a list of [installers][] you want for your packages.

Possible values:

* ["shell": a curl-sh script for unixy systems][shell-installer]
* ["powershell": an irm-iex script for Windows][powershell-installer]
* ["npm": an npm package that runs prebuilt binaries][npm-installer]
* ["homebrew": a Homebrew formula][homebrew-installer]
* ["msi": a Windows MSI installer][msi-installer]


### `bin-aliases`

> <span style="float:right">since 0.14.0<br>[package-local][]</span>
> [📖 read the guide for this feature!][build-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist.bin-aliases]
> mybin = ["somealias"]
> myotherbin = ["someotheralias", "anotheralias"]
> ```

This is a map of binary names to aliases that your [installers][] should create for those [binaries][]. These aliases aren't included in your [archives][], and are only created by the installers themselves. The way the alias is created is installer-specific, and may change in the future. Currently:

* [shell][shell-installer]: symlink
* [powershell][powershell-installer]: hardlink
* [npm][npm-installer]: extra "bins" pointing at the same command
* [homebrew][homebrew-installer]: bin.install_symlink
* [msi][msi-installer]: **not currently supported**


### `binaries`

> <span style="float:right">since 0.29.0<br>[package-local][]</span>
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist.binaries]
> x86_64-pc-windows-msvc = ["a", "b"]
> ```

This setting allows overriding the list of binaries to install on a per-platform basis.


### `install-libraries`

> <span style="float:right">since 0.20.0<br>[package-local][]</span>
> 🔧 this is an experimental feature! \
> [📖 read the guide for this feature!][build-guide] \
> default = `[]`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> install-libraries = ["cdylib", "cstaticlib"]
> ```

**To use this feature, you must also enable the [package-libraries](#package-libraries) setting.**

Which kinds of [compiled libraries][] to unpack and installer with your [installers][].  When enabled, libraries will be installed alongside a package's [binaries][].

When using [shell][shell-installer] and [powershell][powershell-installer] installers The currently-supported [install-paths](#install-path) will place libraries alongside binaries. This means they may appear in the user's `$PATH`, which you may find undesirable, and we may change it.


### shell and powershell installer settings

These settings are specific to the [shell][shell-installer] and [powershell][powershell-installer] installers, which provide a `curl | sh` installer for unix, and the equivalent `irm | iex` for windows. The two largely support the same things and behave the same, and typically want to be configured and enabled together.

#### `install-success-msg`

> <span style="float:right">since 0.15.0<br>[package-local][]</span>
> 📖 read the [shell][shell-installer] and [powershell][powershell-installer] installer guides!\
> default = `"everything's installed!"`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> install-success-msg = "axolotlsay is ready to rumble! >o_o<"
> ```

The message to display on success in the [shell][shell-installer] and [powershell][powershell-installer] installers.


#### `install-path`

> <span style="float:right">since 0.1.0<br>[package-local][]</span>
> 📖 read the [shell][shell-installer] and [powershell][powershell-installer] installer guides!\
> default = `"CARGO_HOME"`
>
> *in your dist-workspace.toml or dist.toml:*s
> ```toml
> [dist]
> install-path = "~/.my-app/"
> ```
>
> ```toml
> [dist]
> install-path = ["$MY_APP_HOME/bin", "~/.my-app/bin"]
> ```

The strategy that script installers ([shell][shell-installer], [powershell][powershell-installer]) should use for selecting a path to install things at, with 3 possible syntaxes:

* "CARGO_HOME": installs as if `cargo install` did it (tries `$CARGO_HOME/bin/`, but if `$CARGO_HOME` isn't set uses `$HOME/.cargo/bin/`). Note that we do not (yet) properly update some of the extra metadata files Cargo maintains, so Cargo may be confused if you ask it to manage the binary.

* "~/some/subdir/": installs to the given subdir of the user's `$HOME`

* "$SOME_VAR/some/subdir": installs to the given subdir of the dir defined by `$SOME_VAR`

> NOTE: `$HOME/some/subdir` is technically valid syntax but it won't behave the way you want on Windows, because `$HOME` isn't a proper environment variable. Let us handle those details for you and just use `~/subdir/`.

All of these error out if none of the required env-vars are set to a non-empty value. Since 0.14.0 you can provide an array of options to try if all the previous ones fail. Such an "install-path cascade" would typically be used to provide an environment variable for changing the install dir, with a more hardcoded home subdir as a fallback:

```toml
install-path = ["$MY_APP_HOME/bin", "~/.my-app/bin"]
```

It hasn't yet been tested whether this is appropriate to pair with things like `$XDG_BIN_HOME`, but we'd sure like it to be.

We do not currently sanitize/escape the path components (it's not really a security concern when the user is about to download+run an opaque binary anyway). In the future validation/escaping of this input will become more strict. We do appear to correctly handle spaces in paths on both windows and unix (i.e. `~/My dist Documents/bin/` works), but we won't be surprised if things misbehave on Interesting Inputs.

Future Improvements:

* In the future [we may support XDG dirs](https://github.com/axodotdev/cargo-dist/issues/287)
* In the future [we may support %windows dirs%](https://github.com/axodotdev/cargo-dist/issues/288)
* For historical reasons `CARGO_HOME` [uses a slightly different install dir structure from the others](https://github.com/axodotdev/cargo-dist/issues/934), and so for safety cannot be paired with the others strategies in an install-path cascade.

(Please file an issue if you have other requirements!)


#### `install-updater`

> <span style="float:right">since 0.12.0<br>[global-only][]</span>
> [📖 read the updater guide!][updater] \
> default = `false`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> install-updater = true
> ```

Determines whether to install a standalone updater program alongside your program when using the [shell][shell-installer] or [powershell][powershell-installer] installers. This program will be named `yourpackage-update`, and can be run by the user to automatically check for newer versions and install them without needing to visit your website.

Users who received your package from a package manager, such as [Homebrew][homebrew-installer] or [npm][npm-installer], will need to use the same package manager to perform upgrades.

This updater is the commandline tool contained in the open source [axoupdater][] package.


### npm installer settings

These settings are specific to the [npm installer][npm-installer].

#### `npm-scope`

> <span style="float:right">since 0.0.6<br>[package-local][]</span>
> [📖 read the npm installer guide!][npm-installer] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> npm-scope = "@axodotdev"
> ```

Specifies that [npm installers][npm-installer] should be published under the given [scope][]. The leading `@` is mandatory. If you newly enable the npm installer in `dist init`'s interactive UI, then it will give you an opportunity to add the scope.

If no scope is specified the package will be global.

See also: [npm-package](#npm-package)


#### `npm-package`

> <span style="float:right">since 0.14.0<br>[package-local][]</span>
> [📖 read the npm installer guide!][npm-installer] \
> default = [package.name](#packagename)
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> npm-package = "mycoolapp"
> ```

Specifies that an [npm installer][npm-installer] should be published under the given name, as opposed to the [name of the package](#packagename) they are defined by.

This does not set the [scope][] the package is published under, for that see [npm-scope](#npm-scope).


### homebrew installer settings

These settings are specific to the [homebrew installer][homebrew-installer].

#### `tap`

> <span style="float:right">since 0.2.0<br>[global-only][]</span>
> [📖 read the homebrew installer guide!][homebrew-installer] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> tap = "axodotdev/homebrew-tap"
> ```

This is the name of a GitHub repository which dist should publish the Homebrew installer to. It must already exist, and the token which creates releases must have write access.

It's conventional for the repo name to start with `homebrew-`.


#### `formula`

> <span style="float:right">since 0.11.0<br>[package-local][]</span>
> [📖 read the homebrew installer guide!][homebrew-installer] \
> default = [package.name](#packagename)
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> formula = "axolotlbrew"
> ```

Specifies the Homebrew formula name for a package, as opposed to the [package's name](#packagename).

This works well specifically for folks who are customizing their bin name and would like the Homebrew formula to match the bin name as opposed to the package name.


## publisher settings

These settings are specific to how we publish your packages to package managers like [homebrew taps][homebrew-installer] and [npm][npm-installer].

### `publish-prereleases`

> <span style="float:right">since 0.2.0<br>[global-only][]</span>
> default = `false`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> publish-prereleases = true
> ```

If you set `publish-prereleases = true`, dist will [publish](#publish-jobs) prerelease versions to package managers such as [homebrew][homebrew-installer] and [npm][npm-installer]. By default, dist will only publish stable versions to avoid polluting your releases. This is especially important for things like Homebrew which don't really have a proper notion of "prereleases" or "literally having more than one published version of a package".


## hosting settings

These settings govern how we host your files with platforms like [GitHub Releases][github-releases-guide], and the text we tell them to display about your releases.

### `hosting`

> <span style="float:right">since 0.5.0<br>[global-only][]</span>
> [📖 read the releases guide!][github-releases-guide] \
> default = `<none>` (infer based on [ci](#ci))
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> hosting = ["axodotdev", "github"]
> ```

Possible values:

* `github`: Use GitHub Releases (default if ci = "github")

Specifies what hosting provider to use when hosting/announcing new releases.

By default we will automatically use the native hosting of your CI provider, so when running on GitHub CI, we'll default to using GitHub Releases for hosting/announcing.


### `display`

> <span style="float:right">since 0.16.0<br>[package-local][]</span>
> [📖 read the releases guide!][github-releases-guide] \
> default = `true`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> display = false
> ```

Specifies whether this package should be displayed in release bodies of [hosting providers](#hosting) (like GitHub Releases). This is useful for hiding things that aren't the "primary" or "featured" application but still need to be included in the release for logistical reasons.


### `display-name`

> <span style="float:right">since 0.16.0<br>[package-local][]</span>
> [📖 read the releases guide!][github-releases-guide] \
> default = [package.name](#packagename)
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> display-name = "my cool app"
> ```

Specifies how to refer to the package in release bodies of [hosting providers](#hosting) (like GitHub Releases). This is useful for situations where the package name *must* have a certain value for logistical reasons but you want to refer to it by a nicer name in marketing materials.


### `force-latest`

> <span style="float:right">since 0.16.0<br>[global-only][]</span>
> [📖 read the releases guide!][github-releases-guide] \
> default = `false`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> force-latest = true
> ```

Overrides dist's default handling of prerelease versions. Ordinarily, dist uses [semver](https://semver.org) rules to determine if a version number is a prerelease or not and has some special handling if it is. With this setting, dist will always consider a version to be the latest no matter what its version number is.

This means that the following prerelease handling behaviour will **no longer apply**:

* If dist interprets a version as a prerelease, it will [publish it to GitHub Releases](#hosting) as a "prerelease" instead of the "latest" release.
* dist will not publish prereleases to [Homebrew][homebrew-installer] or [npm][npm-installer] by default.

See also: [`publish-prereleases`](#publish-prereleases)


### github hosting settings

These settings govern how we host your files on [GitHub Releases][github-releases-guide] and the text we tell them to display.

#### `github-attestations`

> <span style="float:right">since 0.16.0<br>[global-only][]</span>
> 🔧 this is an experimental feature! \
> [📖 read the guide for this feature!](../supplychain-security/attestations/github.md) \
> default = `false`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> github-attestations = true
> ```

If you're using GitHub Releases, this will enable GitHub's experimental artifact attestation feature.

#### `github-attestations-phase`

> <span style="float:right">since 0.30.0<br>[global-only][]</span>
> 🔧 this is an experimental feature! \
> [📖 read the guide for this feature!](../supplychain-security/attestations/github.md) \
> default = `"build-local-artifacts"`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> github-attestations-phase = "host"
> ```

Possible values:

* `host`: Create the GitHub Attestations during the `host` phase.
* `build-local-artifacts`: Create the GitHub Attestations during the `build-local-artifacts` phase (default).


#### `github-attestations-filters`

> <span style="float:right">since 0.30.0<br>[global-only][]</span>
> 🔧 this is an experimental feature! \
> [📖 read the guide for this feature!](../supplychain-security/attestations/github.md) \
> default = `["*"]`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> github-attestations-filters = ["*.json", "*.sh", "*.ps1", "*.zip", "*.tar.gz"]
> ```

Allows filtering GitHub Attestations in the `host` phase. All patterns are globed against the pattern `artifacts/{filter}`.

#### `github-release`

> <span style="float:right">since 0.17.0<br>[global-only][]</span>
> [📖 read the releases guide!][github-releases-guide] \
> default = "auto"
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> github-release = "announce"
> ```

Possible values:

* `auto`: create the GitHub Release whenever is best
* `host`: create the GitHub Release during the host step
* `announce`: create the GitHub Release during the announce step

Controls which stage of the release process the GitHub Release will be created in.

By default, the GitHub Release is created during the "host" phase, as it hosts the files some installers will try to download. **Most users should be well-served by the default setting, and changing it is likely to introduce undesirable publishing race conditions.** The only reason you might want to override this setting is if you're using [`dispatch-releases = true`](#dispatch-releases) and you really want your git tag to be the last operation in your release process (because creating a GitHub Release necessarily creates the git tag if it doesn't yet exist). In this case setting github-release = "announce" will accomplish that, but see below for what race conditions this might introduce.

If using only GitHub Releases, and you force it to run during "announce", there will be a very brief window (~30 seconds) during which generated [Homebrew][homebrew-installer] and [npm][npm-installer] installers are live and referencing URLs that will only exist when the GitHub Release is created, causing the packages to error out when installed.

However, if you're publishing only packages that don't reference hosted artifacts (such as Cargo crates, or any custom publish job that fully embeds the binaries), then there is no race, and you could consider changing the default. That said, it would be a looming footgun if you ever introduce new publish jobs and forget about this.



#### `github-releases-repo`

> <span style="float:right">since 0.14.0<br>[global-only][]</span>
> [📖 read the releases guide!][github-releases-guide] \
> default = `<none>` (use the project's own repository)
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> github-releases-repo = "myorg/public"
> ```

Allows specifying a different repo to publish GitHub Releases to than the current one. This can be used to publish to a public repo from a private one. Let's assume you want to publish to `myorg/public` from `myorg/private`. Then in your config in `myorg/private`, you'd set `github-releases-repo = "myorg/public"`.

To ensure the workflow has permission to do this, you need to create a [GitHub Personal Access Token with the "repo" scope](https://github.com/settings/tokens/new?scopes=repo) that can access `myorg/public`. This must be added as a GitHub SECRET called `GH_RELEASES_TOKEN` on `myorg/private`.

GitHub Releases isn't really designed for this, so there's a few strange things that will happen here:

* GitHub Releases always requires a commit to be tagged, and in this case the tag would be on `myorg/public` even though the workflow is running on `myorg/private`, which (presumably) has unrelated commits. Currently **we will tag the latest commit on the [default branch](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/about-branches#about-the-default-branch) of `myorg/public`**. If you're using [the dispatch-releases flow](#dispatch-releases), no tag will be created on `myorg/private`.

* GitHub Releases will provide a source tarball pointing at the tagged commit on `myorg/public`, but that's (presumably) not the source that your release was actually built from. This cannot be disabled, but it's also essentially harmless. However **dist uploads its own source tarball and that *WILL* contain the source of the private repo**. If you don't want this, use [the `source-tarball = false` setting](#source-tarball).


#### `github-releases-submodule-path`

> <span style="float:right">since 0.15.0<br>[global-only][]</span>
> [📖 read the releases guide!][github-releases-guide] \
> default = `<none>` (use the project's root repository)
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> github-releases-submodule-path = "rel/path/to/submodule"
> ```

Designed for use with [github-releases-repo](#github-releases-repo) setting. When specified, the cached commit of the submodule at this path will be used as the commit to tag in the target repository. If not specified, the latest commit in the target repository will be used instead.


#### `create-release`

> <span style="float:right">since 0.2.0<br>[global-only][]</span>
> [📖 read the releases guide!][github-releases-guide] \
> default = `true`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> create-release = false
> ```

Whether we should create the GitHub Release for you in your Release CI.

If true, dist will create a new GitHub Release and generate
a title/body for it based on your changelog.

If false, dist will assume a draft GitHub Release for the current git tag
already exists with the title/body you want, and just upload artifacts to it, undrafting when all artifacts are uploaded.

See also: [`github-release`](#github-release)


## ci settings

These settings govern how [your CI should work][github-ci], including how to trigger the release process and custom tasks to run.

### `ci`

> <span style="float:right">since 0.0.3<br>[global-only][]</span>
> [📖 read the ci customization guide!][github-ci] \
> default = `[]`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> ci = ["github"]
> ```

This is a list of CI backends you want to support, allowing dist to know what CI scripts to generate. Most dist features require this to be enabled!

["github"][github-ci] is currently the only supported CI backend.

### `build-local-artifacts`

> <span style="float:right">since 0.8.0<br>[global-only][]</span>
> [📖 read the ci customization guide!][github-ci] \
> default = `true`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> build-local-artifacts = false
> ```

`build-local-artifacts = false` disables the builtin CI jobs that would build your binaries and archives (and MSI installers). This allows a Sufficiently Motivated user to use custom `build-local-jobs` to completely replace dist's binary building with something like maturin.

The requirements are simply that you need your custom actions to:

* build archives (tarballs/zips) and checksums that the local CI was expected to produce
* use the github upload-artifacts action to upload all of those to an artifact named `artifacts`

You can get a listing of the exact artifact names to use and their expected contents with:

```
dist manifest --artifacts=local --no-local-paths
```

(`[checksum]` entries are separate artifacts and not actually stored in the archives.)

Also note that for legacy reasons a tarball is expected to have all the contents nested under a root dir with the same name as the tarball (sans extension), while zips are expected to have all the files directly in the root (installers pass `--strip-components=1` to tar when extracting).


### `cache-builds`

> <span style="float:right">since 0.18.0<br>[global-only][]</span>
> [📖 read the ci customization guide!][github-ci] \
> default = `<none>` (inferred, probably `false`)
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> cache-builds = true
> ```

Determines whether CI will try to cache work between builds. Defaults false, unless [`release-branch`](#release-branch) or [`pr-run-mode = "upload"`](#pr-run-mode) are enabled.

This is unlikely to be productive because for safety the cache aggressively invalidates based on things like "Cargo.toml or Cargo.lock changed" (which is always true if you change the version of a Rust project), and a noop cache run can randomly take over 2 minutes (typically more like 10 seconds).

The cases where we enable it by default are the only ones we know where you *might* want to enable it.


### `dispatch-releases`

> <span style="float:right">since 0.8.0<br>[global-only][]</span>
> [📖 read the ci customization guide!][github-ci] \
> default = `false`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> dispatch-releases = true
> ```

When enabled, your release CI is triggered with workflow_dispatch instead of tag-push (relying on creating a GitHub release implicitly tagging).

Enabling this disables tag-push releases, but keeps pr checks enabled.

By default the workflow dispatch form will have "dry-run" populated as the tag, which is taken to have the same meaning as [`pr-run-mode = "upload"`](#pr-run-mode): run the plan and build steps, but not the publish or announce ones. Currently hosting is also disabled, but future versions may add some forms of hosting in this mode.


### `fail-fast`

> <span style="float:right">since 0.1.0<br>[global-only][]</span>
> [📖 read the ci customization guide!][github-ci] \
> default = `false`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> fail-fast = true
> ```

Whether failing builds tasks should make us give up on all other build tasks.

When building a release in CI, you might discover that one platform's build is broken. When this happens you have two options: kill all other builds immediately (`fail-fast = true`), or keep trying to build all the other platforms anyway (`fail-fast = false`) to see what other platforms might have problems.

Either way, the global build task will refuse to run if any of these tasks fail, so you can't get any kind of partial release. However, if the build failure was spurious, resuming all failed tasks should resume without issue.


### `pr-run-mode`

> <span style="float:right">since 0.3.0<br>[global-only][]</span>
> [📖 read the ci customization guide!][github-ci] \
> default = `"plan"`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> pr-run-mode = "upload"
> ```

This setting determines to what extent we run your Release CI on pull-requests:

* "skip": don't check the release process in PRs
* "plan": run 'dist plan' on PRs (recommended, also the default)
* "upload": build and upload an artifacts to the PR (expensive)


### `tag-namespace`

> <span style="float:right">since 0.10.0<br>[global-only][]</span>
> [📖 read the ci customization guide!][github-ci] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> tag-namespace = "some-prefix"
> ```

Setting `tag-namespace = "owo"` will change the tag matching expression we put in your GitHub CI, to require the tag to start with "owo" for dist to care about it. This can be useful for situations where you have several things with different tag/release workflows in the same workspace. It also renames `release.yaml` to `owo-release.yml` to make it clear it's just one of many release workflows.

**NOTE**: if you change tag-namespace, dist will generate the new `owo-release.yml` file, but not delete the old one. Be sure to manually delete the old `release.yml`!


### `merge-tasks`

> <span style="float:right">since 0.1.0<br>[global-only][]</span>
> [📖 read the ci customization guide!][github-ci] \
> default = `false`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> merge-tasks = true
> ```

Whether we should try to merge otherwise-parallelizable tasks onto the same machine, sacrificing latency and fault-isolation for the sake of minor efficiency gains.

For example, if you build for x64 macos and arm64 macos, by default we will generate ci which builds those independently on separate logical machines. With this enabled we will build both of those platforms together on the same machine, making it take twice as long as any other build and making it impossible for only one of them to succeed.

The default is `false`. Before 0.1.0 it was always `true` and couldn't be changed, making releases annoyingly slow (and technically less fault-isolated). This setting was added to allow you to restore the old behaviour, if you really want.



### github ci settings

These settings are specific to [your dist GitHub CI][github-ci].

#### `github-build-setup`

> <span style="float:right">since 0.20.0<br>[global-only][]</span>
> 🔧 this is an experimental feature! \
> [📖 read the ci customization guide!][github-ci] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> github-build-setup = "path/to/build-setup.yml"
> ```

This configuration value should be a path relative to the repository your `.github/workflows` directory.
The file located at that path should contain a yaml array of [steps][github-workflow-step] which will be
performed before we call `dist build`.


#### `github-custom-job-permissions`

> <span style="float:right">since 0.18.0<br>[global-only][]</span>
> [📖 read the ci customization guide!][github-ci] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> publish-jobs = ["npm", "./my-custom-publish"]
> github-custom-job-permissions = { "my-custom-publish" = { packages = "admin" } }
> ```

Allows you to customize the permissions given to your custom CI jobs.

By default all custom `publish-jobs` get `{ id-token = "write", packages = "write" }`.
If you override a publish job's permissions, the default permissions will be removed.
All other custom jobs default to no special permissions.



#### `github-custom-runners`

> <span style="float:right">since 0.6.0<br>[global-only][]</span>
> [📖 read the ci customization guide!][github-ci] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist.github-custom-runners]
> aarch64-unknown-linux-gnu = "buildjet-8vcpu-ubuntu-2204-arm"
> aarch64-unknown-linux-musl = "buildjet-8vcpu-ubuntu-2204-arm"
> ```

Allows specifying which runner to use for a target. The keys within this table are target triples in the same format as the ["targets"](#targets) setting. Any targets not specified in this table will use the defaults.

In addition to defining runners for a target, it's also possible to specify a runner for the global, non-target-specific tasks using the `global` key. This runner will be used for tasks like `plan`, `host`, generating installers, and so on.


### `github-action-commits`

> <span style="float:right">since 0.29.0<br>[global-only][]</span>
> [📖 read the ci customization guide!][github-ci] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist.github-action-commits]
> "actions/checkout" = "11bd71901bbe5b1630ceea73d27597364c9af683"
> ```

Allows overriding which version of a GitHub Action to use. This can be useful to replace the default set of tags used by dist with a specific pinned set of commits.


### custom ci jobs

These settings all similarly extend [your dist GitHub CI][github-ci] with custom jobs to run at specific steps of the release process, which looks like:

1. plan: check settings, decide what we're releasing
2. build-local: compile things for each platform
3. build-global: combine things and generate installers
4. host: upload files to hosting, make URLs live
5. publish: publish to package managers
6. announce: announce to the world that the release was a success

#### `plan-jobs`

> <span style="float:right">since 0.7.0<br>[global-only][]</span>
> [📖 read the ci customization guide!][github-ci] \
> default = `[]`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> plan-jobs = ["./my-job"]
> ```


This setting determines which custom jobs to run during the "plan" phase, which happens at the very start of the build.

#### `local-artifacts-jobs`

> <span style="float:right">since 0.7.0<br>[global-only][]</span>
> [📖 read the ci customization guide!][github-ci] \
> default = `[]`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> local-artifacts-jobs = ["./my-job"]
> ```

This setting determines which custom jobs to run during the "build local artifacts" phase, during which binaries are built.


#### `global-artifacts-jobs`

> <span style="float:right">since 0.7.0<br>[global-only][]</span>
> [📖 read the ci customization guide!][github-ci] \
> default = `[]`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> global-artifacts-jobs = ["./my-job"]
> ```

This setting determines which custom jobs to run during the "build global artifacts" phase, during which installers are built.


#### `host-jobs`

> <span style="float:right">since 0.7.0<br>[global-only][]</span>
> [📖 read the ci customization guide!][github-ci] \
> default = `[]`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> host-jobs = ["./my-job"]
> ```

This setting determines which custom jobs to run during the "host" phase, during which dist decides whether to proceed with publishing the release.


#### `publish-jobs`

> <span style="float:right">since 0.2.0<br>[global-only][]</span>
> [📖 read the ci customization guide!][github-ci] \
> default = `[]`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> publish-jobs = ["homebrew", "npm", "./my-custom-job"]
> ```

This setting determines which publish jobs to run. It accepts 3 kinds of value:

* ["homebrew", for builtin homebrew publishes][homebrew-installer] (since 0.2.0)
* ["npm", for builtin npm publishes][npm-installer] (since 0.14.0)
* ["./my-custom-job" for custom jobs](../ci/customizing.md#custom-jobs) (since 0.3.0)

#### `post-announce-jobs`

> <span style="float:right">since 0.7.0<br>[global-only][]</span>
> [📖 read the ci customization guide!][github-ci] \
> default = `[]`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [dist]
> post-announce-jobs = ["./my-job"]
> ```


This setting determines which custom jobs to run after the "announce" phase. "Announce" is the final phase during which dist schedules any jobs, so any custom jobs specified here are guaranteed to run after everything else.




# the `[workspace]` section

This section is only available in `dist-workspace.toml` files.

### `workspace.members`

> <span style="float:right">since 0.20.0<br>[global-only][]</span>
> [📖 read the project structure guide!][project-guide] \
> default = `[]`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [workspace]
> members = [
>     "cargo:rel/path/to/rust/workspace",
>     "npm:some/js/project/",
>     "npm:some/other/js/project/",
>     "dist:a/generic/project/"
> ]
> ```

In a dist-workspace.toml, this specifies the various projects/workspaces/packages that should
be managed by dist. Each member is of the format `<project-type>:<relative-path>` where
`relative-path` is a path relative to the dist-workspace.toml to a directory containing that type of project, and `project-type` can be one of:

* cargo: expect a Cargo.toml for a cargo-based Rust project in that dir
* npm: expect a package.json for an npm-based JavaScript project in that dir
* dist: expect a dist.toml for a dist-based generic project in that dir


# the `[package]` section

This section is available in `dist.toml` and `dist-workspace.toml` files.

## `package.name`

> <span style="float:right">since 0.12.0<br>[package-only][]</span>
> [📖 read the project structure guide!][project-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [package]
> name = "my-cool-app"
> ```

The name of the package.

All packages must have a name, either sourced from a dist.toml or inherited from a language's native package format like a [Cargo.toml][rust-guide], [package.json][js-guide], etc.

The name is used in a myriad of places to refer to your application and its releases.


## `package.version`

> <span style="float:right">since 0.12.0<br>[package-only][]</span>
> [📖 read the project structure guide!][project-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [package]
> version = "1.2.0-prerelease.2"
> ```

The version of the package. Syntax must be a valid [Cargo SemVer Version][semver-version].

All packages must have a version, either sourced from a dist.toml or inherited from a language's native package format like a [Cargo.toml][rust-guide], [package.json][js-guide], etc.

The version is used in a myriad of places to refer to your application and its releases.


## `package.description`

> <span style="float:right">since 0.12.0<br>[package-only][]</span>
> [📖 read the project structure guide!][project-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [package]
> version = "A cool application that solves all your problems!"
> ```

A brief description of the package.

If not specified, this can be inherited from a language's native package format like a [Cargo.toml][rust-guide], [package.json][js-guide], etc.

This may be used in the metadata of various [installers][].


## `package.authors`

> <span style="float:right">since 0.12.0<br>[package-only][]</span>
> [📖 read the project structure guide!][project-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [package]
> authors = ["axodotdev <hello@axo.dev>"]
> ```

The authors of the package.

If not specified, this can be inherited from a language's native package format like a [Cargo.toml][rust-guide], [package.json][js-guide], etc.

This may be used in the metadata of various [installers][]. We recommend keeping it fairly generic to avoid needless hassles from people changing their names.

## `package.repository`

> <span style="float:right">since 0.12.0<br>[package-only][]</span>
> [📖 read the project structure guide!][project-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [package]
> repository = "https://github.com/axodotdev/axolotolsay"
> ```

A URL to the repository hosting this package.

The following formats are all supported and treated as equivalent:

* `"https://github.com/axodotdev/axolotolsay"`
* `"https://github.com/axodotdev/axolotolsay.git"`
* `"git@github.com:axodotdev/axolotlsay.git"`

If not specified, this can be inherited from a language's native package format like a [Cargo.toml][rust-guide], [package.json][js-guide], etc.

This is *essentially* required as almost all dist features are blocked behind knowing where your project is hosted. All [distable](#dist) packages must agree on this value.


## `package.homepage`

> <span style="float:right">since 0.12.0<br>[package-only][]</span>
> [📖 read the project structure guide!][project-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [package]
> homepage = "https://axodotdev.github.io/axolotlsay"
> ```

A URL to the homepage of the package.

If not specified, this can be inherited from a language's native package format like a [Cargo.toml][rust-guide], [package.json][js-guide], etc.

This may be used in the metadata of various [installers][].


## `package.documentation`

> <span style="float:right">since 0.12.0<br>[package-only][]</span>
> [📖 read the project structure guide!][project-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [package]
> documentation = "https://docs.rs/axolotlsay"
> ```

A URL to the documentation of the package.

If not specified, this can be inherited from a language's native package format like a [Cargo.toml][rust-guide], [package.json][js-guide], etc.

This may be used in the metadata of various [installers][].


## `package.changelog`

> <span style="float:right">since 0.12.0<br>[package-only][]</span>
> [📖 read the project structure guide!][project-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [package]
> changelog = "../CHANGELOG.md"
> ```

A relative path to the changelog file for your package.

If not specified, this can be inherited from a language's native package format like a [Cargo.toml][rust-guide], [package.json][js-guide], etc.

This can be used by various dist features that use your changelog, such as [auto-includes](#auto-includes) and [release-bodies][github-releases-guide]. We will often [autodetect this for you][archives], so this setting is only needed if your changelog has a special name/location/format we can't find.


## `package.readme`

> <span style="float:right">since 0.12.0<br>[package-only][]</span>
> [📖 read the project structure guide!][project-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [package]
> readme = "../README.md"
> ```

A relative path to the readme file for your package.

If not specified, this can be inherited from a language's native package format like a [Cargo.toml][rust-guide], [package.json][js-guide], etc.

This can be used by various dist features that use your readme, such as [auto-includes](#auto-includes). We will often [autodetect this for you][archives], so this setting is only needed if your readme has a special name/location/format we can't find.


## `package.license`

> <span style="float:right">since 0.12.0<br>[package-only][]</span>
> [📖 read the project structure guide!][project-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [package]
> license = "MIT OR Apache-2.0"
> ```

The license(s) of your package, in [SPDX format](https://spdx.org/licenses).

If not specified, this can be inherited from a language's native package format like a [Cargo.toml][rust-guide], [package.json][js-guide], etc.

This may be used in the metadata of various [installers][].


## `package.license-files`

> <span style="float:right">since 0.12.0<br>[package-only][]</span>
> [📖 read the project structure guide!][project-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [package]
> readme = ["../LICENSE-MIT", "../LICENSE-APACHE"]
> ```

Relative paths to the license files for your package.

If not specified, this can be inherited from a language's native package format like a [Cargo.toml][rust-guide], [package.json][js-guide], etc.

This can be used by various dist features that use your license files, such as [auto-includes](#auto-includes). We will often [autodetect this for you][archives], so this setting is only needed if your licenses have a special name/location/format we can't find.


## `package.binaries`

> <span style="float:right">since 0.12.0<br>[package-only][]</span>
> [📖 read the project structure guide!][project-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [package]
> binaries = ["my-app", "my-other-app"]
> ```

Names of binaries (without the extension) your package is expected to build and distribute.

If not specified, this can be inherited from a language's native package format like a [Cargo.toml][rust-guide], [package.json][js-guide], etc.

Including binaries by default opts your package into being [distable](#dist).

See also: [bin-aliases](#bin-aliases), [cstaticlibs](#packagecstaticlibs), [cdylibs](#packagecdylibs)


## `package.cstaticlibs`

> <span style="float:right">since 0.12.0<br>[package-only][]</span>
> [📖 read the project structure guide!][project-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [package]
> cstaticlibs = ["mystaticlib", "some-helper"]
> ```

Names of c-style static libraries (without the extension) your package is expected to build and distribute.

If not specified, this can be inherited from a language's native package format like a [Cargo.toml][rust-guide], [package.json][js-guide], etc.

Including cstaticlibs opts your package into being [distable](#dist) if [`package-libraries = ["cstaticlibs"]`](#package-libraries) is set.

See also: [binaries](#packagebinaries), [cdylibs](#packagecdylibs)


## `package.cdylibs`

> <span style="float:right">since 0.12.0<br>[package-only][]</span>
> [📖 read the project structure guide!][project-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [package]
> cdylibs = ["mydylib", "some-other-helper"]
> ```

Names of c-style dynamic libraries (without the extension) your package is expected to build and distribute.

If not specified, this can be inherited from a language's native package format like a [Cargo.toml][rust-guide], [package.json][js-guide], etc.

Including cdylibs opts your package into being [distable](#dist) if [`package-libraries = ["cdylibs"]`](#package-libraries) is set.

See also: [binaries](#packagebinaries), [cstaticlibs](#packagecstaticlibs)


## `package.build-command`

> <span style="float:right">since 0.12.0<br>[package-only][]</span>
> 🔧 this is an experimental feature! \
> [📖 read the project structure guide!][project-guide] \
> default = `<none>`
>
> *in your dist-workspace.toml or dist.toml:*
> ```toml
> [package]
> build-command = ["make", "dist"]
> ```

A command to run in your package's root directory to build its [binaries](#packagebinaries), [cstaticlibs](#packagecstaticlibs), and [cdylibs](#packagecdylibs).

If not specified, this can be inherited from a language's native package format like a [Cargo.toml][rust-guide], [package.json][js-guide], etc. (This is often preferred since we can natively understand e.g. [Cargo builds](#cargo-build-settings)).


## setting availabilities

Throughout the above docs, different settings will have different rules for where they can be specified (root workspace config file or package config file), and how they'll be inherited.

* global-only: this setting can only be set in the root workspace config ([dist-workspace.toml][js-guide] or [dist.toml][project-guide])
* package-only: this setting can only be set in a package config ([dist.toml][project-guide])
* package-local: this setting can be set in either, with the package overriding the workspace value if it provides one (and otherwise inheriting it).

When you override a package-local setting, the workspace value will be discarded completely. So for instance if the workspace sets `features = ["feature1", "feature2"]` and a package sets `features = ["feature2", "feature3"]`, then that package will only get feature2 and feature3.



[issue-sigstore]: https://github.com/axodotdev/cargo-dist/issues/120
[issue-msvc-crt-static]: https://github.com/axodotdev/cargo-dist/issues/496

[concepts]: ../reference/concepts.md
[installers]: ../installers/index.md
[shell-installer]: ../installers/shell.md
[powershell-installer]: ../installers/powershell.md
[homebrew-installer]: ../installers/homebrew.md
[npm-installer]: ../installers/npm.md
[msi-installer]: ../installers/msi.md
[artifact-url]: ../reference/artifact-url.md
[generate]: ../reference/cli.md#dist-generate
[archives]: ../artifacts/archives.md
[artifact-modes]: ../reference/concepts.md#artifact-modes-selecting-artifacts
[github-build-setup]: ../ci/customizing.md#customizing-build-setup

[workspace-metadata]: https://doc.rust-lang.org/cargo/reference/workspaces.html#the-metadata-table
[cargo-manifest]: https://doc.rust-lang.org/cargo/reference/manifest.html
[workspace]: https://doc.rust-lang.org/cargo/reference/workspaces.html
[semver-version]: https://docs.rs/semver/latest/semver/struct.Version.html
[rust-version]: https://doc.rust-lang.org/cargo/reference/manifest.html#the-rust-version-field
[rustup]: https://rust-lang.github.io/rustup/
[platforms]: https://doc.rust-lang.org/nightly/rustc/platform-support.html
[scope]: https://docs.npmjs.com/cli/v9/using-npm/scope
[crt-static]: https://github.com/rust-lang/rfcs/blob/master/text/1721-crt-static.md#future-work
[axoupdater]: https://github.com/axodotdev/axoupdater
[updater]: ../installers/updater.md
[github-workflow-step]: https://docs.github.com/en/actions/using-workflows/workflow-syntax-for-github-actions#jobsjob_idstepsid

[global-only]: #setting-availabilities
[package-only]: #setting-availabilities
[package-local]: #setting-availabilities

[artifacts]: ../artifacts/index.md
[hosting]: ../ci/index.md
[github-ci]: ../ci/index.md
[github-releases-guide]: ../ci/index.md
[init]: ../updating.md

[distribute]: ../artifacts/index.md

[project-guide]: ../custom-builds.md
[js-guide]: ../quickstart/javascript.md
[rust-guide]: ../quickstart/rust.md

[build-guide]: ../artifacts/index.md
[cargo-build-guide]: ../artifacts/index.md
[binaries]: ../artifacts/index.md
[compiled libraries]: ../artifacts/index.md
