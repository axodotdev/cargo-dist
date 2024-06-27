# Unreleased

Nothing Yet!


# Version 0.17.0 (2024-06-27)

This release is mostly several fixes to how we create GitHub Releases, as well as some internal improvements for future feature work.


## GitHub Release Ordering

We now prefer creating your [GitHub Release in the host step](https://opensource.axo.dev/cargo-dist/book/reference/config.html#github-release), ensuring published npm and Homebrew packages never refer to URLs that don't yet exist.

Conceptually the cargo-dist pipeline is as follow:

1. plan the release
2. build the artifacts
3. host the artifacts
4. publish the packages (these can fetch the hosted artifacts!)
5. announce the release (this says to install the published packages!)

GitHub Releases has always been a bit problematic as a hosting provider because it's both where we want to host our files, and creating it is also in some sense announcing the release. Ideally you would be able to draft the GitHub Release to host your files, publish everything that references those files, and then undraft it at the end to announce and tag the release. However the URLs of artifacts in a draft release are temporary, and go away when you undraft it, so this doesn't work.

When we first created cargo-dist, we didn't support publishing packages, so we put GitHub Release at the end, since it was basically the only "side-effect" of running cargo-dist, and you want those at the end. Once we added publishing of things to npm and homebrew, the dual nature of GitHub releases became way more apparent.

In fact, because the GitHub Release contains instructions to install from npm/homebrew, there was essentially a circular dependency between them, with no way to publish all of them in a non-racey way. At the time we opted for preserving existing behaviour of GitHub Last, resulting in a roughly 30 second period where npm/homebrew packages would be published but would error out on install because the artifacts aren't yet uploaded.

We were of course frustrated with this and [had a lot of words to say about URLs, resulting in us making axo Releases, which solved the problem properly](https://blog.axo.dev/2024/01/axo-releases-urls).

But pure GitHub users aren't going away, and this conflict still exists for them. Since then it's become increasingly clear that we made the wrong call here, and in fact the npm/homebrew package integrity is *way* more important than someone maybe getting an email about a GitHub Release referencing packages that don't yet exist. As such we've reversed the original decision and moved GitHub Releases to the host step.

When using axo Releases together with GitHub Releases, GitHub remains in the announce step where it belongs, because it's more of a mirror/announcement, and not the canonical file host.

If for whatever reason you need to get the old behaviour back, you can use the new [`github-release = "announce"` config](https://opensource.axo.dev/cargo-dist/book/reference/config.html#github-release).

The only reason you might want to override this setting is if you're using [`dispatch-releases = true`](https://opensource.axo.dev/cargo-dist/book/reference/config.html#dispatch-releases) and you really want your git tag to be the last operation in your release process (because creating a GitHub Release necessarily creates the git tag if it doesn't yet exist, and many organizations really don't like when you delete/change git tags). In this case setting `github-release = "announce"` will accomplish that, but the above race conditions would then apply.

* [docs](https://opensource.axo.dev/cargo-dist/book/reference/config.html#github-release)
* @mistydemeo [fix: prefer creating github releases in host step](https://github.com/axodotdev/cargo-dist/pull/1171)


## GitHub Release Reliability

GitHub Releases should once again be created transactionally, preventing a release from being created without its artifacts being uploaded, if uploading the artifacts fails for any reason. This fixes a regression from the previous release.

When using the `dispatch-releases = true` setting, we now more strictly specify the commit that should be tagged, preventing any race conditions from changing it. This race potentially always existed, but only seemed to be observable if you retried a failed release.

* @gankra + @mistydemeo [fix: make github releases more robust](https://github.com/axodotdev/cargo-dist/pull/1164)


## Other Fixes

* @gankra [feat: experimental generic workspaces](https://github.com/axodotdev/cargo-dist/pull/1116)
* @mistydemeo [chore: move axoproject in tree](https://github.com/axodotdev/cargo-dist/pull/1135)
* @mistydemeo [fix: clamp workspace search to current repo](https://github.com/axodotdev/cargo-dist/pull/1158)
* @mistydemeo [fix: pass correct path to generic builds](https://github.com/axodotdev/cargo-dist/pull/1157)
* @mistydemeo [fix: add generic workspace tests](https://github.com/axodotdev/cargo-dist/pull/1150)
* @mistydemeo [fix: pipe working directory to more commands](https://github.com/axodotdev/cargo-dist/pull/1139)
* @mistydemeo [feat: cache cargo-dist binary in global tasks](https://github.com/axodotdev/cargo-dist/pull/1165)



# Version 0.16.0 (2024-06-14)

This release introduces some new [supplychain security features](https://opensource.axo.dev/cargo-dist/book/supplychain-security/index.html), and fixes some bugs.

## GitHub Artifact Attestations

With the new [`github-attestations = true` setting](https://opensource.axo.dev/cargo-dist/book/supplychain-security/attestations/github.html) you can opt into GitHub's experimental artifact attestation system. In the future this may become enabled by default.

* @dunxen [feat: add support for github artifact attestations](https://github.com/axodotdev/cargo-dist/pull/1012)


## Reducing Third-Party Actions

We're working towards replacing some third-party GitHub actions used by cargo-dist with builtin implementations, reducing the surface area for audits. We've begun with replacing [ncipollo/release-action](https://github.com/ncipollo/release-action) with usage of the preinstalled GitHub CLI. To be clear: we have no reason to distrust the contents of action, and it's officially recommended by GitHub. It was just simple to replace with a more first-party solution.

@mistydemeo [feat: use the raw github cli instead of an action for releases](https://github.com/axodotdev/cargo-dist/pull/1089)


## Autodetect Buildjet Runners For Rust Cache

We use [swatinem/rust-cache](https://github.com/Swatinem/rust-cache) to try to speed up the release process. As it turns out, they have special support for buildjet's caching backend, which is faster and presumably more secure to use when running actions on buildjet's infra. Our users often [enable buildjet for arm linux builds](https://opensource.axo.dev/cargo-dist/book/ci/customizing.html#custom-runners), so hopefully those should be faster now!

@gankra + @arlyon [feat: autodetect buildjet runners to use their backend for rust cache](https://github.com/axodotdev/cargo-dist/pull/1129)


## Path Flexibility For Extra Artifacts

Previously [the extra-artifacts setting](https://opensource.axo.dev/cargo-dist/book/reference/config.html#extra-artifacts) didn't support the outputs being produced anywhere but the root of the repository. Now the input can be a relative path, making the feature easier to use.

@gankra [fix: rework extra_artifacts to properly use paths](https://github.com/axodotdev/cargo-dist/pull/1128)


# Version 0.15.1 (2024-06-04)

This is a small release to improve the compatibility of the npm installers with other JS package managers.

In 0.15.0 we introduced a regression for installing via pnpm, resulting in infinite loops that produced a cryptic "argument list too long" error. This only affected pnpm because of the precise timing of when it creates shim scripts for binaries. This has now been fixed, and we've introduced tests to ensure that pnpm is explicitly supported from here on out.

We now signal to yarn that we mutate node_modules (to fetch and install binaries), avoiding issues with yarn PnP which assumes node_modules is immutable. Tests have been introduced to ensure that yarn is explicitly supported from here on out.


# Version 0.15.0 (2024-05-31)

This release contains a ton of new features and some fixes.

For the supplychain folks, we now support windows codesigning and several new hashing algorithms.

For projects with complex repositories, we now have several new options for configuring the release process.


## Windows codesigning

cargo-dist can automatically codesign Windows EXEs and MSIs using SSL.com's eSigner cloud signing service.

Although there are many ways to do code signing, this process is specifically concerned with ensuring Windows SmartScreen recognizes the authenticity of the signatures and doesn't prevent your users from running the application. Otherwise, any user who downloads your application with a web browser will get a popup warning them against running it. (Alternative methods of downloading and installing, such as cargo-dist's powershell installers do not trigger SmartScreen.)

* [docs](https://opensource.axo.dev/cargo-dist/book/signing-and-attestation.html#windows-artifact-signing-with-sslcom-certificates)
* impl @Gankra [feat: reimplement ssldotcom-windows-sign](https://github.com/axodotdev/cargo-dist/pull/1036)


## Additional hash algorithms

The SHA-3 and BLAKE2 hash algorithms are now supported.

Thanks to @sorairolake for contributing this!

* [docs](https://opensource.axo.dev/cargo-dist/book/reference/config.html#checksum)
* impl @sorairolake [feat: support SHA-3 and BLAKE2](https://github.com/axodotdev/cargo-dist/pull/1067)


## Improvements for installing tools within GitHub Actions

Previously, our installers would update the user's `PATH` for local commandline access but wouldn't set themselves up in the `PATH` within GitHub Actions. We've added a feature to the shell and Windows installers which detects if they're running within Actions and adds the newly-installed tool to `GITHUB_PATH` for future tasks within the job.

* impl @Gankra [feat: add paths to GITHUB_PATH if available](https://github.com/axodotdev/cargo-dist/pull/1047)


## Custom installer output

When a shell or powershell installer runs successfully it will print out "everything's installed!", but if you want a different message that better matches your app's look-and-feel, or provides a more useful call-to-action, you can now do that with [the `install-success-msg` setting](https://opensource.axo.dev/cargo-dist/book/reference/config.html#install-success-msg).

* [docs](https://opensource.axo.dev/cargo-dist/book/reference/config.html#install-success-msg)
* impl @ashleygwilliams [feat: custom success msg](https://github.com/axodotdev/cargo-dist/pull/1102)


## Forcing prereleases to be the latest

cargo-dist has traditionally parsed the version number of new releases and used this to determine if the new release is a prerelease or a stable release. We apply some special handling to prereleases, such as marking them as prereleases within GitHub Releases. In 0.15.0, we've added the new [`force-latest`](https://opensource.axo.dev/cargo-dist/book/reference/config.html#force-latest) configuration flag which makes it possible to instruct cargo-dist to treat every release as the latest release regardless of its version number.

This mostly exists to support projects who only plan to produce prereleases for the forseeable future, so that GitHub properly shows them in its UI.

* [docs](https://opensource.axo.dev/cargo-dist/book/reference/config.html#force-latest)
* impl @mistydemeo [feat: allow always marking releases as stable](https://github.com/axodotdev/cargo-dist/pull/1054)

## Configuring the global runner

In 0.6.0, we added support for configuring custom runners for native builds in GitHub Actions. However, until now, all other jobs still ran using our default runner. We've added a setting to the existing custom runner syntax that lets you specify what runner to use for all other jobs by using [the "global" key](https://opensource.axo.dev/cargo-dist/book/reference/config.html#github-custom-runners).

* [docs](https://opensource.axo.dev/cargo-dist/book/reference/config.html#github-custom-runners)
* impl @mistydemeo [feat: allow configuring global runner](https://github.com/axodotdev/cargo-dist/pull/1055)

## Specify commit to publish to in external repo

In 0.14.0, we added the new [`github-releases-repo`](https://opensource.axo.dev/cargo-dist/book/reference/config.html#github-releases-repo) configuration which allows publishing releases to an external repository rather than the one in which the CI job is running. The new [`github-releases-submodule-path`](https://opensource.axo.dev/cargo-dist/book/reference/config.html#github-releases-submodule-path) configuration option enhances that with an additional feature: it allows specifying a submodule within the current repository to use to determine which commit to tag the new release as.

* [docs](https://opensource.axo.dev/cargo-dist/book/reference/config.html#github-releases-submodule-path)
* impl @mistydemeo [feat: allow specifying external commit from submodule](https://github.com/axodotdev/cargo-dist/pull/1026)


## Fixes

* @mistydemeo [fix: improve reliability for fetching standalone updaters](https://github.com/axodotdev/cargo-dist/pull/1052)
* @mistydemeo [fix: write correct install prefix to receipt](https://github.com/axodotdev/cargo-dist/pull/1037)
* @mistydemeo [fix: bin_aliases should come from packages](https://github.com/axodotdev/cargo-dist/pull/1031)
* @Gankra [fix: repair and cleanup npm binary caching](https://github.com/axodotdev/cargo-dist/pull/1050)
* @sorairolake [fix: zstd file extension](https://github.com/axodotdev/cargo-dist/pull/1066)


# Version 0.14.1 (2024-05-08)

This is a bugfix release which fixes release announcement text for apps whose `repository` field in `Cargo.toml` ends with `.git` ([#1020](https://github.com/axodotdev/cargo-dist/issues/1020)). It also updates several dependencies and upgrades the version of Rust used to build cargo-dist to 1.78.0.

## Fixes

* @mistydemeo [fix: host should use web_url()](https://github.com/axodotdev/cargo-dist/pull/1024)


# Version 0.14.0 (2024-05-06)

This is a BIG release for installers, with every installer getting big bugfixes and reworks.

* shell installers:
    * support cascading install-path rules like "install to $MY_APP_HOME, or ~/.my-app if that's not set"
    * can install aliases for your binaries (symlink on unix, hardlink on windows)
    * properly setup PATH for fish shell
    * have more robust platform-support fallbacks
* npm installers:
    * can be automatically published to npm
    * support multiple binaries
    * can install aliases for your binaries (additional bin commands on the package)
    * can have a different name from their parent cargo package
    * have more robust platform-support fallbacks
    * [have all new docs](https://opensource.axo.dev/cargo-dist/book/installers/npm.html)
* homebrew installers
    * can install aliases for your binaries (builtin homebrew aliases)
    * have more idiomatic install-hints
    * [have all new docs](https://opensource.axo.dev/cargo-dist/book/installers/homebrew.html)


## Features


### npm installer rewrite

The npm installer has been rewritten to remove almost every limitation it has.

Notably, you can now have multiple binaries in an npm package without any issue. You can even make aliases for commands with the new [bin-aliases setting](https://opensource.axo.dev/cargo-dist/book/reference/config.html#bin-aliases).

Although for the best user experience, ensuring your package [unambiguously has one true command](https://docs.npmjs.com/cli/v7/commands/npx#description) is ideal. To help with this, the [npm-package setting](https://opensource.axo.dev/cargo-dist/book/reference/config.html#npm-package) was added, allowing you to rename your npm package (which is one of the disambiguators for the primary command of an npm package).

You can also now automatically [publish your npm packages](https://opensource.axo.dev/cargo-dist/book/installers/npm.html#quickstart) by [setting `publish-jobs = ["npm"]`](https://opensource.axo.dev/cargo-dist/book/reference/config.html#publish-jobs)!

* [docs](https://opensource.axo.dev/cargo-dist/book/installers/npm.html)
* impl
    * @gankra [rewrite npm installer](https://github.com/axodotdev/cargo-dist/pull/974)
    * @gankra [add npm-package config](https://github.com/axodotdev/cargo-dist/pull/988)
    * @ashleygwilliams [add builtin npm publish](https://github.com/axodotdev/cargo-dist/pull/966)
    * @ashleygwilliams [rewrite npm installer docs](https://github.com/axodotdev/cargo-dist/pull/985)


### install-path cascades

The [install-path setting](https://opensource.axo.dev/cargo-dist/book/reference/config.html#install-paths) that's used by the shell and powershell installers can now be an array of options to try in sequence. This is mostly useful for checking if special environment variables are set before falling back to some hardcoded subdir of $HOME.

For instance, if you set:

```toml
install-dir = ["$MY_APP_HOME/bin", "~/.my-app/bin"]
```

Then your shell and powershell installers will first check if `$MY_APP_HOME` is defined and install to `$MY_APP_HOME/bin` if it is. If it's not defined then it will use the more hardcoded `$HOME/.my-app/bin`.

* [docs](https://opensource.axo.dev/cargo-dist/book/reference/config.html#install-paths)
* @mistydemeo [impl](https://github.com/axodotdev/cargo-dist/pull/947)


### binary aliases

The shell, powershell, npm, and homebrew installers all now support aliases for binaries with the new [bin-aliases setting](https://opensource.axo.dev/cargo-dist/book/reference/config.html#bin-aliases). These are not included in your downloadable archives, and are setup in an installer-specific way:

* shell: symlinks
* powershell: hardlinks
* npm: extra "bin" entries pointing at the same command
* homebrew: bin.install_symlink

* [docs](https://opensource.axo.dev/cargo-dist/book/reference/config.html#bin-aliases)
* impl
    * @mistydemeo [add binary aliases](https://github.com/axodotdev/cargo-dist/pull/964)
    * @mistydemeo [adjust feature name](https://github.com/axodotdev/cargo-dist/pull/986)
    * @mistydemeo [force symlinks to overwrite](https://github.com/axodotdev/cargo-dist/pull/997)


### publish github releases to other repo

If you need to publish your GitHub Releases to a different repo than the one you run your release workflow on, you can use the new [github-releases-repo setting](github-releases-repo) to specify this.

* [docs](https://opensource.axo.dev/cargo-dist/book/reference/config.html#github-releases-repo)
* @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/967)


### disable alternative source tarballs

You can now disable the backup source tarballs uploaded by cargo-dist with the new [source-tarball setting](https://opensource.axo.dev/cargo-dist/book/reference/config.html#source-tarball).

* [docs](https://opensource.axo.dev/cargo-dist/book/reference/config.html#source-tarball)
* @mistydemeo [impl](https://github.com/axodotdev/cargo-dist/pull/959)


## Fixes

* @gankra [rewrite homebrew installer docs](https://github.com/axodotdev/cargo-dist/pull/954)
* @mistydemeo [use more idiomatic homebrew install expressions](https://github.com/axodotdev/cargo-dist/pull/1000)
* @mistydemeo [fix references to selfupdate command](https://github.com/axodotdev/cargo-dist/pull/957)
* @gankra [unify installer platform-compatibility code](https://github.com/axodotdev/cargo-dist/pull/984)
* @mistydemeo [implement fish support in installer](https://github.com/axodotdev/cargo-dist/pull/958)


# Version 0.13.3 (2024-04-22)

This minor release adds some more resilience to release.yml by explicitly enabling windows
longpath support, in case your repository (or submodules) contain extremely long filenames or paths.

It also adds some missing dependency constraints for custom local and global build jobs.

## Fixes

* @gankra [improvements to release.yml](https://github.com/axodotdev/cargo-dist/pull/951)


# Version 0.13.2 (2024-04-16)

This minor release updates several dependencies and contains one improvement to the shell installer.

## Fixes

* @mistydemeo [update to axoasset which should fix remaining issues with recursive ZIP files on Windows](https://github.com/axodotdev/cargo-dist/pull/939) ([#873])
* @mistydemeo [install the correct architecture binary on macOS when run from a shell inside Rosetta](https://github.com/axodotdev/cargo-dist/pull/941)

[#873]: https://github.com/axodotdev/cargo-dist/issues/873


# Version 0.13.1 (2024-04-12)

This is a release that adds some improvements to the logic for updating PATH and rcfiles in installers.

## Fixes

* @gankra [don't corrupt rcfiles that are missing a trailing newline](https://github.com/axodotdev/cargo-dist/pull/917)
* @mistydemeo [when invoking shell installer from updater, append /bin and use HOME vars more precisely](https://github.com/axodotdev/cargo-dist/pull/928)
* @mistydemeo [as above, but for powershell](https://github.com/axodotdev/cargo-dist/pull/929)
* @mistydemeo [use target-specific keys to fix swatinem rust cache](https://github.com/axodotdev/cargo-dist/pull/927/)


# Version 0.13.0 (2024-04-09)

This releases introduces a 'selfupdate' command, using cargo-dist's new updater support.

It also includes several bugfixes and a technically-breaking-change to the dist-manifest feature.


## Features

A new `cargo dist selfupdate` command has been added which updates cargo dist to the latest
version and runs `cargo dist init` using that new version. As a result, this should in
be a go-to replacement for most uses of `cargo dist init`.

This is based on the new experimental [axoupdater library](https://github.com/axodotdev/axoupdater/).
Essentially it just checks GitHub Releases or axo Releases for anything newer, and fetches and
runs the appropriate shell/powershell installer.

This library is the same one used for the updater feature of cargo-dist, which has cargo-dist
provide a standalone separate updater binary. The benefit of us using the library directly is
that we get a more unified design at the cost of needing to actually change the interface of
our application -- something the separate binary avoids.

* @mistydemeo [initial implementation](https://github.com/axodotdev/cargo-dist/pull/899)
* @gankra [finalize implementation](https://github.com/axodotdev/cargo-dist/pull/906)


## Breaking Change

* @gankra [manifest reform](https://github.com/axodotdev/cargo-dist/pull/848)

This release makes a several changes to the dist-manifest format which ideally shouldn't
be breaking in the strictest sense of the word, but are breaking in spirit. No one is expected
to be effected, as the metadata in question is so niche that not even axo's own tooling
was making a use of it.

The upshot of this breakage is that we now properly collect and merge unambiguous per-platform and
per-binary metadata from build machines, which is groundwork for significantly improved installers
and tooling.

Changes include:

* there is now a top-level `systems` map in the manifest
    * contains gathered information about each system cargo-dist ran on to build your release
    * this effectively deprecates the `system_info` value, which was already optional and not terribly useful
* there is now a top-level `assets` map in the manifest
    * contains gathered information about each binary cargo-dist built
    * refers to the `systems{}` that the binary was built on
    * contains linkage info
* the top-level `linkage` array has been deprecated in favour of the same entries being nested in `assets`, and is now always empty
    * in future versions it may be removed, but the schema didn't mark this field as optional so it can't yet be removed
* `artifacts{}.assets[].id` now can optionally refer to an entry in `assets`
    * this allows you to precisely get the dependencies (linkage) for each binary in an archive, or for the whole archive (by merging them)
    * in the future it will also be used to get things like libc version requirements
* `artifacts{}.checksums{}` has been added
    * contains the actual checksum value(s)
    * the existing `artifacts{}.checksum` only refers to a checksum *file*, as previously the manifest could not be updated with "computed" info
    * this being a map allows artifacts to have multiple checksums, which is useful since lots of things hard require sha256

## Fixes

* @mistydemeo [run apt-get update before installing system deps](https://github.com/axodotdev/cargo-dist/pull/877)
* @ucodery [make formula files pass brew lint](https://github.com/axodotdev/cargo-dist/pull/818)
* @tshepang [fix copyright year](https://github.com/axodotdev/cargo-dist/pull/883)
* @mistydemeo [further update copyright year](https://github.com/axodotdev/cargo-dist/pull/884)
* @tisokun [properly spell GitHub in CI yml](https://github.com/axodotdev/cargo-dist/pull/886)
* @gankra [use mv instead of cp in installer.sh](https://github.com/axodotdev/cargo-dist/pull/894)
* @nokazn [fix broken link in docs](https://github.com/axodotdev/cargo-dist/pull/900)

# Version 0.12.2 (2024-04-04)

This is a minor release which regenerates the shell installer using the fix from 0.12.1. It fixes an issue which would cause the cargo-dist shell installer to fail if cargo-dist itself is running at the time the installer tries to write the new copy.


# Version 0.12.1 (2024-04-04)

This is a minor bugfix release.

## Fixes

* @mistydemeo [fix recursive ZIP generation on Windows](https://github.com/axodotdev/cargo-dist/pull/895)
* @Gankra [fix overwriting actively-running binary in shell installer](https://github.com/axodotdev/cargo-dist/pull/894)


# Version 0.12.0 (2024-03-21)

This release introduces an experimental new feature: an updater which allows your users to install new releases without having to go download a new installer themselves. It also includes a few other bugfixes and improvements.

## Features

### cargo-dist updater

The new cargo-dist updater, [axoupdater](https://github.com/axodotdev/axoupdater), provides a way for your users to easily upgrade your software without needing to check your website for new versions. If you enable the new `install-updater = true` option in cargo-dist, users who install your software via the shell or PowerShell installers will receive a standalone updater program alongside your program itself. Running this program will check for updates and, if necessary, install the new version for them. In addition, axoupdater provides a Rust library with all of its functionality exposed so that you can choose to integrate the updater functionality into your own program directly.

For more information, see the [cargo-dist documentation](https://opensource.axo.dev/cargo-dist/book/installers/updater.html) or consult the axoupdater repository and [documentation](http://docs.rs/axoupdater/latest/axoupdater/).

### Homebrew cask dependencies

Homebrew cask dependencies can now be installed.

* impl @mistydemeo [feat: support cask deps for Homebrew](https://github.com/axodotdev/cargo-dist/pull/855)

### PowerShell installer tests

We now run PowerShell installers end to end in cargo-dist's own tests.

* impl @Gankra [chore: add ruin_me powershell tests](https://github.com/axodotdev/cargo-dist/pull/862)

### Invoke `rustup target add` for additional forms of cross-compiling

We now run `rustup target add` unconditionally when cross-compiling; before, this was limited to macOS or for musl Linux. This is primarily useful for targeting Windows ARM, but may be useful for other cross-compilation targets in the future.

* impl @AustinWise [feat: always rustup target add in cross](https://github.com/axodotdev/cargo-dist/pull/846)

### Allow overriding install path in shell and PowerShell installers

Although some installer configuration allows setting the installation path, there was previously no way to force the installer to install to a location of the user's choosing. We've added a feature to do so via the new `CARGO_DIST_FORCE_INSTALL_DIR` environment variable.

This is primarily intended for cargo-dist's internal use, and is used by the updater to ensure that new releases are installed in the same location as the previous version; other users may find it useful.

* impl @mistydemeo [feat: allow overriding install path](https://github.com/axodotdev/cargo-dist/pull/837)

## Fixes

* @mistydemeo [fix install path in install receipts in certain circumstances](https://github.com/axodotdev/cargo-dist/pull/863)
* @Gankra [fix: for reals reals utf8 for reals FOR REALS](https://github.com/axodotdev/cargo-dist/pull/852)
* @kbattocchi [feat: enable fallback to x64 in Invoke-Installer](https://github.com/axodotdev/cargo-dist/pull/835)

# Version 0.11.1 (2024-02-23)

This release is a few minor improvements, and a new config for homebrew installers.

## Features

The name of your homebrew formula can now be overridden with `formula = "my-cool-formula"`.

* [docs](opensource.axo.dev/cargo-dist/book/reference/config.html#formula)
* impl
    * @ashleygwilliams [initial impl](https://github.com/axodotdev/cargo-dist/pull/791)
    * @gankra [add support to brew publish job](https://github.com/axodotdev/cargo-dist/pull/816)

## Fixes

* @gankra [powershell `iem | iex` exprs are now more robust and can be run from cmd](https://github.com/axodotdev/cargo-dist/pull/808). Only downside is they're more verbose.

* @saraviera [All multi-value cli flags can now be passed as `--arg x y` or `--arg=x,y`](https://github.com/axodotdev/cargo-dist/pull/744). To make this work, a minor breaking change was made to the `cargo dist generate` plumbing command: you must now pass `--mode ci` instead of `ci`. This likely affects no one.

* @mistydemeo [enabled pipefail in more places in cargo-dist's CI](https://github.com/axodotdev/cargo-dist/pull/619)

* @mistydemeo [fixed arm64 musl detection for homebrew and shell installers](https://github.com/axodotdev/cargo-dist/pull/799)



# Version 0.11.0 (2024-02-23)

This release is a few minor improvements, and a new config for homebrew installers.

## Features

The name of your homebrew formula can now be overriden with `formula = "my-cool-formula"`.

* [docs](opensource.axo.dev/cargo-dist/book/reference/config.html#formula)
* @ashleygwilliams [impl](https://github.com/axodotdev/cargo-dist/pull/791)

## Fixes

* @gankra [powershell `iem | iex` exprs are now more robust and can be run from cmd](https://github.com/axodotdev/cargo-dist/pull/808). Only downside is they're more verbose.

* @saraviera [All multi-value cli flags can now be passed as `--arg x y` or `--arg=x,y`](https://github.com/axodotdev/cargo-dist/pull/744). To make this work, a minor breaking change was made to the `cargo dist generate` plumbing command: you must now pass `--mode ci` instead of `ci`. This likely affects no one.

* @mistydemeo [enabled pipefail in more places in cargo-dist's CI](https://github.com/axodotdev/cargo-dist/pull/619)

* @mistydemeo [fixed arm64 musl detection for homebrew and shell installers](https://github.com/axodotdev/cargo-dist/pull/799)


# Version 0.10.0 (2024-02-09)

This release fixes an issue with shell installers and adds an experimental feature for letting cargo-dist coexist with other release tooling.

## Features

### tag-namespace

A new experimental config, `tag-namespace = "some-prefix"` has been added. Setting `tag-name = "owo"` will change the tag matching expression we put in your github ci, to require the tag to start with "owo" for cargo-dist to care about it. This can be useful for situations where you have several things with different tag/release workflows in the same workspace. It also renames `release.yaml` to `owo-release.yml` to make it clear it's just one of many release workflows.

* [docs](opensource.axo.dev/cargo-dist/book/reference/config.html#tag-namespace)
* @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/779)


## Fixes

### Shell Replaces

0.9.0 had a minor error in a new experimental shell installer feature that would cause scary sed errors to appear in the terminal output for apps with multiple binaries. The core functionality of the installer would work perfectly, with the only degradation in functionality being an optional "install-receipt" failing to be saved to the end-user's system.

In addition, the receipt would point to `$CARGO_HOME` instead of `$CARGO_HOME/bin` in the default configuration, which has now been changed.

* @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/776)




# Version 0.9.0 (2024-02-01)

This release contains several new features for users of GitHub Actions. In particular, we've updated the runners we use by default and we now use the latest versions of the upload-artifact and download-artifact GitHub actions.

## Features

### Newer macOS runners for GitHub Actions

In preparation for the retirement of the `macos-11` runners, we've upgraded to the slightly newer `macos-12`. We still prefer running on these older runners, and not the newer `macos-13` or `macos-14` options, for broader compatibility across OS versions.

Users who wish to take advantage of GitHub's [new Apple Silicon runners](https://github.blog/changelog/2024-01-30-github-actions-introducing-the-new-m1-macos-runner-available-to-open-source/), which entered public beta on January 30, should consider setting up custom runners in their `Cargo.toml`:

```toml
[workspace.metadata.dist.github-custom-runners]
aarch64-apple-darwin = "macos-14"
```

* @mistydemeo [impl](https://github.com/axodotdev/cargo-dist/pull/754)

### Updated to the latest upload-artifact/download-artifact GitHub actions

We now use the latest (v4) versions of the upload-artifact and download-artifact GitHub actions. There's no need to change anything in your app, and most users won't see any changes from this. However, users with custom jobs which read from or write to artifacts will need to update their jobs to account for the new structure.

* @mistydemeo [impl](https://github.com/axodotdev/cargo-dist/pull/755)


# Version 0.8.2 (2024-01-29)

This release contains a new testing feature: `--artifacts=lies`. This allows generating fake artifacts during builds instead of real artifacts, allowing for dry-run tests to proceed without the need for real cross-compilation.

## Features

### --artifacts=lies

Calling `cargo dist build --artifacts=lies` now produces stubbed out artifacts for native builds and certain installers, such as the MSI installer and the source tarball. This allows a fake build which nonetheless contains a full set of real artifacts to be run on any machine, including a a local machine without cargo cross-compilation support. This is especially useful for staging releases to test axo Releases.

* @mistydemeo + @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/741)


# Version 0.8.1 (2024-01-26)

Just a small release with a pile of bugfixes!

## Fixes

@dsully [Fix Linuxbrew installers](https://github.com/axodotdev/cargo-dist/pull/697)
@gankra [Fix zstd archive support (was never properly implemented)](https://github.com/axodotdev/axoasset/pull/77)
@gankra [Fix dependabot's interaction with axo releases](https://github.com/axodotdev/cargo-dist/pull/739)


# Version 0.8.0 (2024-01-19)

This release is a mix of quality-of-life changes and fixes.

`dispatch-releases = true` adds a new experimental mode where releases are triggered with workflow-dispatch instead of tag-push.

`build-local-artifacts = false` disables the builtin CI jobs that would build your binaries and archives (and MSI installers). This allows a Sufficiently Motivated user to use custom `build-local-jobs` to completely replace cargo-dist's binary building with something like maturin.

## Features

### dispatch-releases

`dispatch-releases = true` adds a new experimental mode where releases are triggered with workflow-dispatch instead of tag-push (relying on creating a github release implicitly tagging).

Enabling this disables tag-push releases, but keeps pr checks enabled.

By default the workflow dispatch form will have "dry-run" populated as the tag, which is taken to have the same meaning as `pr-run-mode = upload`: run the plan and build steps, but not the publish or announce ones. Currently hosting is also disabled, but future versions may add some forms of hosting in this mode.

* @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/717)
* [docs](https://opensource.axo.dev/cargo-dist/book/reference/config.html#dispatch-releases)



### build-local-artifacts

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

* @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/717)
* [docs](https://opensource.axo.dev/cargo-dist/book/reference/config.html#build-local-artifacts)



## Fixes

* better platform printing
    * @gankra [more pleasant fallback for unknown platforms](https://github.com/axodotdev/cargo-dist/pull/728)
    * @gankra [more known platforms, better display names](https://github.com/axodotdev/axoproject/pull/82)
    * @gankra [better sort](https://github.com/axodotdev/cargo-dist/pull/732)
* @gankra [add proper pr-run-mode guards to custom build jobs](https://github.com/axodotdev/cargo-dist/pull/717)
* @gankra [elevate privileges for custom publish jobs](https://github.com/axodotdev/cargo-dist/pull/722)
* @mistydemeo [tighten minimum glibc requirements in shell installer](https://github.com/axodotdev/cargo-dist/pull/727)
* @gankra [add announcement_tag_is_implicit as a --dry-run signal to dist-manifest](https://github.com/axodotdev/cargo-dist/pull/725)
* @mistydemeo [don't print warnings in some tests, making them run more reliable in some setups](https://github.com/axodotdev/cargo-dist/pull/730)

# Version 0.7.2 (2024-01-15)

This release contains a fix for certain ARM64 Linux builds.

## Fixes

Fixes a regression caused by an overly-strict check for the exit status of `ldd`. This primarily affected certain builds on ARM64 Linux and didn't affect builds on x86_64 Linux.

* impl @gankra [fix: ignore ldd status](https://github.com/axodotdev/cargo-dist/pull/711)


# Version 0.7.1 (2024-01-05)

This release lowers the MSRV for people who install with cargo-install.


# Version 0.7.0 (2024-01-04)

This release contains new customization options for CI.

## Features

### New hooks for custom jobs in CI

In [0.3.0](https://github.com/axodotdev/cargo-dist/releases/tag/v0.3.0), we added support for custom publish jobs. This feature let you customize the CI process with your own jobs at publish-time without having to edit cargo-dist's generated CI scripts, but it was limited to just that one phase. This release expands on that by adding hooks for a number of other steps of the build process. This enables fine-grained customization without having to touch any of the generated scripts. The phases at which custom jobs are available are:

* plan (the beginning of the build process)
* build-local-artifacts
* build-global-artifacts
* host (pre-publish)
* publish
* post-announce (after the release is created)

* impl @mistydemeo
    * [feat(ci): custom steps for other phases](https://github.com/axodotdev/cargo-dist/pull/632)
    * [feat(ci): post-announce jobs](https://github.com/axodotdev/cargo-dist/pull/683)


# Version 0.6.4 (2024-01-04)

This is a minor release which makes internal changes to our packaging process. The actual cargo-dist program is unchanged from 0.6.3.


# Version 0.6.3 (2024-01-02)

This is a minor release to update dependencies and add some cli flags for init.

* @Gankra [add --hosting flag to init to streamline that workflow](https://github.com/axodotdev/cargo-dist/pull/668)
* @illicitonion [Fix image reference in docs](https://github.com/axodotdev/cargo-dist/pull/670)
* @mistydemeo [attempt to more aggressively flush streams for github CI](https://github.com/axodotdev/cargo-dist/pull/679)

# 0.6.2 (2023-12-21)

This is a minor bugfix release.

## Fixes

### Upload final dist-manifest.json to Axo Releases

Fixes an issue where the non-merged `dist-manifest.json` was being uploaded to Axo Releases instead of the final, merged manifest. This issue didn't affect users of GitHub releases.

* impl @Gankra [fix: properly upload the "final" dist-manifest to axo releases](https://github.com/axodotdev/cargo-dist/pull/665)


# 0.6.1 (2023-12-20)

This is a minor bugfix release.

## Features

### Improvements to liblzma integration

This release removes an external dependency on liblzma on certain platforms.

We integrate compressed artifact support from the [axoasset](https://github.com/axodotdev/axoasset) crate. A quirk in a dependency we use means that cargo-dist builds would dynamically link against an external liblzma, but only if it was found in the build environment. As a result, some of our binaries use liblzma from the system and others use an embedded static build. This release unifies the behaviour so that every target uses a static build.

This shouldn't affect most users; we've made this change primarily for consistency. It does, however, ensure that the x86_64 macOS binaries are compatible with a wider variety of systems than they were in the past.

* impl @mistydemeo
    * [feat: use xz2 static feature](https://github.com/axodotdev/axoasset/pull/74)
    * [chore: update axoasset](https://github.com/axodotdev/cargo-dist/pull/657)

## Fixes

### Extra artifacts would always be built

A bug in our build configuration meant that we would always build extra artifacts when they're configured, even for local-only builds. They're now built only at the appropriate time.

* impl @mistydemeo [fix(extra artifacts): avoid inappropriate builds](https://github.com/axodotdev/cargo-dist/pull/661)

# 0.6.0 (2023-12-18)

The headlining features of this release are:

* Support for specifying arbitrary GitHub Actions Runners
* The ability to build and upload extra artifacts along side your main build

We also now distribute aarch64 Linux binaries, which makes it easier to use cargo-dist to build software on aarch64 hosts.

## Features

### Custom Runners in GitHub Actions

This adds support for specifying which runners to use in GitHub CI. This is useful in order to allow cargo-dist to use paid runners, rather than the free runners it defaults to, and to force Linux builds to use a newer version of Ubuntu. By using paid runners, it's also possible to create builds running on ARM64 hosts.

* impl @milesj [Support custom github runners (and arm64)](https://github.com/axodotdev/cargo-dist/pull/614)

Thanks to @milesj for contributing this!

### Build and Host Extra Artifacts

This feature makes it possible to build and upload extra artifacts beyond what the primary build produces. For example, cargo-dist uses this to build and upload its `dist-manifest-schema.json` to each release. You can use this feature to help build and upload docs, manage extra assets for your release, and more.

* impl @mistydemeo [feat: extra build artifacts](https://github.com/axodotdev/cargo-dist/pull/613)

### Generic Builds Now Set CC/CXX Environment Variables

In generic builds, the `CC` and `CXX` environment variables are now set to platform-appropriate compilers. This is mainly applicable to software written in C and C++.

* impl @mistydemeo [feat: set CC/CXX in generic builds](https://github.com/axodotdev/cargo-dist/pull/616)

### Installer improvements

The installer now updates additional shell configuration files, ensuring that users are able to use your software after installing. The installer now also respects the `ZDOTDIR` configuration variable when run in the zsh shell.

* impl @mistydemeo
    * [feat(installer): add additional shell config](https://github.com/axodotdev/cargo-dist/pull/555)
    * [fix(installer): only print source once](https://github.com/axodotdev/cargo-dist/pull/641)
    * [fix: handle unset ZDOTDIR better](https://github.com/axodotdev/cargo-dist/pull/640)

## Improvements

### Generic build output

stdout from generic build tasks is now merged with stderr at the time the job is run instead of printed separately after the build completes.

* impl @mistydemeo [feat(generic): adjust stdout=>stderr redirect](https://github.com/axodotdev/cargo-dist/pull/649)

## Fixes

### "Broken pipe" message in install script

Fixes an issue where the installer script could report a spurious "broken pipe" message in Linux. Note that this didn't affect the installer's behaviour; it still worked as expected.

* impl @rotu [Fix ldd broken pipe error](https://github.com/axodotdev/cargo-dist/pull/627)

### Better installation failure handling in CI

In the rare case that installing cargo-dist failed in CI, the build would formerly continue anyway and fail in a more confusing way. This has been corrected so that the build now fails immediately.

* impl @mistydemeo [fix(ci): fail fast if installer is missing](https://github.com/axodotdev/cargo-dist/pull/618)

### Source tarball fixes

Generating source tarballs will now be skipped if the workspace being built isn't a git repository. It will also be skipped if git isn't installed.

* impl @mistydemeo
    * [fix: check for git presence before calling](https://github.com/axodotdev/cargo-dist/pull/648)
    * [fix: check for git presence before calling](https://github.com/axodotdev/cargo-dist/pull/648)

### Improved error reporting in Powershell installer

The Windows Powershell installer now provides better error output on the terminal.

* impl @mistydemeo + @gankra [fix(powershell): replace errors with throw](https://github.com/axodotdev/cargo-dist/pull/651)


# Version 0.5.0 (2023-11-27)

This release was probably going to be several releases, but everything got finished at the same time, so here's a Mega Release!

The headline features are:

* New Support For Axo Releases, As An Alternative To Github Releases (Launching Soon™)
* New Support For Generic Build Steps, In Any Language (Experimental)
* Significantly Improved MSI Installer Support

## Features

### Axo Releases

Axo Releases users can now enable builtin cargo-dist support by setting

`hosting = ["axodotdev"]`

in their `[workspace.metadata.dist]`.

To sign up for the Axo Releases closed beta, go to https://dash.axo.dev/

You can ask for more details by [joining our discord](https://discord.gg/ECnWuUUXQk) or sending a message to `hello@axo.dev`!

Axo Releases has a more robust pipelined model for creating and hosting a release, which more
closely matches the actual design of cargo-dist. But since we'd only ever supported Github Releases,
some significant internal reckoning was required.

This reckoning primarily appears in the existence of the new "cargo dist host" subcommand, which
was created to make "side-effectful networking" explicit, instead of riddling several random commands
with various --dry-run flags.

`host` takes several --steps:

* create: ask Axo Releases to create hosting for the Apps we want to publish
* upload: upload built Artifacts to the hosting that `create` made
* release: create Releases for the hosted artifacts, making perma-urls like /v1.0.0/ live
* announce: announce all the Releases, wiring them into "list all releases" and "latest release" endpoints
* check: equivalent to `create` but just checks that authentication is properly setup, without side-effects

The distinction between upload, release, and announce in particular lets us provide a more
reliable/transactional release process -- we can make the hosting live, publish to package managers,
and *then* update URLs like /latest/ once everything works, instead of racily doing it all
at once and having to frantically hack things back to normal when something weird happens.
It should also make it possible for us to provide features like Release/PR Previews.

* docs
    * [hosting config](https://opensource.axo.dev/cargo-dist/book/reference/config.html#hosting)
* impl
    * @gankra [preparatory refactor](https://github.com/axodotdev/cargo-dist/pull/546)
    * @gankra [create gazenot client library](https://github.com/axodotdev/gazenot)
    * @mistydemeo [break tag parsing into "axotag" crate](https://github.com/axodotdev/cargo-dist/pull/567)
    * @gankra [properly set announcement body for abyss](https://github.com/axodotdev/cargo-dist/pull/586)
    * @mistydemeo [add a comment about Axo Releases beta](https://github.com/axodotdev/cargo-dist/pull/600)
    * @gankra [cleanup github releases / ci contents](https://github.com/axodotdev/cargo-dist/pull/596)


### Generic Builds

0.5.0 contains experimental support for building non-cargo-based projects. These can be in any language, and follow any repository layout, so long as they're accompanied by a cargo-dist manifest file that provides information on how to build and install it. For more information, consult the documentation.

* docs
    * [guide](https://opensource.axo.dev/cargo-dist/book/generic-builds.html)
    * [example npm project](https://github.com/axodotdev/axolotlsay-js)
    * [example C project](https://github.com/axodotdev/cargo-dist-c-example)
* impl
    * @mistydemeo [add generic project type](https://github.com/axodotdev/axoproject/pull/45)
    * @mistydemeo [handle missing PackageId](https://github.com/axodotdev/cargo-dist/pull/549)
    * @mistydemeo [implement generic builds](https://github.com/axodotdev/cargo-dist/pull/553)
    * @mistydemeo [rebase fixup](https://github.com/axodotdev/cargo-dist/pull/569)
    * @mistydemeo [print stdout from generic builds](https://github.com/axodotdev/cargo-dist/pull/570)
    * @mistydemeo [fix --artifacts=global with generic builds](https://github.com/axodotdev/cargo-dist/pull/573)


### MSI

We've contributed several upstream improvements to cargo-wix, the tool we use to build MSIs, and integrated
that functionality back into cargo-dist.

Where previously you needed to use cargo-wix CLI flags to set various images in your installers,
they are now exposed in `[package.metadata.wix]` as well as `banner`, `dialog`, and `product-icon`.

There are now also `eula` and `license` configs on `[package.metadata.wix]` that allow you to specify
where to source the eula/license from, and also allow you to explicitly disable auto-eula/auto-license
functionality with `eula = false` and `license = false`. `cargo dist init` will by default set those
to false if it sees they aren't defined in `[package.metadata.wix]` yet, making things more well-behaved
by default. To restore the old auto-eula behaviour, set them to `true`.

In addition, significant refactoring was done to the eula/license backend of cargo-wix so that cargo-dist
can properly understand when those files need to be auto-generated. Previously auto-generated licenses/eulas
would just produce broken templates, because cargo-dist wouldn't know about them and get confused.

* docs
    * [cargo-wix docs](https://volks73.github.io/cargo-wix/cargo_wix/#configuration)
    * [cargo-dist msi docs](https://opensource.axo.dev/cargo-dist/book/installers/msi.html)
* impl
    * @gankra [refactor eulas and add new config](https://github.com/volks73/cargo-wix/pull/247)
    * @gankra [add config for setting installer images](https://github.com/volks73/cargo-wix/pull/250)
    * @gankra [use new cargo-wix features](https://github.com/axodotdev/cargo-dist/pull/503)


### Source Tarballs

cargo-dist will now generate its own source tarballs, and upload them to your release, named "source.tar.gz". The source tarballs that github provides are actually generated on demand with unspecified settings, so to ensure both Axo Releases and Github Releases have access to the same results, we need cargo-dist to generate the source tarball itself. We use the same mechanism as Github (asking git itself to generate them), but we can't bitwise-identically reproduce their (unspecified, technically-not-guaranteed) behaviour.

* @mistydemeo [impl](https://github.com/axodotdev/cargo-dist/pull/604)


## Maintenance/Fixes

* @rukai [Remove rust-toolchain-version from the workspaces setup guide](https://github.com/axodotdev/cargo-dist/pull/578)
* @jwodder [Give "upload-local-artifacts" jobs friendlier display names](https://github.com/axodotdev/cargo-dist/pull/557)


# Version 0.4.3 (2023-11-08)

This is a small bugfix release which resolves an issue where we would sometimes generate non-working Homebrew installers.

* @mistydemeo [Homebrew: Fixed an issue where generated class names might not match the name Homebrew looks for](https://github.com/axodotdev/cargo-dist/pull/554)

# Version 0.4.2 (2023-10-31)

Just a little release to get a couple small fixes in people's hands!

* @mistydemeo [Linkage report: Fixed an issue where Linux libraries not associated with an apt package would be followed by ()](https://github.com/axodotdev/cargo-dist/pull/525)
* @gankra [Includes: check for existence of included files/dirs as late as possible to allow build.rs to generate them](https://github.com/axodotdev/cargo-dist/pull/528)

(This is a rerelease of 0.4.1, because that one wasn't properly rebased to include all the advertised fixes.)

# Version 0.4.1 (2023-10-30)

(See 0.4.2 for the actual release)

Just a little release to get a couple small fixes in people's hands!

* @mistydemeo [Linkage report: Fixed an issue where Linux libraries not associated with an apt package would be followed by ()](https://github.com/axodotdev/cargo-dist/pull/525)
* @gankra [Includes: check for existence of included files/dirs as late as possible to allow build.rs to generate them](https://github.com/axodotdev/cargo-dist/pull/528)


# Version 0.4.0 (2023-10-25)

This release contains several major features related to package dependencies. cargo-dist can now install dependencies for you in CI, ensure your users have those dependencies in their installers, and provide you insights into what external libraries your package links against! It also enables support for statically-built musl binaries on Linux.

## Features

### Install custom dependencies

Way back in our [very first blog post](https://blog.axo.dev/2023/02/cargo-dist), we wrote about how users could customize the GitHub CI scripts we output to install custom dependencies. As of cargo-dist 0.4.0, you won't need to do that anymore! System dependencies &mdash; that is, dependencies installed via the system's package manager instead of `cargo` &mdash; can now be specified in your cargo-dist config in `Cargo.toml` using a syntax very similar to how your `cargo` dependencies are specified. For example:

```toml
[workspace.metadata.dist.dependencies.homebrew]
cmake = { targets = ["x86_64-apple-darwin"] }
libcue = "2.2.1"

[workspace.metadata.dist.dependencies.apt]
cmake = '*'
libcue-dev = { version = "2.2.1-2" }
```

For more information, see the [documentation](https://opensource.axo.dev/cargo-dist/book/reference/config.html#dependencies).

* impl
    * @mistydemeo [initial impl](https://github.com/axodotdev/cargo-dist/pull/428)
    * @mistydemeo [improve Homebrew integration](https://github.com/axodotdev/cargo-dist/pull/504)


### Find out what your builds linked against

Complementing the ability to specify system dependencies, we've added a new feature that lets you tell which libraries your Rust programs have dynamically linked against. While most Rust software is statically linked, installing external dependencies may mean that your software links against something on the system; you can visualize which libraries your software uses, and which packages they come from, by viewing the output of the build step in CI.

In addition, cargo-dist now uses this information to choose which dependencies to specify when building system package manager installers such as a Homebrew formula. If cargo-dist detects that your binary links against a package provided by Homebrew, it will ensure that a user who `brew install`s your package will also get that other package.

This feature has full support for macOS and Linux. On Windows, we're not able to list which package a system library comes.

* impl
    * @mistydemeo [initial impl](https://github.com/axodotdev/cargo-dist/pull/426)
    * @mistydemeo [infer dependencies via linkage](https://github.com/axodotdev/cargo-dist/pull/475)
    * @mistydemeo [fetch full name of Homebrew tap](https://github.com/axodotdev/cargo-dist/pull/474)
    * @mistydemeo [improve apt package resolution](https://github.com/axodotdev/cargo-dist/pull/495)


### musl support

This release adds support for a long-requested feature, creating Linux binaries statically linked against musl instead of glibc. These can be enabled adding the `x86_64-unknown-linux-musl` target triple to your list of desired targets.

Note that because these binaries are statically linked, they cannot dynamically link against any other C libraries &mdash; including C libraries installed using the system dependency feature mentioned above. If your software links against system libraries, please ensure that a static library is available to the build.

* impl
    * @mistydemeo [initial impl](https://github.com/axodotdev/cargo-dist/pull/483)
    * @gankra + @mistydemeo [use musl binaries in installers](https://github.com/axodotdev/cargo-dist/pull/497)

### msvc-crt-static opt-out

cargo-dist has [always forced +crt-static on, as it is considered more correct for targetting Windows with the typical statically linked Rust binary](https://github.com/rust-lang/rfcs/blob/master/text/1721-crt-static.md). However with the introduction of initial support for chocolatey as a system package manager, it's now very easy for our users to dynamically link other DLLs. Once you do, [it once again becomes more correct to dynamically link the windows crt, and to use systems like Visual C(++) Redistributables](https://github.com/axodotdev/cargo-dist/issues/496).

Although we [would like to teach cargo-dist to handle redistributables for you](https://github.com/axodotdev/cargo-dist/issues/496), we're starting with a simple escape hatch: if you set `msvc-crt-static = false` in `[workspace.metadata.dist]`, we'll revert to the typical Rust behaviour of dynamically linking the CRT.

* @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/507)
* [docs](https://opensource.axo.dev/cargo-dist/book/reference/config.html#msvc-crt-static)


# Version 0.3.1 (2023-09-28)

This is a minor bugfix release which fixes an issue certain builds would encounter on Windows.

## Fixes

### Windows builds run under Powershell

Starting in version 0.3.0, we switched Windows builds to run under bash instead of Powershell. This introduced problems for certain builds, so we've switched them back to Powershell.

The majority of users will not be affected by this and will not need to upgrade; this primarily affects a limited number of users building software with libraries or dependencies which are sensitive to the shell in which they're built. For example, users building OpenSSL on Windows as a part of their cargo-dist build may have been affected.

* @frol + @mistydemeo [impl](https://github.com/axodotdev/cargo-dist/pull/461)


# Version 0.3.0 (2023-09-27)

This release is a big overhaul of cargo-dist's UX! [Our CI scripts have been completely redesigned](https://opensource.axo.dev/cargo-dist/book/introduction.html#distributing) to allow your release process to be tested in pull-requests, so you don't have to worry as much about your release process breaking!

Since we can now test your release process frequently, we've also made most cargo-dist commands default to erroring out if anything is out of sync and needs to be regenerated.

To make this easier, we've also introduced an experimental new system for [user-defined hooks](https://opensource.axo.dev/cargo-dist/book/ci/github.html#custom-jobs), allowing you to write custom publish jobs without having to actually edit release.yml.

This release also introduces initial support for msi installers with the wonderful help of [cargo-wix](https://github.com/volks73/cargo-wix)!



## Features

### CI redesign

This is the big ticket item of the release, the CI has been completely redesigned! We recommend reading the docs below for details, but some high-level details:

* The CI now runs `cargo dist plan` on pull-requests
* This can be cranked up to `cargo dist build`, with results uploaded to the PR workflow, allowing you to download+test them
* To do this, we now use GitHub's upload-artifact/download-artifact system, instead of using a draft GitHub release as scratch storage
* This means we also no longer create a draft Release on startup, and instead transactionally create the full Release at the very end
* `cargo dist plan` will now check that the CI script is up to date and not hand-edited (can be opted out)
    * The user-defined publish jobs feature helps you avoid hand-edits
    * More such features are in the pipeline for the next release!

* impl
    * @mistydemeo + @gankra [initial impl](https://github.com/axodotdev/cargo-dist/pull/378)
    * @gankra [cleanup init logic](https://github.com/axodotdev/cargo-dist/pull/392)
    * @mistydemeo [use checkout@v4](https://github.com/axodotdev/cargo-dist/pull/442)
    * @mistydemeo [add docs](https://github.com/axodotdev/cargo-dist/pull/443)

* docs
    * [high-level summary](https://opensource.axo.dev/cargo-dist/book/introduction.html#distributing)
    * [detailed docs](https://opensource.axo.dev/cargo-dist/book/ci/github.html)

### user-defined publish jobs

You can now define custom hand-written publish jobs that cargo-dist's CI will know how to invoke, without actually having to hand-edit release.yml!

* @mistydemeo [impl](https://github.com/axodotdev/cargo-dist/pull/417)
* [docs](https://opensource.axo.dev/cargo-dist/book/ci/github.html#custom-jobs)

### default to not publishing prereleases to homebrew

Homebrew doesn't have a notion of package "versions", there is Only The Latest Version, so we changed the default to only publishing to your homebrew tap if you're cutting a stable release. You can opt back in to the old behaviour with `publish-prereleases = true`.

* @mistydemeo [impl](https://github.com/axodotdev/cargo-dist/pull/401)
* [docs](https://opensource.axo.dev/cargo-dist/book/reference/config.html#publish-prereleases)

### generate command

This feature is a bit of an internal affair that you don't necessarily need to care about, but it's big enough that we figured it's worth mentioning.

The "plumbing" `generate-ci` command which is invoked by `cargo dist init` has been reworked into a more general `generate` command, as the introduction of msi installers means we now have two kinds of checked-in generated output.

Most notably, `generate --check` now exists, which produces an error if `generate` would change the contents (ignoring newline-style). **Most cargo-dist commands now run `generate --check` on startup, making it an error to have your release.yml out of date or hand-edited**. This is a key piece to the puzzle of the new CI design, as it lets you catch issues with your release process in PRs.

The `allow-dirty = ["ci"]` config was introduced to disable these `generate` modifying or checking release.yml, for users that still really need to hand-edit. We're actively working on several features that should make it less necessary to do hand-edits.

* impl
    * @mistydemeo [initial impl](https://github.com/axodotdev/cargo-dist/pull/381)
    * @gankra [generalize for msi](https://github.com/axodotdev/cargo-dist/pull/391)
    * @gankra [improved --allow-dirty behaviour](https://github.com/axodotdev/cargo-dist/pull/397)
    * @mistydemeo [default to --artifacts=all in generate](https://github.com/axodotdev/cargo-dist/pull/410)
    * @gankra [ignore newline style when checking file equality](https://github.com/axodotdev/cargo-dist/pull/414)
    * @mistydemeo [hide generate-ci alias command](https://github.com/axodotdev/cargo-dist/pull/434)
    * @gankra [cleanup more references to generate-ci](https://github.com/axodotdev/cargo-dist/pull/444)
* docs
    * [generate cli command](https://opensource.axo.dev/cargo-dist/book/reference/cli.html#cargo-dist-generate)
    * [allow-dirty config](https://opensource.axo.dev/cargo-dist/book/reference/config.html#allow-dirty)

### msi installer

Initial msi installer support is here, based on the wonderful [cargo-wix](https://volks73.github.io/cargo-wix/cargo_wix/). We contributed several upstream improvements to cargo-wix for our purposes, and look forward to helping out even more in the future!

* impl
    * @gankra [initial impl](https://github.com/axodotdev/cargo-dist/pull/370)
    * @gankra [properly handle multiple subscribers to a binary](https://github.com/axodotdev/cargo-dist/pull/421)
    * @gankra [don't forward WiX output to stdout](https://github.com/axodotdev/cargo-dist/pull/418)
* [docs](https://opensource.axo.dev/cargo-dist/book/installers/msi.html)

## Fixes

### more useful checksum files

The checksum files we generate are now in the expected format for tools like sha256sum, making them more immediately useful.

* @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/420)

## Maintenance

### more polished cli output

CLI Output has been streamlined and cleaned up a lot in this release!

* @gankra [remove redundant output](https://github.com/axodotdev/cargo-dist/pull/411)
* @gankra [various improvements](https://github.com/axodotdev/cargo-dist/pull/437)
* @gankra [better help diagnostics](https://github.com/axodotdev/cargo-dist/pull/447)

### refreshed docs

The docs have been significantly reworked to reflect how much cargo-dist has changed and improved over the last few releases. Installers have rapidly grown from "something we're trying out" to "the star of the show", so they're now front-and-center with room for their own guides.

This was a big undertaking, and not everything has been reworked yet. Further improvements will be done more incrementally.

* @gankra [big docs overhaul](https://github.com/axodotdev/cargo-dist/pull/451)
* @mistydemeo [don't suggest --profile in install instructions](https://github.com/axodotdev/cargo-dist/pull/404)
* @tshepang [make search more useful](https://github.com/axodotdev/cargo-dist/pull/386)
* @tshepang [remove stray char](https://github.com/axodotdev/cargo-dist/pull/388)


# Version 0.2.0 (2023-08-30)

This release includes a bunch of features that resolve several of our user's needs.

* Support for creating Homebrew packages on macOS and automatically uploading them to a private tap
* Ability to specify `--features` your application should be built with for production releases
* Ability to use more tag formats like `0.1.0`, `releases/v0.1.0`, `my-app/1.0.0`, etc.
* Ability to Bring Your Own Github Release (BYOGR) that cargo-dist uploads to

In the background of these changes we've also been working on improving some of the architecture
of cargo-dist to make it easier to add new installers and publishing steps.

## Features

### Homebrew Formula Support

Generating a Homebrew formula can be enabled by adding `"homebrew"` to the list
of installers in `Cargo.toml`. The formula file can be automatically uploaded
to a tap to simplify `brew install`.

This also introduces the first hint of the publish-jobs config, which will quickly
grow support for automatically publishing to crates.io, npm, and more!

* impl
    * @gankra [split out global task and have it fetch local results](https://github.com/axodotdev/cargo-dist/pull/333)
    * @gankra [properly pass --dir to gh release download](https://github.com/axodotdev/cargo-dist/pull/336)
    * @mistydemeo [Homebrew formula file](https://github.com/axodotdev/cargo-dist/pull/318)
    * @mistydemeo [Pushing to Homebrew tap](https://github.com/axodotdev/cargo-dist/pull/340)
    * @mistydemeo [Add publish-jobs config](https://github.com/axodotdev/cargo-dist/pull/359)
    * @mistydemeo [Add explicit version tag](https://github.com/axodotdev/cargo-dist/pull/348)
    * @mistydemeo [Fix Homebrew messages in init](https://github.com/axodotdev/cargo-dist/pull/353)
    * @mistydemeo [Add Homebrew docs](https://github.com/axodotdev/cargo-dist/pull/341)
* [docs](https://opensource.axo.dev/cargo-dist/book/installers.html#homebrew)

### Feature Flags

You can now change which Cargo features cargo-dist builds your project with, by setting `features`, `all-features`, and `default-features` on `[package.metadata.dist]` (and `[workspace.metadata.dist]` but this is less likely to be what you want for non-trivial workspaces).

This is useful for projects which choose to have the default features for their project set to something other than the "proper" shipping configuration. For instance if your main package is both a library and an application, and you prefer to keep the library as the default for people depending on it. If all the "app" functionality is hidden behind a feature called "cli", then `features = ["cli"]` in `[package.metadata.dist]` will do what you want.

If you enable any of these features, we may automatically turn on `precise-builds` to satisfy the requirements.

See the docs for all the details.

* @gankra + @Yatekii [impl](https://github.com/axodotdev/cargo-dist/pull/321)
* docs
    * [features](https://opensource.axo.dev/cargo-dist/book/config.html#features)
    * [all-features](https://opensource.axo.dev/cargo-dist/book/config.html#all-features)
    * [default-features](https://opensource.axo.dev/cargo-dist/book/config.html#default-features)
    * [precise-builds](https://opensource.axo.dev/cargo-dist/book/config.html#precise-builds)

### Tag Formats

cargo-dist's git tag parser has been made much more robust and permissive:

* You can now prefix release tags with anything preceding a '/'
* The 'v' prefix on a version is now optional
* You can now use package-name/v1.0.0 instead of package-name-v1.0.0

Putting this all together, all of these formats are now allowed:

* unified (release everything with the given version)
  * v1.0.0
  * 1.0.0
  * blah/blah/v1.0.0
  * whatever/1.0.0
* precise (release only this one package)
  * package-name-v1.0.0
  * package-name-1.0.0
  * package-name/v1.0.0
  * package-name/1.0.0
  * blah/blah/package-name/v1.0.0
  * blah/blah/package-name/1.0.0
  * blah/blah/package-name-v1.0.0
  * blah/blah/package-name-1.0.0

And of course `-prerelease.1`-style suffixes can be added to any of those.

Thanks to @Sharparam for all the great work on the implementation and docs for this!

* @Sharparam + @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/346)
* [docs](https://opensource.axo.dev/cargo-dist/book/workspace-guide.html#announcement-tags)

### Bring Your Own Github Release

A new `create-release` config has been added, which makes cargo-dist interoperate with things like
[release drafter](https://github.com/release-drafter/release-drafter/) which create a draft body/title
for your Github Release.

When you set `create-release = false` cargo-dist will assume a draft Github Release for the current git tag already exists with the title/body you want, and just upload artifacts to it. At the end of a successful publish it will undraft the Github Release.

* @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/367)
* [docs](https://opensource.axo.dev/cargo-dist/book/config.html#create-release)


### Enhanced Release Description

The table of release artifacts has been improved and now resembles the version
in oranda.

* @mistydemeo [impl](https://github.com/axodotdev/cargo-dist/pull/357)

## Fixes

* @mistydemeo [Fix a typo in deprecated rustup update lines](https://github.com/axodotdev/cargo-dist/pull/342)
* @gankra [Fixes handling of cargo --message-format](https://github.com/axodotdev/cargo-dist/pull/363)
* @mistydemeo [Fixes handling repository URLs that end in .git](https://github.com/axodotdev/cargo-dist/pull/298).

## Maintenance

Thanks to everyone who contributed docs and cleanups, the real MVPs!!!

* @Sharparam [remove unreachable code in installer.sh](https://github.com/axodotdev/cargo-dist/pull/345)
* @orhun [update instructions for Arch Linux](https://github.com/axodotdev/cargo-dist/pull/326)
* @tshepang [various](https://github.com/axodotdev/cargo-dist/pull/375) [fixes](https://github.com/axodotdev/cargo-dist/pull/328) [throughout](https://github.com/axodotdev/cargo-dist/pull/330) [the docs](https://github.com/axodotdev/cargo-dist/pull/331)



# Version 0.1.0 (2023-08-11)

The standout features of this release are custom install paths ("install my app to `~/.my-app` and add that to PATH"), archive checksums (releases should now include `my-app.tar.xz.sha256`), and refined builds (builds are more fault-tolerant, lower latency, and you can opt out of building `--workspace`).

To update your cargo-dist config and release.yml [install cargo dist 0.1.0](https://opensource.axo.dev/cargo-dist/) and run `cargo dist init` (you should also remove rust-toolchain-version from your config, it's deprecated).

The codebase also got some major cleanups to make it easier to contribute and iterate on installers. All templates are now migrated to jinja2 (as opposed to adhoc string replace), and we have integration tests that can validate that installers work as intended.



## Features

### custom install paths

One of our most frequently requested features is here, custom install paths! (And also installers adding things to PATH!)

When using cargo-dist's script installers (`shell` and `powershell`), we need to unpack the binaries to somewhere that will be useful for the user. By default cargo-dist will install to `$CARGO_HOME` (`~/.cargo/bin/`), because for our userbase (and many CI environments) that tends to be a user-local directory that's already on PATH (and yes we now properly check and respect `$CARGO_HOME`!).

With this feature not only can you customize where binaries get installed to, but the installer scripts now also understand how to check if that directory is on PATH, and if not register it in the appropriate places (and tell the user how to refresh PATH).

The new install-path config currently takes 3 possible formats (that we will surely expand with a lot more options very quickly):

* "CARGO_HOME": explicitly requests the default behaviour
* "~/.myapp/some/subdir": install to the given subdirectory of $HOME
* "$MY_ENV_VAR/some/subdir/" install to the given subdirectory of $MY_ENV_VAR

(Note that `$HOME/some/subdir` is not equivalent to `~/some/subdir` for various reasons, just always use the latter and we'll take care of those details for you.)

See the docs for finer details, caveats, and future plans.

* docs
    * [install-path](https://opensource.axo.dev/cargo-dist/book/config.html#install-path)
    * [shell installer](https://opensource.axo.dev/cargo-dist/book/installers.html#shell)
    * [powershell installer](https://opensource.axo.dev/cargo-dist/book/installers.html#powershell)
* impl
    * @gankra [add install-path](https://github.com/axodotdev/cargo-dist/pull/284)
    * @gankra [teach scripts to edit PATH](https://github.com/axodotdev/cargo-dist/pull/293)


### archive checksums

By default all archives will get a paired checksum file generated and uploaded to the release (default sha256). So for instance if you produce `my-app-x86_64-unknown-linux-gnu.tar.gz` then there will also be `my-app-x86_64-unknown-linux-gnu.tar.gz.sha256`. This can be configured with the new `checksum` config.

* [docs](https://opensource.axo.dev/cargo-dist/book/config.html#checksum)
* @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/243)



### refined builds

Several changes were made to the way we build your projects, and more knobs were added to the `[workspace.metadata.dist]` config to allow you to tune the build to suit your needs.

By default we now:

* Build all target platforms on separate machines/tasks, improving concurrency and fault-tolerance (**GitHub Releases should now be twice as fast!**). Previously we would attempt to merge tasks to reduce the number of machines, infamously making both x64 mac and arm64 mac share a machine, doubling the latency of releases. You can get the old behaviour by setting `merge-tasks = true`.

* Allow all build tasks to continue running, even if one of them fails (the GitHub Release will only be auto-undrafted if *all* builds pass). This allows you to salvage as much of a release as possible if only one of your target platforms has a broken build, potentially manually rerunning the task. You can get the old behaviour be setting `fail-fast = true`.

* Recursively checkout submodules when fetching your project to build (seems harmless if you don't need it, and makes us work with more projects).

* Do not try to set the toolchain in rustup when rust-toolchain-version isn't set (and that config is now deprecated, so you should unset it). Pinning of compiler toolchains is really common in major projects like Firefox with dedicated release engineers, but it's kinda overkill for smaller projects. On balance we think letting your release toolchain silently update over time as your infra updates is a better default for most projects (especially since Rust is really good at stability). Anyone who really wants toolchain pinning would be better served by using rust-toolchain.toml (so that integration tests and local dev also check the toolchain used for releases).

In addition, you can now set `precise-builds = true` if you don't want us to build your apps with `--workspace`. There's a lot of complicated factors involved here but basically the difference is in how feature selection works in Cargo when you have multiple packages sharing a workspace. `--workspace` gets you a maximal default, precise-builds gets you a minimal default. For most projects there won't be a difference.

* docs
    * [precise-builds](https://opensource.axo.dev/cargo-dist/book/config.html#precise-builds)
    * [merge-tasks](https://opensource.axo.dev/cargo-dist/book/config.html#merge-tasks)
    * [fail-fast](https://opensource.axo.dev/cargo-dist/book/config.html#fail-fast)
    * [rust-toolchain-version](https://opensource.axo.dev/cargo-dist/book/config.html#rust-toolchain-version)
* impl
    * @gankra [precise-builds + merge-tasks](https://github.com/axodotdev/cargo-dist/pull/277)
    * @gankra [fail-fast](https://github.com/axodotdev/cargo-dist/pull/276)
    * @gankra [recursively checkout submodules](https://github.com/axodotdev/cargo-dist/pull/248)
    * @gankra [deprecate rust-toolchain-version](https://github.com/axodotdev/cargo-dist/pull/275)



### orchestration features

A few new CLI features were added to cargo-dist to enable more programmatic manipulation of it. These are mostly uninteresting to normal users, and exist to enable future axo.dev tools that build on top of cargo-dist.

* `cargo dist init --with-json-config=path/to/config.json`
    * [docs](https://opensource.axo.dev/cargo-dist/book/cli.html#--with-json-config-with_json_config)
    * @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/279)
* The dist-manifest-schema.json is now properly hosted in releases
    * [docs](https://opensource.axo.dev/cargo-dist/book/schema.html)
    * @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/280)


### changelog "Unreleased" section

When parsing your changelog, prereleases can now also match the special "Unreleased" heading,
making it easier to keep a changelog for the upcoming release without committing to its version.

* [docs](https://opensource.axo.dev/cargo-dist/book/simple-guide.html#release-notes)
* @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/250)



## Fixes

### including directories

The `include` config will now work properly if you provide it a path to a directory
(the functionality was stubbed out but never implemented).

* [docs](https://opensource.axo.dev/cargo-dist/book/config.html#include)
* @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/295)


### release.yml license

At the request of end users, we've added a small legal notice at the top of the generated github release.yml file to indicate that the contents of the file are permissibly licensed. This hopefully makes it easier for package distributors and employees at large companies w/legal review to confidently use cargo-dist!

* @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/310)

## Maintenance

* codebase broken up into more files
    * @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/294)

* more code pulled out to axoasset
    * @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/295)

* migrated all generated files to jinja2 templates
    * @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/297)

* added an integration test "gallery" of projects that use cargo-dist
    * @gankra [initial impl](https://github.com/axodotdev/cargo-dist/pull/292)
    * @gankra [improvements](https://github.com/axodotdev/cargo-dist/pull/296)
    * @gankra [improvements](https://github.com/axodotdev/cargo-dist/pull/299)
    * @gankra [improvements](https://github.com/axodotdev/cargo-dist/pull/300)
    * @gankra [improvements](https://github.com/axodotdev/cargo-dist/pull/302)

* other great cleanups/fixes
    *  @striezel [fix typos](https://github.com/axodotdev/cargo-dist/pull/254)





# Version 0.0.7 (2023-05-09)

This is just a quick little release that makes the npm package tarballs we can generate
look like "properly" packed tarballs that can be directly published to npm without unpacking them.
This allows you to `npm publish URL_TO_TARBALL` directly without any issues.

@gankra [impl](https://github.com/axodotdev/cargo-dist/pull/246)




# Version 0.0.6 (2023-05-03)

This release is a pretty big improvement to cargo-dist's UX!

* `cargo dist init` is now interactive and useful for updating your config/install. This is especially useful for updating your project to a new version of cargo-dist, or enabling new installers, as the interactive UI will automatically prompt you to do so and help you keep your config coherent. It also reduces the chances of your CI script getting out of sync, as it runs generate-ci at the end for you. If you want the old non-interactive behaviour, just pass `--yes` which auto-accepts all recommendations.
    * [docs](https://opensource.axo.dev/cargo-dist/book/way-too-quickstart.html#setup)
    * impl
        * @gankra [initial impl](https://github.com/axodotdev/cargo-dist/pull/227)
        * @gankra [fixups](https://github.com/axodotdev/cargo-dist/pull/230)

* Support for generating an npm project that installs your app into node_modules! Just add "npm" to your installers (using `cargo dist init` for this is recommended, as it will prompt you to make any other necessary changes to your config).
    * [docs](https://opensource.axo.dev/cargo-dist/book/installers.html#npm)
    * impl:
        * @gankra [initial impl](https://github.com/axodotdev/cargo-dist/pull/210)
        * @gankra [fixups](https://github.com/axodotdev/cargo-dist/pull/219)
        * @frol [fix logging](https://github.com/axodotdev/cargo-dist/pull/224)
        * @shadows-withal [support package.json keywords](https://github.com/axodotdev/cargo-dist/pull/228)


* `cargo dist plan` is a new command for getting a local preview of what your release CI will build. (This is just a synonym for `cargo dist manifest` but with nicer defaults for what you *usually* want.)
    * [docs](https://opensource.axo.dev/cargo-dist/book/way-too-quickstart.html#check-what-ci-will-build)
    * impl
        * @gankra [initial impl as "status"](https://github.com/axodotdev/cargo-dist/pull/230)
        * @gankra [rename "status" to "plan"](https://github.com/axodotdev/cargo-dist/pull/232)

* Bare `cargo dist` is no longer a synonym for `build` and now just prints help. This makes it a bit nicer to get your footing with cargo-dist, as we don't suddenly do builds or complain about not being initialized on first run.
    * @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/230)

* Artifact names no longer contain redundant version numbers, so `my-app-v1.0.0-installer.sh` is now just `my-app-installer.sh`. This makes it possible to statically link the "latest" build with this format: https://github.com/axodotdev/cargo-dist/releases/latests/download/cargo-dist-installer.sh
    * @gankra [impl](https://github.com/axodotdev/cargo-dist/commit/8a417f239ef8f8e3ab66c46cf7c3d26afaba1c87)

* The compression format used for executable-zips can now be set with `windows-archive` and `unix-archive` configs. Supported values include ".tar.gz", ".tar.xz", ".tar.zstd", and ".zip". The defaults (.zip on windows, .tar.xz elsewhere) are unchanged, as we believe those have the best balance of UX and compatibility.
    * [docs](https://opensource.axo.dev/cargo-dist/book/config.html#windows-archive)
    * @gankra [impl](https://github.com/axodotdev/cargo-dist/pull/211)

* other great cleanups/fixes
    * @AlexITC [fix typo at README.md](https://github.com/axodotdev/cargo-dist/pull/203)
    * @jwodder [remove trailing spaces from templates](https://github.com/axodotdev/cargo-dist/pull/213)
    * @jwodder [fix broken links in book](https://github.com/axodotdev/cargo-dist/pull/215)
    * @jwodder [remove useless uses of cat from release.yml](https://github.com/axodotdev/cargo-dist/pull/223)
    * @gankra [factor out and use axoproject](https://github.com/axodotdev/cargo-dist/pull/207)
    * @gankra [factor out and use axocli](https://github.com/axodotdev/cargo-dist/pull/209)
    * @gankra [0.0.6 docs blitz](https://github.com/axodotdev/cargo-dist/pull/231)
    * @gankra [fix config subsetting](https://github.com/axodotdev/cargo-dist/pull/234)



# Version 0.0.5 (2023-03-15)

This is a bug-fix release for an issue with cross-platform line endings that affected
users who installed cargo-dist with `cargo install`. Prebuilt binaries were unaffected.
Specifically folks reported in [#181] that they were seeing the Shell installer (for Mac and Linux)
be generated with mixed CRLF and LF line endings, which was causing both functionality
and development issues (git churn).

For those unfamiliar- the line endings on Windows machines are different than those
on Mac and Linux ones and it can cause a lot of unfortunate chaos.

There are 2 styles of control characters to mark a line break in a text file:

- `LF`, (`\n`), Linux/Mac: `LF` stands for "Line Feed"
- `CRLF`, (`\r\n`), Windows: `CR` stands for "Carriage Return"

The presence of CRLF line endings in a shell script will cause issues. Similarly LF
line endings in a powershell script will cause issues. (Citation needed on the powershell
thing but sure let's play it safe/idiomatic here.)

The problem was that the `.crate` uploaded to crates.io had CRLF endings in some templates
because `cargo publish` was run on windows and the git repo was configured to checkout files
with platform-specific endings. The prebuilt binaries were checked out and built on linux
(Github CI), and so only used LF endings.

The reason we got *mixed* LF and CRLF is because the contents of the installer scripts come from
mixed sources: the bulk comes from template files on disk, but a few key lines are injected
programmatically by rust code with `writeln` (and `write` with manual `\n`). Note that Rust's
println/writeln are guaranteed to emit LF on all platforms (because really CRLF should just be
fazed out and platform-specific writeln would be a mess). This was good and desirable, the
main screw up was the line endings in the stored template being forwarded verbatim instead
of all being rewritten to LF.

To be EXTRA SURE this doesn't happen in the future we just straight up rewrite all newlines
before writing the final result, making the newlines stored in cargo-dist's git repo irrelevant.

[181]: https://github.com/axodotdev/cargo-dist/issues/181

# Version 0.0.4 (2023-03-03)

This is a smaller release than originally planned to get some platform support that was blocking folks out the door. Features that were originally planned for this one will ideally be part of the next release.

* aarch64-apple-darwin ("apple silicon"/"arm64 macos") is now properly supported, and can be cross-compiled from x64 macos (and x64 can be crossed from arm64)
    * if you have rustup installed we will `rustup target add` before attempting the build, as this is the only requirement (thanks for making it easy, Apple!)
    * add this target to your Cargo.toml before you `cargo dist regenerate-ci` to make sure the CI knows to build it!
    * currently both mac builds will be multiplexed onto the same runner. this will increase latency of your releases but should reduce the total resource usage of your CI (by avoiding fixed overheads). We might make this configurable in the future, but if you care about Universal MacOS binaries which staple x64 and arm64 together, your build will end up looking like this anyway (not yet implemented).

* added rosetta-style "you don't have an arm64 build but you do have an x64 one, so we'll use that" fallback to the powershell installer, as arm64 windows supports automatic emulation (and folks seems to recommend relying on that over bothering with arm64 windows builds at this point?)


# Version 0.0.3 (2023-02-27)

A major overhaul has been done to the design to rationalize some improperly defined features/behaviours. When you update to this version **we recommend following these MIGRATION INSTRUCTIONS**:

1. (optional) delete `[profile.dist]` from your Cargo.toml
2. run `cargo dist init --ci=github`
3. run `cargo dist generate-ci`

Performing Step 1 will result in Step 2 getting you our new recommended default profile; linux users were having issues with the debuginfo stuff.

Step 2 will introduce default configuration to your Cargo.toml that's necessary for the new design to work reliably. You can add `--installer=shell` and `--installer=powershell` here if you want those to be setup automatically.

Step 3 will completely blow away your release.yml CI with the new design. The overall approach is the same but everything is more consistent and coherent.

The new design is described in detail in [the new cargo-dist book](https://axodotdev.github.io/cargo-dist/book/)!


## Configuration

You can now include persistent configuration for cargo-dist in `[workspace.metadata.dist]` and `[package.metadata.dist]`. [See the book for details](https://axodotdev.github.io/cargo-dist/book/config.html#metadatadist).

## Artifact Modes

Previously cargo-dist had some vague notions of what it was supposed to do when you invoked it, because there were platform-specific artifacts like executable-zips but also more platform-agnostic ones like installer scripts. This result in flags like `--no-builds` with messy semantics and hacks to filter out artifacts we "don't want right now" in the CI scripts (`--no-builds` was is removed in this release, it was busted).

Now cargo-dist can produce well-defined subsets of all tne possible artifacts with the `--artifacts` flag:

> --artifacts = "local" | "global" | "all" | "host"
>
> Artifacts can be broken up into two major classes:
>
> * local: made for each target system (executable-zips, symbols, MSIs...)
> * global: made once (curl-sh installers, npm package, metadata...)
>
> ("all" selects both of these at once)
>
> Having this distinction lets us run cargo-dist independently on multiple machines without collisions between the outputs by spinning up machines that run something like:
>
> * linux-runner1 (get full manifest): cargo-dist manifest --artifacts=all --output-format=json
> * linux-runner2 (get global artifacts): cargo-dist --artifacts=global
> * linux-runner3 (get linux artifacts): cargo-dist --artifacts=local --target=x86_64-unknown-linux-gnu
> * windows-runner (get windows artifacts): cargo-dist --artifacts=local --target=x86_64-pc-windows-msvc
>
> If left unspecified, we will pick a fuzzier "host" mode that builds "as much as possible" for the local system. This mode is appropriate for local testing/debugging/demoing. If no --target flags are passed on the CLI then "host" mode will try to intelligently guess which targets to build for, which may include building targets that aren't defined in your metadata.dist config (since that config may exclude the current machine!).
>
> The specifics of "host" mode are intentionally unspecified to enable us to provider better out-of-the-box UX for local usage. In CI environments you should always specify one of the other three options!

Note that the introduction of persistent Cargo.toml configuration is crucial to this semantic redesign, as it allows each invocation to be aware of the "full" set of artifacts across all platforms, and then filter down to it.

If you pass `--installer`, `--ci`, or `--target` this will replace the Cargo.toml value for all packages for that invocation. This is most useful for `--target` in conjunction with `--artifacts=local` as it lets us precisely select which platform-specific artifacts to build on the current machine (all 3 of these flags can be passed repeatedly).

**WARNING!** If you specify --artifacts and --target, the selected targets can only be a *subset* of the ones defined in your Cargo.toml. This ensures `cargo dist --artifacts=global` has behaviour consistent with `cargo dist --artifacts=local --target=...`, as global artifacts need to be aware of all targets at once. "host" mode bypasses this restriction so that runs of cargo dist on developer machines can do *something* useful even if the Cargo.toml doesn't know about the host platform.


## Announcement/Release Selection

There is also now a `--tag` flag for specifying the git tag to use for announcing a new release. This tag must have a specific format detailed below. The tag serves two purposes:

* It specifies the subset of the workspace that we want to Announce/Release
* When using CI, it becomes the unique ID for a Github Release, which is necessary for everything to correctly compute download URLs

`cargo dist build` and `cargo dist manifest` now both require that you either specify a --tag that "makes sense", or that your workspace is simple enough for a tag to be computed for you. In CI, each git tag you push will create an independent run of cargo-dist's CI to make a Github Release for that tag, and each invocation of cargo-dist will have that tag passed to it, ensuring they all agree on the above details.

There are two kinds of tag formats that are accepted:

* Unified Announcement: `v{VERSION}` selects all packages with the given version (v1.0.0, v0.1.0-prerelease, etc.)
* Singular Announcement: `{PACKAGE-NAME}-v{VERSION}` selects only the given package (error if the version doesn't match)

Note that other criteria may prevent a package from being selected: it has no binaries, it has dist=false, it has publish=false, etc. If you do not specify a --tag, cargo-dist will check if all still-selectable packages share a version, and if they do it will make a Unified Announcement for them (erroring otherwise).

These two modes support the following workflow:

* Releasing a workspace with only one binary-having package (either mode works but Unified is Cleaner)
* Releasing a workspace where all binary-having packages are versioned in lockstep (Unified)
* Releasing an individual package in a workspace with its own independent versioning (Singular)
* Releasing several packages in a workspace at once, but all independently (Push multiple Singular tags at once)

Basically the one thing we can't deal with is you saying "I would like a single coherent Announcement (Github Release) for packageA 0.1.0 and packageB 0.2.0", because nothing really ties them together. If you disagree, please let us know how you think it can/should work!

Although you *could* use extremely careful versioning in conjunction with Unified Announcements to release a weird subset of the packages in your workspace, you really *shouldn't* because the Github Releases will be incoherent (v0.1.0 has these random packages, v0.2.0 has these other random packages... huh?), and you're liable to create painful tag collisions.

**WARNING!** cargo-release *largely* already generates tags that express these exact semantics, except for one annoying corner case (that I've found so far): if you have a non-virtual workspace (the root Cargo.toml is an actual package with child packages), it will always try to tag releases of the root package with a Unified Tag, even when using `--workspace`. This will not play well with cargo-dist. Initial testing suggests virtual workspaces behave much better.

## Release Notes

Release notes are now temporarily simplified for reliability:

* For the purposes of a top level Announcement (Github Release), notes are now no longer associated with the individual apps being published, meaning there's only one set of notes generated.

* If you have a RELEASES* or CHANGELOG* file in the root of your workspace, we will assume these are the release notes for any Unified Announcement (see the previous section) and try to include the relevant section at the top of the Github Release. This is done with the [parse_changelog](https://github.com/taiki-e/parse-changelog) library. If parsing/lookup fails we continue on silently.

* If the above process succeeds, the heading of the section we found will become the new title of the Github Release. For example, if we find `1.2.0` matches `# Version 1.2.0 (2023-01-25)`, the title of the Github Release will become "Version 1.2.0 (2023-01-25)".

* If you are publishing `1.2.0-prerelease` and we don't find that in your RELEASES/CHANGELOG file, we will now also look for bare `1.2.0` (stripping the prerelease/build portions), on the assumption that these are the WIP release notes for the version you're prereleasing. This lets you iterate on a version without having to churn headings every time you want to cut a prerelease (we recommend including a parenthetical indicating the version is not yet released).

* If the above explained deferring happens, we will modify the release note's title to include the prerelease suffix. This ensures they are easily identifiable as prereleases on GitHub's releases page.

* We will no longer attempt to include your release notes for Singular Announcements (see the previous section). They will only get auto-generated installers/downloads sections. This is obviously suboptimal, and will be fixed, we just need to do design work on the proper way to handle those cases. (Please tell me in [issue #139](https://github.com/axodotdev/cargo-dist/issues/139)!)



## Fixes

* The generated Github CI script is now Valid YAML. The script ran fine, but it was rightfully angering YAML linters!
* The generated Github CI now has a single unified "build artifacts" task with a shared matrix for global artifacts (shell script installers) and local artifacts (executable zips) (previously the "global" artifacts had their own weird task)
* We now properly detect if `cargo dist init` has been run by checking for the presence of `[profile.dist]` in your root Cargo.toml
* There are now top level fields in dist-manifest.json for release notes for the "full announcement" of all Releases. These fields should be preferred when generating e.g. the body of a Github Release, as they will behave more correctly when there are multiple Releases.
* **If multiple binaries are defined by one Cargo package, they will now be considered part of the same "app" and bundled together in executable-zips.** Previously we would give each binary its own "app". The new behaviour matches how 'cargo install' works and is compatible with the expectations of 'cargo binstall'. You kinda have to go out of your way to shove multiple binaries under one package, so we figure if you do, we should respect it!
* If a package specifies publish=false in its Cargo.toml, we will take this as a hint to not dist it. You can override this behaviour by setting `[package.metadata.dist] dist = true` in that Cargo.toml.
* Installer artifacts are now properly prefixed with the id of the Release they're part of, preventing conflicts when doing multiple Releases at once (installer.sh => my-app-v1.0.0-installer.sh).
* Installers now properly handle packages that define multiple binaries (installing all of them, just like cargo-install)
* Installers now properly know the Github Release they are going to point to (previously they would guess based on the version of the package which was broken in complicated workflows)
* --installer=github-shell and --installer=github-powershell have had the "github-" prefix removed. They now generically use the concept of an "artifact download url" which will be configurable in the future (for now it only gets populated if ci=github is set and your workspace has a coherent definition for "repository" in its Cargo.tomls).
* We will error out if you try to run `cargo dist generate-ci` and the `cargo-dist-version` in your config doesn't match the version you're currently running
* If you're running arm64 macos ("apple silicon"), shell installers will now try to fallback to installing x64 macos binaries if no arm ones are available (so Rosetta can deal with it)


# Version 0.0.2 (2023-01-31)

cargo-dist:

* Added proper detection of README/LICENSE/RELEASES/CHANGELOG files, which are now copied to the root of executable-zips.
    * We will defer to Cargo fields like "readme" and "license-file" if present
    * Otherwise we will search the root directory of the package and the root directory of the workspace (preferring results from the former)
* Release note handling:
    * --ci=github will manually set the title and body of the Github Release
    * The body is a generated listing of installers/downloads
    * If your RELEASES/CHANGELOG parses with parse_changelog library we'll append the current release's notes to the body, and use the heading for the title
    * If we don't parse your RELEASES/CHANGELOG we will default to a title of "v{VERSION}"

cargo-dist-schema:

* Changed PathBufs to Strings since the paths may be from a different OS and Rust Paths are generally platform-specific. Seemed like a ticking timebomb for some weird corner case.
* Added "changelog" as a valid AssetKind
* Added "changelog_title" and "changelog_body" to Release
    * These are used to populate a Github Release
* Added "description" to Artifact
    * Currently just used to describe some installers
* Made Artifact::name Optional to futureproof
    * If None this indicates the artifact is purely informative and no file exists (i.e. "you can install with cargo-binstall")

# Version 0.0.1 (2023-01-23)

This is the first alpha release of cargo-dist with some minimal functionality!

There are also a couple 0.0.1 prereleases that came before this one that exist to define a sort of "bootstrapping history" for the first "real" release's binary builds because I find it vaguely satisfying and you can't stop me.
