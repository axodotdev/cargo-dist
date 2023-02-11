# Next Version (unreleased)

A major overhaul has been done to the design to rationalize some improperly defined features/behaviours, when you update to this version you will need to rerun `cargo dist init` (and need to generate-ci).

## Configuration (Cargo.toml)

You can now persistently configure cargo-dist with `[workspace.metadata.dist]` in your root Cargo.toml, with the ability to override some settings per-package with `[package.metadata.dist]`.

The following settings can *only* be set in `[workspace]`:

* cargo-dist-version: (Cargo SemVer format) specifies the desired version of cargo-dist for building the project. Currently only used when generating CI scripts. When you run `cargo dist init` the version you're using will be set here.
* rust-toolchain-version: (rustup toolchain format) species the desired version of rust/cargo for building the project. Currently only used when generating CI scripts.When you run `cargo dist init` the version you're using will be set here(?)
* ci: a list of CI backends to support (currently only `["github"]` will work). This is used by `cargo dist generate-ci` so you no longer need to pass the flag every time. Possibly it will also be used for detecting other features like github-style release note generation should be enabled?

The following settings can be set on either `[workspace]` or `[package]`, with the latter overriding the former:

* dist: (bool, defaults true) whether this package's binaries should be visible to cargo-dist for the purposes of Releases.
* installers: (list of installer kinds) the default set of installers to generate for releases
* targets: (list of rust-style target-triples) the default set of targets to use for releases (sort of, see the section on CLI configuration)
* include: (list of paths relative to that Cargo.toml's dir) extra files to include in executable-zips (does not currently support paths or wildcards)
* auto-includes: (bool, defaults true) whether dist should add README/LICENSE/etc files to your executable-zips when it finds them.

## Configuration (Artifact CLI flags)

Previously cargo-dist had some vague notions of what it was supposed to do when you invoked it, because there were platform-specific artifacts like executable-zips but also more platform-agnostic ones like installer scripts. This result in flags like `--no-builds` with messy semantics and hacks to filter out artifacts we "don't want right now" in the CI scripts (`--no-builds` was is removed in this release, it was busted).

Now cargo-dist can produce well-defined subsets of the all possible artifacts with the `--artifacts` flag:

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
> If let unspecified, we will pick a fuzzier "host" mode that builds "as much as possible" for the local system. This mode is appropriate for local testing/debugging/demoing. If no --target flags are passed on the CLI then "host" mode will try to intelligently guess which targets to build for, which may include building targets that aren't defined in your metadata.dist config (since that config may exclude the current machine!).
>
> The specifics of "host" mode are intentionally unspecified to enable us to provider better out-of-the-box UX for local usage. In CI environments you should always specify one of the other tree options!

Note that the introduction of persistent Cargo.toml configuration is crucial to this semantic redesign, as it allows each invocation to be aware of the "full" set of artifacts across all platforms, and then filter down to it.

If you pass `--installer`, `--ci`, or `--target` this will replace the Cargo.toml value for all packages for that invocation. This is most useful for `--target` in conjuction with `--artifacts=local` as it lets us precisely select which platform-specific artifacts to build on the current machine (all 3 of these flags can be passed repeatedly).


## Configuration (Announcement/Release Selection)

(NOT YET IMPLEMENTED!)

There is also now a `--tag` flag for specifying the git tag to use for announcing a new release. This tag will be used for things like the download URLs when fetching from github-releases. This is necessary to specify the correct behaviour in situations where there are multiple packages with binaries in the workspace, potentially with different versions.

In the future this flag will also be used to specify a subset of the workspace to release. For instance `--tag=blah-v1.0.0` could be used to specify "only create an announcement/release for the 'blah' package's binaries".


## Fixes

* installer artifacts are now properly prefixed with the id of the Release they're part of, preventing conflicts when doing multiple Releases at once (installer.sh => my-app-v1.0.0-installer.sh).
* the generated github CI script is now Valid YAML. The script ran fine, but it was rightfully angering YAML linters!
* (NOT YET IMPLEMENTED) we now properly detect if `cargo dist init` has been run by checking for the presence of `[profile.dist]` in your root Cargo.toml
* (NOT YET IMPLEMENTED) there is now top level fields in dist-manifest.json for release notes for the "full announcement" of all Releases. These fields should be preferred when generating e.g. the body of a Github Release, as they will behave more correctly when there are multiple Releases.




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