# Troubleshooting

Having an issue with dist? Here's some of the common issues and solutions!

<!-- toc -->


## What We Would Usually Do

Regardless of the issue, these are the "default" troubleshooting steps that are good to keep in mind:

1. diagnose: [run `dist plan`](./quickstart/rust.md#check-what-ci-will-build)
2. update and repair: [run `dist init` again](./updating.md)
3. test your process: [try `pr-run-mode = "upload"`](./ci/customizing.md#build-and-upload-artifacts-on-every-pull-request)

These are also great steps to follow proactively, if you're updating your dist config.



## Nothing To Release / Missing Packages / Too Many Packages

dist tries to support as many release workflows as possible, and that means it needs you to tell it what you're interested in releasing. There are several ways to opt things in and out of being released; the most important are:

* [git tag formats](./workspaces/workspace-guide.md#announcement-tags)
    * git tags select which packages you're interested in doing a release for
    * do all the packages you want to publish have the same version as your git tag?
    * are you prefixing the tag with something that looks like a package name?
* [`[package].publish`](./reference/config.md#publish)
    * tells cargo whether the package should be published to crates.io
    * dist assumes you don't want to release `publish = false` packages (since they're probably for testing)
* [`[package.metadata.dist].dist`](./reference/config.md#dist)
    * overrides `publish` for dist releases, either to force a package on or off



## Recovering Failed CI Jobs

Sometimes CI fails, and that's ok! The steps to follow depend on what went wrong. There's little that can't be recovered, you've got this.


### Failed CI: Spurious

If you believe the failure was spurious (Github CI flaking out, some networked service being temporarily down, a SECRET not being set...), then good news: it's totally safe to "retry failed jobs" in the Github CI interface! We should pick up your release process from where it left off.

We *DO NOT* recommend "retry all jobs". Either it's redundant or it can cause problems with trying to repeat side-effects like publishing a package.


### Failed CI: Busted Builds

If you believe something was busted in your release process, and the commit you tried to release from isn't suitable, that's generally ok! Usually this will occur because some part of your build is broken, perhaps only when releasing with dist, or only on a particular platform.

If this is the case then presumably your release process errored out before the the "host" step where we actually uploaded anything, so good news: no side effects need to be rolled back!

Well, one side-effect needs to be dealt with, but it's the one you did to kick off the release: [delete the git tag from github and your local machine](https://stackoverflow.com/questions/5480258/how-can-i-delete-a-remote-tag), get your build sorted out, and then tag the new release commit.

"Get your build sorted out" is of course, eliding a lot of details. If the issue appears to be exclusive to dist CI, we recommend opening a PR against your project with [`pr-run-mode = "upload"`](./ci/customizing.md#build-and-upload-artifacts-on-every-pull-request) temporarily enabled. This will run all of the build steps for your release process without you needing to push a git tag, so you're free to experiment and rapidly iterate without *any* risk of side-effects.



## Oops, My Changelog!

Changelogs are arguably the most important and challenging part of a release process. Although dist currently has no way to Do Changelogs For You, it does have several features for Using Your Changelogs:

* [Having dist parse your RELEASES or CHANGELOG file](./workspaces/simple-guide.md#release-notes)
* [Telling dist you're bringing your own changelogs with tools like release-drafter](./ci/customizing.md#bring-your-own-release)

The latter will just have dist not clobber the changelogs you upload to Github Releases, and is therefore easy to fix: just hand-edit your Github Release more.

The former is more challenging to fix, and is a place we're trying to improve. dist will natively understand your changelogs and bake them into a few different things:

* The changelog will be sent to your release hosting providers (github and/or axo) as part of the announcement
* The changelog will be stored in the dist-manifest.json
    * Which will in turn [get used by oranda, if you're using that](https://github.com/axodotdev/oranda)

This is to say, trying to hand-edit your way out of this situation requires you to find and fix a lot of data if you Really Want A Perfect Changelog. In some cases we've found it simpler to just redo the whole release process (either by deleting a Github Release or bumping the version number).



## Repository URLs / Source Hosts / Hosting Providers

[Many features](./reference/artifact-url.md) of dist depend on the ability to know where your project is hosted, and where the build results will get uploaded. The most common issue users encounter here is not having a defined [Source Host](./reference/artifact-url.md#source-hosts), which basically just means you need to audit the `[package].repository` values you set in your Cargo.tomls and make sure they consistently point to your GitHub repo. [See the Source Host docs for details](./reference/artifact-url.md#source-hosts).


## The Protip Zone

Sometimes users run into issues that are quickly resolved by "I had no idea Rust let you do that", so here's some quick pointers to useful Rust/Cargo/Rustup features (and a promise that we handle them properly):

* [rust-toolchain.toml exists to tell rustup your project should be built with a specific version of Rust](https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file)
* [the `[[bin]]` section of a Cargo.toml lets a single package have multiple binaries, or to rename the only binary](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#binaries)
* [`publish = false` lets you tell Cargo not to publish a package](https://doc.rust-lang.org/cargo/reference/manifest.html#the-publish-field)
* [Cargo packages can inherit keys from the workspace package to keep things in sync](https://doc.rust-lang.org/cargo/reference/workspaces.html#the-package-table)
