<div class="oranda-hide">

# `dist` (formerly known as `cargo-dist`)

</div>

[![crates.io](https://img.shields.io/crates/v/cargo-dist.svg)](https://crates.io/crates/cargo-dist)
[![docs](https://docs.rs/cargo-dist/badge.svg)](https://docs.rs/cargo-dist)
[![Rust CI](https://github.com/axodotdev/cargo-dist/workflows/Rust%20CI/badge.svg?branch=main)](https://github.com/axodotdev/cargo-dist/actions/workflows/ci.yml)

*dist distributes your binaries*

The TL;DR is that with dist set up, just doing this:

```sh
git commit -am "release: 0.2.0"
git tag "v0.2.0"
git push
git push --tags
```

Will make [this Github Release](https://github.com/axodotdev/axolotlsay/releases/tag/v0.2.0):

Or if you're using [oranda](https://axodotdev.github.io/oranda/), you'll get [this website](https://axodotdev.github.io/axolotlsay/).


## Plan, Build, Host, Publish, Announce

Cutting releases of your apps and distributing binaries for them has a lot of steps, and cargo-dist is quickly growing to try to cover them all!

To accomplish this, dist functionality can be broken up into two parts:

* building (**planning** the release; **building** binaries and installers)
* distributing (**hosting** artifacts; **publishing** packages; **announcing** releases)

The build functionality can be used on its own if you just want some tarballs and installers, but everything really comes together when you use the distribution functionality too.


## Building

As a build tool, dist can do the following:

* Pick good build flags for "shippable binaries"
* Make [tarballs][] and [installers][] for the resulting binaries
* Generate [machine-readable manifests][manifest] so other tools can understand the results

That's a short list because "we make [installers][]" is doing a lot of heavy lifting. Each installer could be (and sometimes is!) an entire standalone tool with its own documentation and ecosystem.


## Distributing

As a distribution tool, dist gets to flex its biggest superpower: **it generates [its own CI scripts][ci-providers]**. For instance, enabling [GitHub CI][ci-providers] with `dist init` will generate release.yml, which implements the full pipeline of plan, build, host, publish, announce:

* Plan
    * Waits for you to push a git tag for a new version (v1.0.0, my-app-v1.0.0, my-app/1.0.0, ...)
    * Selects what apps in your workspace to announce new releases for based on that tag
    * Generates [a machine-readable manifest][manifest] with changelogs and build plans
* Build
    * Spins up machines for each platform you support
    * Builds your [binaries and tarballs][tarballs]
    * Builds [installers][] for your binaries
* Publish:
    * Uploads to package managers
* Host + Announce:
    * Creates (or edits) a GitHub Release
    * Uploads build artifacts to the Release
    * Adds relevant release notes from your RELEASES/CHANGELOG

[tarballs]: https://axodotdev.github.io/cargo-dist/book/artifacts/archives.html
[installers]: https://axodotdev.github.io/cargo-dist/book/installers/index.html
[manifest]: https://axodotdev.github.io/cargo-dist/book/reference/schema.html
[ci-providers]: https://axodotdev.github.io/cargo-dist/book/ci/index.html

# Read The Book!

We've got all the docs you need over at the [dist book](https://axodotdev.github.io/cargo-dist/book/)!

* [Introduction](https://axodotdev.github.io/cargo-dist/book/introduction.html)
* [Install](https://axodotdev.github.io/cargo-dist/book/install.html)
* [Way-Too-Quickstart](https://axodotdev.github.io/cargo-dist/book/quickstart/index.html)
* [Workspaces Guide](https://axodotdev.github.io/cargo-dist/book/workspaces/index.html)
* [Reference](https://axodotdev.github.io/cargo-dist/book/reference/index.html)

<div class="oranda-hide">

# Contributing

## Updating Snapshots

dist's tests rely on [cargo-insta](https://crates.io/crates/cargo-insta) for snapshot testing various
outputs. This allows us to both catch regressions and also more easily review UI/output changes. If a snapshot
test fails, you will need to use the `cargo insta` CLI tool to update them:

```sh
just dev-install
```

Once installed, you can review and accept the changes with:

```sh
cargo insta review
```

If you know you like the changes, just use `cargo insta accept` to auto-apply all changes.

(If you introduced brand-new snapshot tests you will also have to `git add` them!)

> NOTE: when it succeeds, cargo-dist-schema's `emit` test will actually commit the results back to disk to `cargo-dist-schema/cargo-dist-schema.json` as a side-effect. This is a janky hack to make sure we have that stored and up to date at all times (the test also uses an insta snapshot but insta snapshots include an extra gunk header so it's not something we'd want to link end users). The file isn't even used for anything yet, I just want it to Exist because it seems useful and important. In the future we might properly host it and have our outputs link it via a `$schema` field.

## Cutting Releases

dist is self-hosting, so you just need to push a git-tag with the right format to "do" a release. Of course there's lots of other tedious tasks that come with updating a release, and we use cargo-release to handle all those mechanical details of updating versions/headings/tags. See [these sections of the docs for the release workflow we use](https://axodotdev.github.io/cargo-dist/book/workspaces/cargo-release-guide.html#using-cargo-release-with-pull-requests).

TL;DR:

* Update CHANGELOG.md's "Unreleased" section to include all the release notes you want
* run cargo-release as described in the docs
* ..you're done!

Note that we've wired up dist and cargo-release to understand the "Unreleased" heading so you
should never edit that name, the tools will update it as needed.

If that releases succeeds, we recommend updating the bootstrap version of dist as a follow up:

* install the version of dist you just released on your system
* run `dist init --yes`
* commit "chore: update bootstrap dist to ..."

Note that as a consequence of the way we self-host, dist's published artifacts will always be built/generated by a previous version of itself. This can be problematic if you make breaking changes to cargo-dist-schema's format... so don't! Many things in the schema are intentionally optional to enable forward and backward compatibility, so this should hopefully work well!

</div>
