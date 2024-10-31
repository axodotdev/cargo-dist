# Using cargo-release

<!-- toc -->

> NOTE: It will be helpful to read [the section on dist Announcement Tags][announcements], because that is the interface boundary between cargo-release and dist. TL;DR: dist interprets a git tag of "v1.0.0" as "Announce/Release the whole workspace" (Unified Announcement) and "my-app-v1.0.0" or "my-app/v1.0.0" as "Announce/Release that one package" (Singular Announcement).

> NOTE: this guide assumes you're running [cargo-release v0.22.0][release-22] or greater, as that version made several significant changes to default behaviours (for the better!).

dist intentionally doesn't handle these steps of cutting a release for you:

* updating the versions of your packages
* writing your release notes
* committing the results
* tagging your commits
* pushing to your repo
* publishing to crates.io

There's a lot of different workflows for these things and we're happy to leave that to you. All dist cares about is that a tagged commit eventually ends up in your repo (and that the format of that commit reflects the versions/names in your Cargo.tomls).

That said, you might find [cargo-release][] useful because it can handle all of the above things for you in a single command like `cargo release 1.0.0`. This section is dedicated to explaining how to use cargo-release with dist in various situations.



## cargo-release Basics

> NOTE: cargo-release will never do anything side-effectful unless you also pass it `--execute`. Unless otherwise specified, we are discussing the behaviour when that flag is passed, but will be omitting it for safety/brevity.

In [a simple project][simple-guide] with one package, without any configuration set for cargo-release, the command `cargo release 1.0.0` is roughly sugar for:

```sh
<does some basic checks for uncommitted files and upstream being ahead>
<edits your Cargo.toml to have version 1.0.0>
git commit -am "chore: Release my-app version 1.0.0"
git tag v1.0.0
cargo publish
git push --atomic <remote-branch> refs/tags/v1.0.0
```

(The `git push --atomic` is basically a more robust version of `git push && git push --tags`)

Hey neat that's basically everything I listed at the start of this section! And the tag format is exactly what dist expects for [a simple project][simple-guide]!! What a coincidence!!! 😸

If you don't want some of these behaviours, you can disable them permanently with `[workspace.metadata.release]` in your Cargo.toml, or disable temporarily with CLI flags. See the [cargo-release reference][cargo-release-ref] for all the details but here's some important ones to only get a subset of the behaviours:

* Don't want to publish? Set `publish = false` in the config or pass `--no-publish`
* Don't want to push? Set `push = false` in the config or pass `--no-push`
* Don't want to tag? Set `tag = false` in the config or pass `--no-tag`

See [this section for specific details on using cargo-release with github pull requests (PRs)][with-pr].

Note also that you can use `[package.metadata.release]` to set configs on individual packages and not the whole workspace.




## cargo-release Advanced Usage

With [a more complex project/workspace][workspace-guide], cargo-release won't work as well out of the box with dist. To understand why, we need to understand the rules it applies consistently that can be strange if unexpected.

When you run `cargo release` **it should follow the same rules cargo does for selecting the subset of the workspace to operate on**. That is, if you were to run `cargo test`, the packages that actually get tested are the same ones that `cargo release` will attempt to release! I'll try to briefly summarize (imperfectly, workspaces can get really Complicated):

