# Concepts

<!-- toc -->

Here's the section where I use a bunch of Capitalized Words to indicate they are a Special Concept in dist as I try to explain how it works. These are the "advanced" docs of dist; see [the guide][guide] for the "beginner" docs.

An invocation of dist has 4 major inputs:

* The structure of your project's [Cargo Workspace][workspace] (via [cargo-metadata][])
* The config in your Cargo.toml `[workspace.metadata.dist]` (and `[package.metadata.dist]`)
* The "announcement tag" (e.g. `--tag=v1.0.0`)
* The "artifact mode" (e.g. `--artifacts=all`)

The first two define the full "Universe" of your project -- the platforms/binaries/[installers][] that dist wants to build. The second two tell dist what subset of the Universe to actually bother with.

It's important to the structure of dist that every invocation is aware of the full Universe and how it's being subsetted, because for instance if you want a shell script installer that does platform detection and fetches binaries, it needs to know about all the binaries/platforms it has to select from, even if this *particular* run of dist won't build them all!

First let's look at how dist computes the Universe.


# Defining Your Apps

Each Cargo package in your workspace that has [binary targets][] is considered an App by dist. dist exists to build Apps, so making sure you and it agree on is important! (We prefer "App" over "package" because we want the freedom to one day decouple the two concepts -- for now they are strictly equivalent.)

In addition to your executables dist can publish your [cdylibs][], including WASM bundles. Note that, for Rust specifically, there can be [messy issues around Cargo clobbering itself when you define two many things under one package][cargo-conflicts].

Most invocations of dist will start by printing out a brief summary of the Apps that dist has found:

![screenshot of the debug log, described below][workspace-log]

The summary includes a list of every package in your workspace. If that package defines binaries, they will be listed underneath the package. If the package's listing is greyed out, that means dist has decided it's either Not An App or that it's not part of the current Announcement ([see the Announcement section][announcements-section]), along with a parenthetical reason for its rejection, such as: "no binaries", "publish = false", "dist = false", or "didn't match tag".

In the above example the available Apps are "evil-workspace", "many-bin", and "third-bin". "many-bin" defines two binaries, while the other two Apps only define one.

To match cargo-install's behaviour, if a package defines multiple binaries then they will be considered part of the same App and zips/[installers][] for it will contain/install all of them. We figure if you went out of your way to have multiple binaries under one package (as opposed to separate packages for each), you did that for a reason! If you don't want that, make separate packages. There is currently no way to group multiple packages into a single App, although there probably will be one day.

