# Unreleased

Nothing Yet!


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

At the request of end users, we've added a small legal notice at the top of the generated github release.yml file to indicate that the contents of the file are permissibly licensed. This hopefully makes it easier for package distributors and employees at large companies w/legal review to confidentally use cargo-dist!

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