* When run in the subdirectory of a package, execution is scoped to that package
* When run in the root of a [virtual workspace][] (where the root Cargo.toml isn't an actual package), execution applies to all packages
* When run in the root of a non-virtual workspace (where the root Cargo.toml is probably the "main" package that all other packages exist to implement), execution **only applies to the root package**.
* When run with `--workspace`, execution applies to all packages (good for making a non-virtual workspace behave more like a virtual one).
* Specific packages can be selected with `-p`/`--package`
* I haven't checked if cargo-release respects [default-members][] but that's a thing too!

**By default, cargo-release will create a separate git tag for every package it's releasing.** The default format of these tags depends on the shape of your workspace:

* If there is a root package (the workspace is non-virtual), releases of the root package will be tagged as `v{VERSION}` ("v1.0.0").
* All other packages will be tagged `{PACKAGE_NAME}-v{VERSION}` ("my-app-v1.0.0")

As we'll see below, these combined behaviours have the following interactions with dist:

* ✅ one package workspace: tags it like "v1.0.0"
* ✅ virtual workspace, independent versions: tags each package like "my-app-v1.0.0"
* ✅ virtual workspace, independent versions: tags each package like "my-app/v1.0.0" (needs additional configuration in cargo-release, see below)
* ❌ virtual workspace, unified versions: we want a single tag like "v1.0.0"
* ❌ non-virtual workspace: it will mix the tag formats, which *might* be ok in one situation

Now let's dig into each of these situations in more detail.




## One Package

TLDR: cargo-release Just Works.

```sh
cargo release 1.0.0
```


-------

As stated previously, cargo-release works great with dist if you have [a simple project][simple-guide] consisting of a single package (the kind of project `cargo new my-app` or `cargo init my-app` will create).

See the previous sections for what this will do and how to configure the behaviour if, e.g. you want to hold off on publishing to crates.io or pushing.

The more general version of this situation -- where you have one root package and all the other workspace members are libraries that exist to implement it -- has two possible solutions depending on how you want to version/release the libraries:

* [version/release the libraries in lockstep][non-virtual-unified-section]
* [version/release the libraries separately][all-libs-section]




## Virtual Workspace With Independent Versions

TLDR: cargo-release just needs you to specify which package to release.

```sh
cargo release -p my-package 1.0.0
```

--------


If you have a [virtual workspace][] (one where the root Cargo.toml isn't an actual package) and want everything in the workspace to be versioned/released independently, then dist will default to operating on all your packages at once, and you should do the same thing you would do if you were running `cargo publish`: either use `-p` to select the relevant packages or `cd` into the subdir of that package before running the command.

Each tag will induce dist to produce an independent Announcement (Github Release) for that package.

If the package is a library the Github Release won't have any builds/artifacts uploaded. [See here for details][lib-hack].

Note that we currently don't support finding/emitting Release Notes for Singular Releases (simply haven't had time to design and implement it yet).

### Using slash in tag prefix with cargo-release

For cargo-release to work with tag prefixes that use a slash, you must configure it to use a different prefix for tags in `Cargo.toml`.

For a virtual workspace, put the following in your root Cargo.toml:

```toml
[workspace.metadata.release]
tag-prefix = "{{crate_name}}/"
```

Please refer to [the cargo-release reference][cargo-release-ref-config] for further information on how you can configure cargo-release.



## Virtual Workspace With Unified Versions

TLDR: cargo-release just needs you to specify that versioning/tagging should be unified.

```toml
# Add this config to your root Cargo.toml (virtual manifest)
[workspace.metadata.release]
shared-version = true
tag-name = "v{{version}}"
```

```sh
cargo release 1.0.0
```

----------------

If you have a [virtual workspace][] (one where the root Cargo.toml isn't an actual package) and want everything in the workspace to be versioned/released in lockstep with a single Unified Announcement (One Big Github Release), then you're going to need to configure cargo-release as above.

After that it works perfectly, and cargo-release will even automagically handle publishing your packages to crates.io in the right sequence and waiting for the publishes to propagate before running the next one (no more "oops sorry the package you just published isn't actually propagated to the registry yet so the package that depends on it can't be published").

(See the next section on non-virtual workspaces with unified versions for some grittier details on what's going on here.)



## Non-Virtual Workspace With Unified Versions

TLDR: this is much the same as the virtual workspace case **but you need to pass --workspace on the CLI**.

```toml
# Add this config to your root Cargo.toml (virtual manifest)
[workspace.metadata.release]
shared-version = true
tag-name = "v{{version}}"
```

```sh
cargo release 1.0.0 --workspace
```

--------------

If you have a non-virtual workspace (one where the root Cargo.toml is a package) and want everything in the workspace to be versioned/released in lockstep with a single Unified Announcement (One Big Github Release), then it's *almost* the same as the virtual case (see the previous section).

The one caveat is that dist is consistent to a fault here, and even though we've explicitly told it things should be versioned/tagged in lockstep, **running it in the root of your project still only releases the root package**, and that's not what you want!

We need to tell it that we *really* meant it and pass `--workspace`!

What's happening here is that `cargo-release` is conceptually defined to run on each package individually, with just the "git push" step being unified. The tagging settings we're providing work because it's basically repeatedly going "oh hey I was already going to make that tag, no need to make it again". It doesn't have a proper notion of the entire workspace being released in perfect lockstep, so if you ask it to release only some of the packages it will happily oblige.

In the virtual workspace this Just Works because commands in the root directory are implicitly `--workspace`.



## Non-Virtual Workspace With Totally Independent Versions

TLDR: this is a more complicated mess because but you *probably* want to make the root package have the Singular Announcement format, and then you just need to be explicit about each package you want to release on the CLI:

```toml
# Add this config to your root Cargo.toml (main package)
[package.metadata.release]
tag-name = "{{crate_name}}-v{{version}}"
```

```sh
cargo release -p my-package 1.0.0
```

-------------

If you have a non-virtual workspace (one where the root Cargo.toml is a package) and want everything in the workspace to be versioned/released independently, then the simplest approach is to make everything behave like it does in the [Virtual Workspace With Independent Versions][virtual-independent-section].

However if you find yourself in this position it's likely that your workspace actually looks like:

* root package is The One Application this project exists to develop
* all other packages are libraries that support it

In this *precise* configuration you may be able to avoid configuration by adopting a hybrid "Partially Independent Versions" approach as described in the next section.



## Non-Virtual Workspace With Independent Libraries

TLDR: technically this Just Works but you need to be specific about what packages you're publishing and may have annoying issues in the future.

```sh
cargo release -p my-package 1.0.0
```

-----------

So if your workspace looks like this:

* root package is The One Application this project exists to develop
* all other packages are libraries that support it

Whenever you `cargo release` the root package, it will get tagged without a prefix ("v1.0.0") and dist will create a Unified Announcement. Even though there are other packages in the workspace, dist will take this in stride because as far as it's concerned **this looks exactly the same as a workspace with one package**. Which is to say, it's no different from [a simple project][simple-guide] as far as dist is concerned.

Whenever you `cargo release` a library, it will get tagged with a prefix ("my-lib-v1.0.0") and dist will create a minimal Singular Announcement. [See here for details][lib-hack]. In future versions we might change this default (or at least make it configurable).

I have some vague concerns that this will be wonky if you ever introduce a second application to the workspace, but honestly that's probably going to be true regardless of if you were using dist, so maybe it's fine? Really I just don't trust non-virtual workspaces...



## Library-only Workspaces

dist really isn't designed for this but technically you can use the [Singular Library Trick][lib-hack] if you want. If you want dist to properly support this, please let us know!




## Previewing Your Release

cargo-release defaults to dry-run semantics, only doing side-effectful operations if you pass it `--execute`. It will also do its best to detect problems early and error out if things seem wrong. This absolutely rules!

There are two things to keep in mind:

* cargo-release's dry-run is imperfect and has some differences from the real run
* cargo-release isn't aware of dist, so it can't check if what it's about to do will blow up in CI or not

Let's start with the dry-run differences. I don't know them all but the *biggest* one that I hit is that it doesn't fully emulate bumping the versions in your Cargo.tomls. Notably when it checks if `publish` will work, it's building the current version of the packages. If your build is aware of its own version this can cause/miss problems (and you'll see funky stuff like "Upgrading my-app from 1.0.0 to 2.0.0" ... "Packaging my-app 1.0.0").

As for being aware of dist... I want to design some features for this, but I'm not quite sure what it should look like yet.

I think in the short-term, the best I can offer you is "make a temporary git branch and tell cargo-release to --execute but not push/tag/publish, then ask dist what it thinks extremely manually". A rough sketch:

```sh
# make a temp branch where we can mess stuff up
git checkout -b tmp-release

# ask cargo-release what it thinks should happen
# (substitute the actual cargo-release command you'd use here)
cargo release 1.0.0
```

That should end with a line that looks like "Pushing main, v1.0.0 to origin". The first item is the branch it's pushing to, all the following items are all the tags it wants to push. Now that we know the tags, we can ask cargo-release to update the package versions and then ask dist what it thinks of those tags:

```sh
# just bump versions
cargo release 1.0.0 --execute --no-push --no-tag --no-publish

# ask dist what should be produced for the given tag
dist plan --tag=<tag-you-want-to-check>
```

If that runs successfully and prints out the artifacts you expect, that's pretty good sign running cargo-release For Real will work! (You can also try `dist build` if you're worried about the actual build failing.)




## Using cargo-release with Pull Requests

> In this section we will be using `$BRANCH` and `$VERSION` as placeholders for the branch you make your PR on and the version you want to release.

Many teams have policies that prevent pushing to main, and require you to open pull requests instead. This conflicts with the *default* behaviour of cargo-release, but it works fine with some extra flags to encourage it to defer the steps until later. Specifically, use the following to "partially" run cargo-release:

```sh
cargo release --no-publish --no-tag --allow-branch=$BRANCH $VERSION
```

The release process then has the following steps:

* step 0: create a new branch for the PR
* step 1: < finalize things like changelogs and commit >
* step 2: **partially** run `cargo release ...` to update your Cargo.tomls and push your branch
* step 3: < open a pr, review, merge >
* step 4: **fully** run `cargo release` on main to complete the process (publish and tag)

Crucially, neither invocation of `cargo release` will modify your main branch directly. Step 4 will only push a git tag for the commit that is already on main.

Here's what this looks in practice:


```sh
# step 0: make a branch
git checkout -b $BRANCH


# step 1: update things like the changelog
# < edit some files or whatever here >
git commit -am "prep release"


# step 2: have cargo-release handle tedious mechanical stuff
# this will:
#  * do some safety checks like "git index is clean"
#  * update version numbers in your crates (and handle inter-dependencies)
#  * git commit -am "chore: release $NAME $VERSION" (one commit for the whole workspace)
#  * git push (remember we're on a branch)
cargo release --no-publish --no-tag --allow-branch=$BRANCH $VERSION


# step 3: open a PR and review/merge to main
# NOTE: the above steps will result in two commits
#       we recommend using github's "merge and squash" feature to clean up
# ...


# step 4: remove the shackles from cargo release and RUN ON MAIN
# this will:
#  * tag the commit
#  * push the tag
#  * publish all crates to crates.io (handles waiting for dep publishes to propagate)
#  * trigger dist when it sees the tag (if applicable)
# THIS WON'T CREATE NEW COMMITS
#
# running "dist plan" is totally optional, but this is is the best time to check
# that your dist release CI will produce the desired result when you push the tag
git checkout main
git pull
dist plan
cargo release
```

[cargo-release]: https://github.com/crate-ci/cargo-release
[simple-guide]: ./simple-guide.md
[cargo-release-ref]: https://github.com/crate-ci/cargo-release/blob/master/docs/reference.md
[cargo-release-ref-config]: https://github.com/crate-ci/cargo-release/blob/master/docs/reference.md#configuration
[workspace-guide]: ./workspace-guide.md
[default-members]: https://doc.rust-lang.org/cargo/reference/workspaces.html#the-default-members-field
[virtual workspace]: https://doc.rust-lang.org/cargo/reference/workspaces.html#virtual-workspace
[announcements]: ./workspace-guide.md#announcement-tags
[release-22]: https://github.com/crate-ci/cargo-release/releases/tag/v0.22.0
[virtual-independent-section]: #virtual-workspace-with-independent-versions
[all-libs-section]: #non-virtual-workspace-with-independent-libraries
[non-virtual-unified-section]: #non-virtual-workspace-with-unified-versions
[lib-hack]: ./workspace-guide.md#singular-library-hack
[with-pr]: #using-cargo-release-with-pull-requests