If you don't want a package-with-binaries to be considered an App that dist should care about, you can use Cargo's own builtin [publish = false][publish-false]. You can also use `dist = false` or `dist = true` in [dist's own config][config-dist], which when defined will take priority over `publish`.



# Defining Your Artifacts

Ok so you've defined your App, but what should we actually build for it? Let's look at what `dist init --ci=github --installer=shell --installer=powershell --yes` dumps into your root Cargo.toml:

```toml
# Config for 'dist'
[workspace.metadata.dist]
# The preferred dist version to use in CI (Cargo.toml SemVer syntax)
cargo-dist-version = "0.0.3"
# CI backends to support
ci = ["github"]
# The installers to generate for each app
installers = ["shell", "powershell"]
# Target platforms to build apps for (Rust target-triple syntax)
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]

# The profile that 'dist' will build with
[profile.dist]
inherits = "release"
lto = "thin"
```

The parts we're really interested in here are "installers", "targets", and `[profile.dist]`.

First the easy part: `profile.dist` is the profile dist will build everything with. We define a separate profile from `release` so that it can be tuned more aggressively for builds that are longer or more resource-intensive without making it tedious to develop locally.

The other 3 fields are defining the various Artifacts that should be produced for each App in the workspace (because this is `[workspace.metadata]` and not `[package.metadata]`).

For each entry in `targets` you will get a build of your App for [that platform][rust-platform] in the form of an [archive][].

For each entry in `installers` you get that kind of [installer][installers] for your App. There are two classes of installer: "global" and "local". This will be explained further in [the section on artifact modes][artifact-modes-section], but the tl;dr is that "global" installers are one-per-App while "local" installers are one-per-platform-per-app, similar to a [Github CI Matrix](https://docs.github.com/en/actions/using-jobs/using-a-matrix-for-your-jobs).

"shell" and "powershell" are both global installers. There aren't currently any implemented local installers, but those would be things like a windows "msi" or macos "dmg", where you ostensibly want individual installers for each architecture.




# Announcements (Selecting Apps)

dist's self-generated CI is triggered by pushing git tags with specific formats like "v1.0.0", "my-app-v1.0.0" or "my-app/v1.0.0". Each tag will trigger its own independent run of that CI workflow. That tag defines the subset of the workspace (what packages) we want to produce a single unified Announcement for (i.e. a single Github Release). Every invocation of dist in that CI run will be passed that git tag with the `--tag` flag to ensure consensus on what to Announce (and therefore build and upload).

1 Git Tag = 1 dist Announcement = 1 Github Release

Even when not running in CI, this concept of a coherent Announcement Tag is important enough that we will always try to guess one even if none is provided. The "build", "manifest", and "plan" commands will refuse to run if a coherent Announcement Tag can't be determined to help you catch problems before you start pushing to CI. If this happens you will get a printout telling you some options:

![the error printout, described below][announce-error]

Here we have the same workspace we saw in the ["defining your apps" section][defining-your-apps-section], but we get a complaint from `dist manifest`:

> There are too many unrelated apps in your workspace to coherently Announce!
>
> Please either specify --tag, or give them all the same version

**This introduces the one big rule for Announcements: all the Apps being Announced together have to agree on their Version.** We need something to tie the announcement together and "3 random Apps with different Versions" has nothing to use! You should really just have 3 separate Announcements for those Apps. If you disagree, please let us know!

The error goes on to recommend the two formats for the Announcement Tag:

* Unified Announcement: VERSION selects all packages with the given version (v1.0.0, 0.1.0-prerelease.1, releases/1.2.3, ...)
* Singular Announcement: PACKAGE-VERSION or PACKAGE/VERSION selects only the given package (my-app-v1.0.0, my-app/1.0.0, release/my-app/v1.2.3-alpha, ...)

These two modes support the following workflows:

* Releasing a workspace with only one App (either mode works but Unified is Cleaner)
* Releasing a workspace where all Apps are versioned in lockstep (Unified)
* Releasing an individual App in a workspace with its own independent versioning (Singular)
* Releasing several Apps in a workspace at once, but all independently (Push multiple Singular tags at once)

In this case the error notes two valid Unified Announcements:

> `--tag=v0.5.0` will Announce: evil-workspace, third-bin
> `--tag=v0.7.6` will Announce: many-bin

This tells us that evil-workspace and third-bin actually already agree on their Version. If we *do* want a Unified Announcement, we probably want to bring many-bin into agreement, or mark it as publish=false or dist=false.

Although you *could* use extremely careful versioning in conjunction with Unified Announcements to release a weird subset of the packages in your workspace, you really *shouldn't* because the Github Releases will be incoherent (v0.1.0 has these random packages, v0.2.0 has these other random packages... huh?), and you're liable to create painful tag collisions.

Normally dist will error out if the Announcement Tag selects no Apps, because it exists to build and distribute Apps and you just asked it to do nothing (which is probably a mistake). This would however create annoying CI errors if you just wanted to tag Individual Releases for your libraries. To make this more pleasant, **dist will produce a very minimal build-less Announcement (and therefore Github Release) if you explicitly request a Singular Announcement that matches a library-only package**. This feature is kind of half-baked, please let us know what you want to happen in this situation!





# Artifact Modes (Selecting Artifacts)

Now that we have a coherent Announcement and therefore have selected what apps we want to Release, we need to select what artifacts we want to build (or get a manifest for). Enumerating the exact artifacts for each invocation of dist would be tedious and error-prone, so we provide the `--artifacts=...` flag to specify the *Artifact Mode*, which is a certain subset of the Universe of all Artifacts:

* "local": artifacts that are per-target platform ([archives][archive], symbols, msi installers...)
* "global": artifacts that are one-per-app (shell installer, npm package...)
* "all": both global and local (so the whole Universe)
* "host": the default mode that kind of breaks the rules to let you test things out locally

Let's ignore "host" mode for a bit and focus on the other three. Each one of these is intended to be used for specific tasks.



## All Artifacts Mode

The "all" Artifact Mode is largely intended for the `manifest` command, to get a listing of everything that would be produced if you were to push the given tag to CI. Here we check what v0.5.0 would produce for our favourite example workspace:

```sh
dist manifest --tag=v0.5.0 --artifacts=all --no-local-paths
```

![A listing of the various Artifacts that should be produced][human-manifest-example]


If we add `--output-format=json` we will get the `dist-manifest.json` that CI uploads to your Github Release:

```sh
dist manifest --tag=v0.5.0 --artifacts=all --no-local-paths --output-format=json
```

This is the only way that CI uses the flag, but you could also use "all" with `build` (the default dist command) if you want to get all the artifacts built at once, although you should probably filter the `--target`s as discussed in the section on "local".

`dist manifest --artifacts=all --no-local-paths` is so useful/common that we provide an alias for it: `dist plan`. The above can be simplified to:

```sh
dist plan --tag=v0.5.0
```

```sh
dist plan --tag=v0.5.0 -ojson
```


## Global Artifacts Mode

The "global" Artifact Mode allows you to unambiguously create a task that will build all the Artifacts for your Apps that *aren't* platform-specific and therefore only need to be made once per App:

```sh
dist build --tag=v0.5.0 --artifacts=global --no-local-paths
```

![A global build producing only shell and powershell installers][global-build-example]

Here we see that it only results in the "shell" and "powershell" installers getting built. The code to generate these should be totally cross-platform, so any runner is suitable for the task. The CI creates one "global" task that uses linux because that's the fast/cheap one.


## Local Artifacts Mode

The "local" Artifact Mode allows you to unambiguously create a task that will build all the Artifacts for your Apps that *are* platform-specific and therefore should have a copy made for every target platform.

If you just use this flag bare, dist *will* respect the request and try to build for all platforms at once... and this will probably fail, because cross-compilation is hard. Each "local" run should generally use `--target` to filter down the set of all supported targets to the ones you can confidently build on the current machine (`rustc -vV` will tell you the "host" target platform if you're not sure).

In my case it's "x86_64-pc-windows-msvc", so let's try that:

```sh
dist build --tag=v0.5.0 --artifacts=local --target=x86_64-pc-windows-msvc --no-local-paths
```

![A local build producing only archives for the current platform][local-build-example]

Note that you can pass `--target` multiple times to select more than one. Note also that `--target` is not allowed to select targets that aren't specified by the config your Cargo.toml. This ensures that global installers are consistently aware of all the platform-specific artifacts they can fetch. ("host" mode breaks this rule.) ((Also in theory `--installer` should work the same for selecting specific installers but it's not well tested because there isn't any reason to ever use that outside of `dist init`.))

CI will spin up one "local" task for each of the major desktop platforms, grouping the targets that are easy to build on those platforms together. In the future we might want to spawn one task per target, or at least make that an option you can pick. That said, some Artifacts like macOS universal binaries may find it useful to have multiple targets built on the same machine!



## Host Artifacts Mode

Host mode is the default "do something useful on my machine" mode. It's intended for testing and demoing dist on your project, and is never used in CI due to its intentionally fuzzy semantics.

It's currently roughly equivalent to `--artifacts=all --target=HOST_TARGET`, but HOST_TARGET is allowed to fall outside the set of targets defined in your Cargo.toml, because it's not terribly useful to tell someone trying out dist on ARM64 Linux that their platform isn't defined in the config.

In principle when we have better support for cross-compilation we might also try to build "nice" crosses like "intel apple => arm64 apple". Do not rely on the behaviour of this mode, always use one of the 3 other modes in your infra/scripts!

If you *do* pass `--target` in host mode then we won't do fuzzy target selection and will just build the targets you ask for like normal.




# Putting It All Together

Ok so here's what goes through dist's brains when you run it:

1. Read in the workspace/config/cli-flags
2. Determine the Announcement Tag (select the Apps) ("v1.0.0")
3. Determine what Targets we're building for
3. Call the specific Version of each App a "Release" ("my-app-v1.0.0")
4. For each Release-Target pair, create a "ReleaseVariant" ("my-app-v1.0.0-x86_64-apple-darwin")
5. Add archive Artifacts to each Release (broadcasted to each Variant, filtered by Artifact Mode)
6. Add all the enabled Installers to each Release (local ones broadcasted to each Variant, filtered by Artifact Mode)
7. Compute the Build Steps necessary to produce each Artifact ("run cargo, copy this file, ...")
8. Generate top-level Announcement info like the body for a Github Release
9. run the Build Steps (ignored by `manifest`/`plan`)
10. print a manifest of the computed Announcement/Releases/Artifacts

CI will parse the resulting (`--output-format=json`) manifest of each `build` invocation to know what artifacts were produced and need to be uploaded to the Github Release.

CI will just invoke dist in the following sequence:

1. create-release: `dist manifest --artifacts=all --output-format=json --no-local-paths`
2. upload-artifacts (local): `dist build --artifacts=local --target=... --output-format=json`
3. upload-artifacts (global): `dist build --artifacts=global --output-format=json`
4. publish-release: none, just marks the Github Release as a non-draft

(All the upload-artifacts tasks are in parallel, and there are multiple "local" tasks to cover the target platforms.)


[config-dist]: ../reference/config.md#dist

[guide]: ../workspaces/index.md
[installers]: ../installers/index.md
[archive]: ../artifacts/archives.md
[announcements-section]: #announcements-selecting-apps
[artifact-modes-section]: #artifact-modes-selecting-artifacts
[defining-your-apps-section]: #defining-your-apps

[workspace-log]: ../img/workspace-log.png
[announce-error]: ../img/announcement-error.png
[human-manifest-example]: ../img/human-manifest-all.png
[global-build-example]: ../img/global-build.png
[local-build-example]: ../img/local-build.png

[binary targets]: https://doc.rust-lang.org/cargo/reference/cargo-targets.html#binaries
[publish-false]: https://doc.rust-lang.org/cargo/reference/manifest.html#the-publish-field
[cdylibs]: https://doc.rust-lang.org/cargo/reference/cargo-targets.html#library
[cargo-conflicts]: https://github.com/rust-lang/cargo/issues/6313
[rust-platform]: https://doc.rust-lang.org/nightly/rustc/platform-support.html
[cargo-metadata]: https://doc.rust-lang.org/cargo/commands/cargo-metadata.html
[workspace]: https://doc.rust-lang.org/cargo/reference/workspaces.html
