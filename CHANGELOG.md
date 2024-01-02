# Unreleased

Nothing Yet!


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
