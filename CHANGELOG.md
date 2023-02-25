# Version 0.0.3 (under development)

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

**WARING!** If you specify --artifacts and --target, the selected targets can only be a *subset* of the ones defined in your Cargo.toml. This ensures `cargo dist --artifacts=global` has behaviour consistent with `cargo dist --artifacts=local --target=...`, as global artifacts need to be aware of all targets at once. "host" mode bypasses this restriction so that runs of cargo dist on developer machines can do *something* useful even if the Cargo.toml doesn't know about the host platform.


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

Although you *could* use extremely careful versioning in conjuction with Unified Announcements to release a weird subset of the packages in your workspace, you really *shouldn't* because the Github Releases will be incoherent (v0.1.0 has these random packages, v0.2.0 has these other random packages... huh?), and you're liable to create painful tag collisions.

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
* If a package specifes publish=false in its Cargo.toml, we will take this as a hint to not dist it. You can override this behaviour by setting `[package.metadata.dist] dist = true` in that Cargo.toml.
* Installer artifacts are now properly prefixed with the id of the Release they're part of, preventing conflicts when doing multiple Releases at once (installer.sh => my-app-v1.0.0-installer.sh).
* Installers now properly handle packages that define multiple binaries (installing all of them, just like cargo-install)
* Installers now properly know the Github Release they are going to point to (previously they would guess based on the version of the package which was broken in complicated workflows)
* --installer=github-shell and --installer=github-powershell have had the "github-" prefix removed. They now generically use the concept of an "artifact download url" which will be configurable in the future (for now it only gets populated if ci=github is set and your workspace has a coherent definition for "repository" in its Cargo.tomls).
* We will error out if you try to run `cargo dist generate-ci` and the `cargo-dist-version` in your config doesn't match the version you're currently running



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
