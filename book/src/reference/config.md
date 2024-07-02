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


### authors

> This is a builtin Cargo config, [see the upstream docs](https://doc.rust-lang.org/cargo/reference/manifest.html#the-authors-fields)

This is required by [MSI installers](../installers/msi.md), as they need a "manufacturer".


### `[[bin]]`

> This is a builtin Cargo config, [see the upstream docs](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#binaries)

This is the list of binaries a package defines. Because we consider an "App" to be "A Cargo Package", this field lets you nest multiple binaries under a single "App" or just rename the main binary.

### description

> This is a builtin Cargo config, [see the upstream docs](https://doc.rust-lang.org/cargo/reference/manifest.html#the-description-field)

This is used by [Homebrew installers](../installers/homebrew.md), as they need a formula desc.

### homepage

> This is a builtin Cargo config, [see the upstream docs](https://doc.rust-lang.org/cargo/reference/manifest.html#the-homepage-field)

This is used by [Homebrew installers](../installers/homebrew.md), as they need a formula homepage. If not specified, the formula homepage will fall back to the [repository](#repository) url.

### license-file

> This is a builtin Cargo config, [see the upstream docs](https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields)

cargo-dist defaults to trying to include certain "important" static files in your archives. A LICENSE is one of them.

If you specify a path to a license file, cargo-dist will use that for all packages it affects. Otherwise, cargo-dist will search for LICENSE* or UNLICENSE* files in the package's root directory and the workspace's root directory (preferring the package). If multiple are defined in the same directory, we will grab them all (this is necessary for the extremely common dual MIT/Apache license, which often results in two LICENSE-* files).

Note that the Cargo license-file flag only accepts one path, so it can't handle the dual-license-file case. This cargo feature largely exists as an escape hatch for weird licenses which can't be described by the SPDX format of the "license" field.


### name

> This is a builtin Cargo config, [see the upstream docs](https://doc.rust-lang.org/cargo/reference/manifest.html#the-name-field)

The name of your package will become the name cargo-dist uses to refer to your package. There is currently no notion of a "prettier display name" (if you have a use for that, let us know!).


### publish

> This is a builtin Cargo config, [see the upstream docs](https://doc.rust-lang.org/cargo/reference/manifest.html#the-publish-field)

If you set `publish = false` in your Cargo.toml we will treat this as a hint that cargo-dist should ignore all the affected packages completely. You can override this with dist's own `dist = true` config.


### readme

> This is a builtin Cargo config, [see the upstream docs](https://doc.rust-lang.org/cargo/reference/manifest.html#the-readme-field)

cargo-dist defaults to trying to include certain "important" static files in your archives. A README is one of them.

If you specify a path to a README file, cargo-dist will use that for all the packages it affects. If you don't, then cargo-dist will search for a README* file in the package's root directory and the workspace's root directory (preferring the package).


### repository

> This is a builtin Cargo config, [see the upstream docs](https://doc.rust-lang.org/cargo/reference/manifest.html#the-repository-field)

cargo-dist has an internal notion of an "artifact download URL" that is required for things like [installers][] that detect the current platform and fetch binaries. If your CI backend is "github" then we will base the "[artifact download URL][artifact-url]" on the "repository" key. To be safe, we will only do this if your workspace agrees on this value. It's fine if only some packages bother setting "repository", as long as the ones that do use the exact same string. If they don't we will fail to compute an "artifact download URL", emit a warning, and ignore your request for installers that require it. (This might want to be a hard error in the future.)


### version

> This is a builtin Cargo config, [see the upstream docs](https://doc.rust-lang.org/cargo/reference/manifest.html#the-version-field)

The version of your package is used pervasively, and cargo-dist will generally error out if you ask it to build "my-app-1.0.0" when the actual "my-app" package is set to version "1.1.0".



## workspace.metadata.dist

Cargo allows other tools to include their own project-wide settings in [metadata tables][workspace-metadata]. The one cargo-dist uses is `[workspace.metadata.dist]`, which must appear in your root Cargo.toml (whether or not it's [virtual][workspace]). You can override them on a per-package basis with `[package.metadata.dist]`, which accepts all the same fields (except for those which must be specified once globally, see the docs for each individual option).


### allow-dirty

> since 0.3.0

Example: `allow-dirty = ["ci", "msi"]`

This is a list of generate tasks for cargo-dist to ignore when checking if generated configuration is up to date. It's useful for users who customize their own configuration beyond cargo-dist's generated defaults and want to avoid cargo-dist overwriting it.

Possible values are:

* "ci": don't check/regenerate ci scripts (release.yml)
* "msi": don't check/regenerate msi templates (main.wxs)


### all-features

> since 0.2.0

Example: `all-features = true`

Specifies that all features for a package should be enabled when building it (when set to true this tells us to pass `--all-features` to Cargo).

Defaults false.

If you use this you *probably* want to set it on `[package.metadata.dist]` and
not `[workspace.metadata.dist]`. See ["inferring precise-builds"](#inferring-precise-builds) for details.


### auto-includes

> since 0.0.3

Example: `auto-includes = false`

Allows you to specify whether cargo-dist should auto-include README, (UN)LICENSE, and CHANGELOG/RELEASES files in [archives][]. Defaults to true.


### `[bin-aliases]`

> since 0.14.0

Example:

```toml
[package.metadata.dist.bin-aliases]
"myrealbin" = ["somealias", "otheralias"]
"myotherbin" = "alias"
```

The `[bin-aliases]` setting lets you specify aliases that should be introduced for your binaries by your installers. These aliases aren't included in your archives, and are only created by the installers themselves. The way the alias is created is installer-specific, and may change in the future. Currently:

* shell: symlink
* powershell: hardlink
* npm: extra "bins" pointing at the same command
* homebrew: bin.install_symlink
* msi: not currently supported


### build-local-artifacts

> since 0.8.0

Example: `build-local-artifacts = false`

(defaults `true`)

`build-local-artifacts = false` disables the builtin CI jobs that would build your binaries and archives (and MSI installers). This allows a Sufficiently Motivated user to use custom `build-local-jobs` to completely replace cargo-dist's binary building with something like maturin.

The requirements are simply that you need your custom actions to:

* build archives (tarballs/zips) and checksums that the local CI was expected to produce
* use the github upload-artifacts action to upload all of those to an artifact named `artifacts`

You can get a listing of the exact artifact names to use and their expected contents with:

```
cargo dist manifest --artifacts=local --no-local-paths
```

(`[checksum]` entries are separate artifacts and not actually stored in the archives.)

Also note that for legacy reasons a tarball is expected to have all the contents nested under a root dir with the same name as the tarball (sans extension), while zips are expected to have all the files directly in the root (installers pass `--strip-components=1` to tar when extracting).


### cargo-dist-version

> since 0.0.3

Example: `cargo-dist-version = "0.0.3"`

**This can only be set globally**

This is added automatically by `cargo dist init`, and is a recording of its own version for the sake of reproducibility and documentation. When you run [generate][] the resulting CI scripts will use that version of cargo-dist to build your applications.

The syntax must be a valid [Cargo-style SemVer Version][semver-version] (not a VersionReq!).

If you delete the key, generate will just use the version of cargo-dist that's currently running.


### checksum

> since 0.1.0

Example: `checksum = "sha512"`

Specifies how to checksum [archives][]. Supported values:

* "sha256" (default) - generate a .sha256 file for each archive
* "sha512" - generate a .sha512 file for each archive
* "sha3-256" - generate a .sha3-256 file for each archive
* "sha3-512" - generate a .sha3-512 file for each archive
* "blake2s" - generate a .blake2s file for each archive
* "blake2b" - generate a .blake2b file for each archive
* "false" - do not generate any checksums

The hashes should match the result that sha256sum, sha512sum, etc. generate, and the file should be readable by those sorts of commands.

Future work is planned to [support more robust signed checksums][issue-sigstore].


### ci

> since 0.0.3

Example: `ci = ["github"]`

**This can only be set globally**

This is a list of CI backends you want to support, allowing subsequent runs of [generate][] to know what CI scripts to generate. Its presence also enables certain CI-specific features. For instance if "github" is included we'll try to generate the body for a Github Release and tell [installers][] to fetch binaries from a Github Release.  Once we introduce more CI backends we'll need to more completely rationalize what that means. In all likelihood each set of CI scripts will need to explicitly select just its own CI by passing `--ci=...` for every invocation.

"github" is currently the only supported CI backend.

`cargo dist init` can set this if you pass `--ci=...`


### create-release

> since 0.2.0

Example: `create-release = false`

**This can only be set globally**

Whether we should create the Github Release for you in your Release CI.

If true (default), cargo-dist will create a new Github Release and generate
a title/body for it based on your changelog.

If false, cargo-dist will assume a draft Github Release for the current git tag
already exists with the title/body you want, and just upload artifacts to it.
At the end of a successful publish it will undraft the Github Release.


### custom-success-msg

> since 0.15.0

Example: `custom-success-msg = "axolotlsay is ready to rumble! >o_o<"`

A custom message to display on success in the [shell](../installers/shell.md) and [powershell](../installers/powershell.md) installers.

Defaults to "everything's installed!"


### default-features

> since 0.2.0

Example: `default-features = false`

Specifies that default features for a package should be enabled when building it (when set to false, this tells us to pass `--no-default-features` to Cargo).

Defaults true.

If you use this you *probably* want to set it on `[package.metadata.dist]` and not `[workspace.metadata.dist]`. See ["inferring precise-builds"](#inferring-precise-builds) for details.


### dependencies

> since 0.4.0

Allows specifying dependencies to be installed from a system package manager before the build begins. This is useful if your tool needs certain build tools (say, cmake) or links against C libraries provided by the package manager. This is specified in a Cargo-like format which should be familiar. Dependencies can be specified in two forms:

* A simple form, in which only a version is specified. If any version will do, use `'*'`.
* A complex form, in several extra options can be specified.

Supported options are:

* `version` - A specific version of the package to install. This must be specified in the format that the package manager itself uses. Not used on Homebrew, since Homebrew does not support any method to specify installing specific versions of software.
* `stage` - When exactly cargo-dist should make use of this package. Two values are supported: `build`, which specifies that the package should be installed before the build occurs; and `run`, which specifies that the package should be installed alongside your software at the time end users run it. The default is `build`. If `run` is specified for Homebrew dependencies, and you've enabled the Homebrew installer, the Homebrew installer will specify those packages as dependencies.
* `targets` - A set of one or more targets to install the package on, in Rust target-triple format. If not specified, the package is installed on all targets. This is meant as an override to allow a package to be conditionally installed on only certain platforms; for example, a platform may need a build dependency only on Apple Silicon macOS, or have different build dependencies between x86_64 and ARM Windows.

Supported package managers:

* Apt (Linux)
* Chocolatey (Windows)
* Homebrew (macOS)

Example:

```toml
[workspace.metadata.dist.dependencies.homebrew]
cmake = '*'
libcue = { stage = ["build", "run"] }

[workspace.metadata.dist.dependencies.apt]
cmake = '*'
libcue-dev = { version = "2.2.1-2" }

[workspace.metadata.dist.dependencies.chocolatey]
lftp = '*'
cmake = { version = '3.27.6', targets = ["aarch64-pc-windows-msvc"] }
```


### dispatch-releases

> since 0.8.0

Example: `dispatch-releases = true`

(defaults `false`)

`dispatch-releases = true` adds a new experimental mode where releases are triggered with workflow_dispatch instead of tag-push (relying on creating a github release implicitly tagging).

Enabling this disables tag-push releases, but keeps pr checks enabled.

By default the workflow dispatch form will have "dry-run" populated as the tag, which is taken to have the same meaning as `pr-run-mode = upload`: run the plan and build steps, but not the publish or announce ones. Currently hosting is also disabled, but future versions may add some forms of hosting in this mode.


### display

> since 0.16.0

Example: `display = false`

(defaults `true`)

Specifies whether this App should be displayed in release bodies (like GitHub Releases). This is useful for hiding things that aren't the "primary" or "featured" application but still need to be included in the release for logistical reasons.


### display-name

> since 0.16.0

Example: `display-name = "my cool app"`

(defaults to the App's actual name)

Specifies how to refer to the App in release bodies (like GitHub Releases). This is useful for situations where the app name *must* have a certain value for logistical reasons but you want to refer to it by a nicer name.


### dist

> since 0.0.3

Example: `dist = false`

Specifies whether cargo-dist should ignore this package. It primarily exists as an alternative for `publish=false` or an override for `publish=false`.


### extra-artifacts

> since 0.6.0

Example:

```toml
[[workspace.metadata.dist.extra-artifacts]]
artifacts = ["dist-manifest-schema.json"]
build = ["cargo", "dist", "manifest-schema", "--output=dist-manifest-schema.json"]
```

Allows building extra artifacts to upload to your releases. Users can download these directly alongside artifacts like release tarballs or installers. To enable this feature, create an `extra-artifacts` array on your workspace or package configuration. This takes two keys:

* `build`: A command or script to run to produce these artifacts. This is an array of one or more strings; the first string is the command cargo-dist will run, and any subsequent strings are arguments to pass to that command.
* `artifacts`: An array of artifacts that cargo-dist expects to exist after the `build` command is run. Every artifact in this list will be uploaded individually to your release.

cargo-dist uses this feature to distribute its `dist-manifest-schema.json`.


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


### features

> since 0.2.0

Example: `features = ["serde-support", "fancy-output"]`

Specifies feature-flags that should be passed to a package when building it. This lets you enable features that should be on "in production" but for whatever reason shouldn't be on by default.

For instance for packages that are a library and a CLI binary, some developers prefer to make the library the default and the CLI opt-in. In such a case you would want to add `features = ["cli"]` to your `[package.metadata.dist]`.

If you use this you *probably* want to set it on `[package.metadata.dist]` and
not `[workspace.metadata.dist]`. See ["inferring precise-builds"](#inferring-precise-builds) for details.

### formula

> since 0.11.0

Example: `formula = "axolotlbrew"`

Specifies a string to override the default Homebrew formula name (the app name). This works
well specifically for folks who are customizing their bin name and would like the Homebrew
formula to match the bin name as opposed to the app name (which, in Rust, is the crate name).

You must set this on `[package.metadata.dist]` and not `[workspace.metadata.dist]`.

### force-latest

> since 0.15.0

Overrides cargo-dist's default handling of prerelease versions. Ordinarily, cargo-dist uses [semver](https://semver.org) rules to determine if a version number is a prerelease or not and has some special handling if it is. With this setting, cargo-dist will always consider a version to be the latest no matter what its version number is. This means that the following prerelease handling behaviour will no longer apply:

* If cargo-dist interprets a version as a prerelease, it will publish it to GitHub as a "prerelease" instead of the "latest" release.
* cargo-dist will not publish prereleases to [Homebrew][homebrew-installer] or [npm][npm installers] by default.

See also the ["publish-prereleases"](#publish-prereleases) setting.


### github-attestations

> since 0.16.0

Example: `github-attestations = true`

Defaults false (but may become true in the future).

If you're using GitHub Releases, this will enable GitHub's experimental artifact attestation feature. [See the full docs for details](../supplychain-security/attestations/github.md).


### github-custom-job-permissions

> since 0.18.0

Example:

```
publish-jobs = ["npm", "./my-custom-publish"]
github-custom-job-permissions = { "my-custom-publish" = ["packages: admin"] }
```

Allows you to customize the permissions given to your custom CI jobs.

By default all custom `publish-jobs` get `["id-token: write", "packages: write"]`.
If you override a publish job's permissions, the default permissions will be removed.
All other custom jobs default to no special permissions.


### github-custom-runners

> since 0.6.0 (target-specific runners), 0.15.0 (global runner)

Example:

```toml
[workspace.metadata.dist.github-custom-runners]
aarch64-unknown-linux-gnu = "buildjet-8vcpu-ubuntu-2204-arm"
aarch64-unknown-linux-musl = "buildjet-8vcpu-ubuntu-2204-arm"
```

Allows specifying which runner to use for a target. The keys within this table are target triples in the same format as the ["targets"](#targets) setting. Any targets not specified in this table will use the defaults.

In addition to defining runners for a target, it's also possible to specify a runner for the global, non-target-specific tasks using the `global` key. This runner will be used for tasks like `plan`, `host`, generating installers, and so on.


### github-release

> since 0.17.0

Example: `github-release = "announce"`

Possible values:

* `auto`: create the GitHub Release whenever is best
* `host`: create the GitHub Release during the host step
* `announce`: create the GitHub Release during the announce step

Controls which stage of the release process the GitHub Release will be created in.

By default, the GitHub Release is created during the "host" phase, as it hosts the files some installers will try to download. If axo Releases is also enabled, it will be moved back to the "announce" phase, as the files will be primarily hosted on axo Releases, and GitHub Releases will be treated like a backup and announcement of the release.

**Most users should be well-served by the default setting, and changing it is likely to introduce undesirable publishing race conditions.** The only reason you might want to override this setting is if you're using [`dispatch-releases = true`](#dispatch-releases) and you really want your git tag to be the last operation in your release process (because creating a GitHub Release necessarily creates the git tag if it doesn't yet exist). In this case setting github-release = "announce" will accomplish that, but see below for what race conditions this might introduce.

If using only GitHub Releases, and you force it to run during "announce", there will be a very brief window (~30 seconds) during which generated Homebrew and npm installers are live and referencing URLs that will only exist when the GitHub Release is created, causing the packages to error out when installed.

However, if you're publishing only packages that don't reference hosted artifacts (such as Cargo crates, or any custom publish job that fully embeds the binaries), then there is no race, and you could consider changing the default. That said, it would be a looming footgun if you ever introduce new publish jobs and forget about this.


### github-releases-repo

> since 0.14.0

Example: `github-releases-repo = "myorg/public"`

Allows specifying a different repo to publish GitHub Releases to than the current one. This can be used to publish to a public repo from a private one. Let's assume you want to publish to `myorg/public` from `myorg/private`. Then in your config in `myorg/private`, you'd set `github-releases-repo = "myorg/public"`.

To ensure the workflow has permission to do this, you need to create a [GitHub Personal Access Token with the "repo" scope](https://github.com/settings/tokens/new?scopes=repo) that can access `myorg/public`. This must be added as a GitHub SECRET called `GH_RELEASES_TOKEN` on `myorg/private`.

GitHub Releases isn't really designed for this, so there's a few strange things that will happen here:

* GitHub Releases always requires a commit to be tagged, and in this case the tag would be on `myorg/public` even though the workflow is running on `myorg/private`, which (presumably) has unrelated commits. Currently **we will tag the latest commit on the [default branch](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/about-branches#about-the-default-branch) of `myorg/public`**. If you're using [the dispatch-releases flow](#dispatch-releases), no tag will be created on `myorg/private`.

* GitHub Releases will provide a source tarball pointing at the tagged commit on `myorg/public`, but that's (presumably) not the source that your release was actually built from. This cannot be disabled, but it's also essentially harmless. However **cargo-dist uploads its own source tarball and that *WILL* contain the source of the private repo**. If you don't want this, use [the `source-tarball = false` setting](#source-tarball).

### github-releases-submodule-path

> since 0.15.0

Designed for use with `github-releases-repo` above. When specified, the cached commit of the submodule at this path will be used as the commit to tag in the target repository. If not specified, the latest commit in the target repository will be used instead.


### global-artifacts-jobs

> since 0.7.0

Example: `global-artifacts-jobs = ["./my-job"]`

This setting determines which custom jobs to run during the "build global artifacts" phase, during which installers are built.


### host-jobs

> since 0.7.0

Example: `host-jobs = ["./my-job"]`

This setting determines which custom jobs to run during the "host" phase, during which cargo-dist decides whether to proceed with publishing the release.


### hosting

> since 0.5.0

Example: `hosting = ["axodotdev", "github"]`

Possible values:

* `axodotdev`: Use Axo Releases (currently in closed beta)
* `github`: Use Github Releases (default if ci = "github")

Specifies what hosting provider to use when hosting/announcing new releases.

By default we will automatically use the native hosting of your CI provider, so when running on Github CI, we'll default to using Github Releases for hosting/announcing.

If Axo Releases and Github Releases are both enabled, we will host/announce on both platforms, but the Github Release's contents will regard the Axo Release as the canonical source for the files. Specifically if you have a shell installer, the Github Release will contain a shell installer that fetches from Axo Releases and it will tell you to `curl | sh` with a URL to Axo Releases.

(Ideally files uploaded to both hosts should be bitwise identical, which means we have to "pick"
a host to win for fetching installers, and if you're using Axo Releases at all you *probably* want that one to win.)


### include

> since 0.0.3

Example: `include = ["my-cool-file.txt", "../other-cool-file.txt", "./some/dir/"]`

This is a list of additional *files* or *directories* to copy into the root of all [archives][] that this setting affects. The paths are relative to the directory of the Cargo.toml that you placed this setting in. Globs are not supported.


### installers

> since 0.0.3

Example: `installers = ["shell", "powershell"]`

This is a list of installers you want to be made for your application(s). In principle this can be overridden on a per-package basis but that is not well tested. See [the full docs on installers for the full list of values][installers].

See "repository" for some discussion on the "Artifact Download URL".


### install-path

> since 0.1.0

Examples:

```toml
install-path = "~/.my-app/"
install-path = ["$MY_APP_HOME/bin", "~/.my-app/bin"]
```

The strategy that script installers ([shell][shell-installer], [powershell][powershell-installer]) should use for selecting a path to install things at, with 3 possible syntaxes:

* `CARGO_HOME`: (default) installs as if `cargo install` did it (tries `$CARGO_HOME/bin/`, but if `$CARGO_HOME` isn't set uses `$HOME/.cargo/bin/`). Note that we do not (yet) properly update some of the extra metadata files Cargo maintains, so Cargo may be confused if you ask it to manage the binary.

* `~/some/subdir/`: installs to the given subdir of the user's `$HOME`

* `$SOME_VAR/some/subdir`: installs to the given subdir of the dir defined by `$SOME_VAR`

> NOTE: `$HOME/some/subdir` is technically valid syntax but it won't behave the way you want on Windows, because `$HOME` isn't a proper environment variable. Let us handle those details for you and just use `~/subdir/`.

All of these error out if none of the required env-vars are set to a non-empty value. Since 0.14.0 you can provide an array of options to try if all the previous ones fail. Such an "install-path cascade" would typically be used to provide an environment variable for changing the install dir, with a more hardcoded home subdir as a fallback:

```toml
install-path = ["$MY_APP_HOME/bin", "~/.my-app/bin"]
```

It hasn't yet been tested whether this is appropriate to pair with things like `$XDG_BIN_HOME`, but we'd sure like it to be.

We do not currently sanitize/escape the path components (it's not really a security concern when the user is about to download+run an opaque binary anyway). In the future validation/escaping of this input will become more strict. We do appear to correctly handle spaces in paths on both windows and unix (i.e. `~/My cargo-dist Documents/bin/` works), but we won't be surprised if things misbehave on Interesting Inputs.

Future Improvements:

* In the future [we may support XDG dirs](https://github.com/axodotdev/cargo-dist/issues/287)
* In the future [we may support %windows dirs%](https://github.com/axodotdev/cargo-dist/issues/288)
* For historical reasons `CARGO_HOME` [uses a slightly different install dir structure from the others](https://github.com/axodotdev/cargo-dist/issues/934), and so for safety cannot be paired with the others strategies in an install-path cascade.

(Please file an issue if you have other requirements!)

### install-updater

> since 0.12.0

Example: `install-updater = true`

Defaults to false.

NOTE: this feature is currently experimental.

Determines whether to install a standalone updater program alongside your program. This program will be named `yourpackage-update`, and can be run by the user to automatically check for newer versions and install them without needing to visit your website. This updater will only be installed for users who use the shell or Powershell installers; users who received your package from a package manager, such as Homebrew or npm, will need to use the same package manager to perform upgrades.

This updater is the commandline tool contained in the open source [axoupdater] package.

For more information, see the [updater] documentation.

### local-artifacts-jobs

> since 0.7.0

Example: `local-artifacts-jobs = ["./my-job"]`

This setting determines which custom jobs to run during the "build local artifacts" phase, during which binaries are built.


### merge-tasks

> since 0.1.0

Example: `merge-tasks = true`

**This can only be set globally**

Whether we should try to merge otherwise-parallelizable tasks onto the same machine, sacrificing latency and fault-isolation for more the sake of minor efficiency gains.

For example, if you build for x64 macos and arm64 macos, by default we will generate ci which builds those independently on separate logical machines. With this enabled we will build both of those platforms together on the same machine, making it take twice as long as any other build and making it impossible for only one of them to succeed.

The default is `false`. Before 0.1.0 it was always `true` and couldn't be changed, making releases annoyingly slow (and technically less fault-isolated). This config was added to allow you to restore the old behaviour, if you really want.


### msvc-crt-static

> since 0.4.0

Example: `msvc-crt-static = false`

Specifies how The C Runtime (CRT) should be linked when building for Windows. Rust defaults to this being `= false` (dynamically link the CRT), but cargo-dist actually defaults to making this `= true` (statically link the CRT). [The Rust default is mostly a historical accident, and it's widely regarded to be an error that should one day be changed][crt-static]. Specifically it's a mistake for the typical Rust application which statically links everything else, because Windows doesn't actually guarantee that the desired things are installed on all machines by default, and statically linking the CRT is a supported solution to this issue.

However when you *do* want a Rust application that dynamically links more things, it then becomes correct to dynamically link the CRT so that your app and the DLLs it uses can agree on things like malloc. However Rust's default is still insufficient for reliably shipping such a binary, because you really should also bundle a "Visual C(++) Redistributable" with your app that installs your required version of the CRT. The only case where it's *probably* fine to not do this is when shipping tools for programmers who probably already have all of that stuff installed (i.e. anyone who installs the Rust toolchain will have that stuff installed).

This config exists as a blunt way to return to the default Rust behaviour of dynamically linking the CRT if you really want it, but more work is needed to handle Redistributables for that usecase.

[See this issue for details and discussion][issue-msvc-crt-static].


### npm-package

> since 0.14.0

Example: `npm-package = "mycoolapp"`

Specifies that an [npm installer][] should be published under the given name, as opposed to the name of the app (cargo package) they are defined by.

This does not set the [scope][] the package is published under, for that see [npm-scope](#npm-scope).


### npm-scope

> since 0.0.6

Example: `npm-scope = "@axodotdev"`

Specifies that [npm installers][] should be published under the given [scope][]. The leading `@` is mandatory. If you newly enable the npm installer in `cargo dist init`'s interactive UI, then it will give you an opportunity to add the scope.

If no scope is specified the package will be global.


### plan-jobs

> since 0.7.0

Example: `plan-jobs = ["./my-job"]`

This setting determines which custom jobs to run during the "plan" phase, which happens at the very start of the build.


### post-announce-jobs

> since 0.7.0

Example: `post-announce-jobs = ["./my-job"]`

This setting determines which custom jobs to run after the "announce" phase. "Announce" is the final phase during which cargo-dist schedules any jobs, so any custom jobs specified here are guaranteed to run after everything else.


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


### pr-run-mode

> since 0.3.0

Example: `pr-run-mode = "skip"`

This setting determines to what extent we run your Release CI on pull-requests:

* "skip": don't check the release process in PRs
* "plan": run 'cargo dist plan' on PRs (recommended, also the default)
* "upload": build and upload an artifacts.zip to the PR (expensive)


### publish-jobs

> since 0.2.0

Example: `publish-jobs = ["homebrew", "npm", "./my-custom-job"]`

This setting determines which publish jobs to run. It accepts 3 kinds of value:

* ["homebrew", for builtin homebrew publishes](../installers/homebrew.md) (since 0.2.0)
* ["npm", for builtin npm publishes](../installers/npm.md) (since 0.14.0)
* ["./my-custom-job" for custom jobs](../ci/customizing.md#custom-jobs) (since 0.3.0)


### publish-prereleases

> since 0.2.0

Example: `publish-prereleases = true`

If you set `publish-prereleases = true`, cargo-dist will publish prerelease versions to package managers such as Homebrew. By default, cargo-dist will only publish stable versions.


### rust-toolchain-version

> since 0.0.3 (deprecated in 0.1.0)

Example: `rust-toolchain-version = "1.67.1"`

> Deprecation reason: [rust-toolchain.toml](https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file) is a more standard/universal mechanism for pinning toolchain versions for reproducibility. Teams without dedicated release engineers will likely benefit from unpinning their toolchain and letting the underlying CI vendor silently update them to "some recent stable toolchain", as they will get updates/improvements and are unlikely to have regressions.

**This can only be set globally**

This is added automatically by `cargo dist init`, recorded for the sake of reproducibility and documentation. It represents the "ideal" Rust toolchain to build your project with. This is in contrast to the builtin Cargo [rust-version][] which is used to specify the *minimum* supported Rust version. When you run [generate][] the resulting CI scripts will install that version of the Rust toolchain with [rustup][]. There's nothing special about the chosen value, it's just a hardcoded "recent stable version".

The syntax must be a valid rustup toolchain like "1.60.0" or "stable" (should not specify the platform, we want to install this toolchain on all platforms).

If you delete the key, generate won't explicitly setup a toolchain, so whatever's on the machine will be used (with things like rust-toolchain.toml behaving as normal). Before being deprecated the default was to `rustup update stable`, but this is no longer the case.


### source-tarball

> since 0.14.0

Example: `source-tarball = false`

By default, cargo-dist creates and uploads source tarballs from your repository. This setting disables that behaviour. This is especially useful for users who distribute closed-source software to hosts outside their git repos and who would prefer not to distribute source code to their users.


### ssldotcom-windows-sign

> since 0.15.0

Example: `ssldotcom-windows-sign = "prod"`

If you wish to sign your Windows artifacts (EXEs and [MSIs](../installers/msi.md)) such that Windows SmartScreen won't complain about them, this is the feature for you. [See the full guide for details](../signing-and-attestation.md#windows-artifact-signing-with-sslcom-certificates).

This setting takes one of two values:

* "prod": use the production ssl.com signing service
* "test": use the testing ("sandbox") ssl.com signing service

These strings match the [environment_name setting](https://github.com/SSLcom/esigner-codesign/blob/32825070bd8ca335577862dc735343ae155f2652/README.md#L48) that [SSL.com's code signing action uses](https://github.com/SSLcom/esigner-codesign) uses.


### tag-namespace

> since 0.10.0

Example: `tag-namespace = "some-prefix"`

Setting `tag-namespace = "owo"` will change the tag matching expression we put in your github ci, to require the tag to start with "owo" for cargo-dist to care about it. This can be useful for situations where you have several things with different tag/release workflows in the same workspace. It also renames `release.yaml` to `owo-release.yml` to make it clear it's just one of many release workflows.

**NOTE**: if you change tag-namespace, cargo-dist will generate the new `owo-release.yml` file, but not delete the old one. Be sure to manually delete the old `release.yml`!


### tap

> since 0.2.0

Example: `tap = "axodotdev/homebrew-formulae"`

This is the name of a GitHub repository which cargo-dist should publish the Homebrew installer to. It must already exist, and the token which creates releases must have write access.

See the [installers documentation][homebrew-installer] for more information on Homebrew support.


### targets

> since 0.0.3

Example: `targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc"]`

This is a list of [target platforms][platforms] you want your application(s) to be built for. In principle this can be overridden on a per-package basis but that is not well tested.

The supported choices are:

* x64 macOS: "x86_64-apple-darwin"
* x64 Windows: "x86_64-pc-windows-msvc"
* x64 Linux: "x86_64-unknown-linux-gnu"
* arm64 macOS (Apple silicon): "aarch64-apple-darwin"
* arm64 Linux: "aarch64-unknown-linux-gnu"
* x64 Linux (musl): x86_64-unknown-linux-musl
* arm64 Linux (musl): aarch64-unknown-linux-musl

By default all runs of `cargo-dist` will be trying to handle all platforms specified here at once. If you specify `--target=...` on the CLI this will focus the run to only those platforms. As discussed in [concepts][], this cannot be used to specify platforms that are not listed in `metadata.dist`, to ensure different runs agree on the maximum set of platforms.


### unix-archive

> since 0.0.5

Example: `unix-archive = ".tar.gz"`

Allows you to specify the file format to use for [archives][] that target not-windows. The default is
".tar.xz". See "windows-archive" below for a complete list of supported values.



### windows-archive

> since 0.0.5

Example: `windows-archive = ".tar.gz"`

Allows you to specify the file format to use for [archives][] that target windows. The default is
".zip". Supported values:

* ".zip"
* ".tar.gz"
* ".tar.xz"
* ".tar.zstd" (deprecated for Zstd)
* ".tar.zst" (recommended for Zstd)

See also unix-archive below.



## Subsetting CI Flags

Several `metadata.dist` configs have globally available CLI equivalents. These can be used to select a subset of `metadata.dist` list for that run. If you don't pass any, it will be as-if you passed all the values in `metadata.dist`. You can pass these flags multiple times to provide a list. This includes:

* `--target`
* `--installer`
* `--ci`

See [Artifact Modes][artifact-modes] for how you might use this kind of subsetting.

Caveat: the default "host" Artifact Mode does something fuzzier with `--target` to allow you to build binaries that are usable on the current platform. Again see [Artifact Modes][artifact-modes].


[issue-sigstore]: https://github.com/axodotdev/cargo-dist/issues/120
[issue-msvc-crt-static]: https://github.com/axodotdev/cargo-dist/issues/496

[concepts]: ../reference/concepts.md
[installers]: ../installers/index.md
[shell-installer]: ../installers/shell.md
[powershell-installer]: ../installers/powershell.md
[homebrew-installer]: ../installers/homebrew.md
[npm installers]: ../installers/npm.md
[artifact-url]: ../reference/artifact-url.md
[generate]: ../reference/cli.md#cargo-dist-generate
[archives]: ../artifacts/archives.md
[artifact-modes]: ../reference/concepts.md#artifact-modes-selecting-artifacts

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
