# Guide: More Complex Workspaces

Now that we've [looked at a simple example][] with `cargo new`, let's start looking at ways to make a [Cargo Workspace][] more complicated, and how cargo-dist will deal with them.

But first, let's define some precise terminology: 

Rust projects typically exist as a single *[Workspace][workspace]*, which is a collection of one or more *[Packages][package]* that are all developed in the same repository ([crates.io][crates-io] dependencies are not considered part of the workspace). A workspace always has a root Cargo.toml where certain workspace-global settings are defined. 

If the root Cargo.toml *doesn't* define a Package then we say it's a *[Virtual Workspace][virtual-workspace]*. A Virtual Workspace puts all the packages on the same level, treating them as equals. If you don't use a Virtual Workspace you are essentially saying the entire project exists to produce that one root Package. Both approaches make sense in different contexts. I personally prefer virtual workspaces because it makes cargo (and other tools) default to operating on all packages at once, which is usually what I want (e.g. I want `cargo test` to test the entire workspace, I want `cargo fmt` to format the whole workspace, and so on).

A *[Package][package]* is the thing defined by a Cargo.toml (except for the root Cargo.toml of a Virtual Workspace, which defines no package). Many people reasonably assume "Package" and "Crate" are synonyms -- after all you host your Packages on a website called "[crates.io][crates-io]"! As it turns out, this is not the case: a Package can in fact define multiple Crates at the same time.

A *[Crate][]* is the actual unit of compilation that *rustc* thinks about, like a single library or binary. For the purposes of cargo-dist, you don't really need a perfect understanding of what is or isn't a "crate". The important takeaway is that a single Package can contain multiple things that are conflated with a single unified name and version. As we'll see, this can be useful.



## Multiple Binaries In One Package

So here's where the difference between a "Package" and a "Crate" is most relevant: [Cargo lets a single Package define multiple binaries][bins]. See those docs for all the details. This can be convenient if you want to produce a single logical application that provides a suite of CLIs. For instance, you might want to make a standalone "my-tool" CLI that can be invoked as `cargo my-tool` as well. The easiest way to do this is to define a second "cargo-my-tool" binary as part of the "my-tool" Package. Once you do, `cargo install my-tool` will install both!

cargo-dist tries to respect this semantic. If you define multiple binaries in a Package, we will treat the Package as one "Application" and bundle both binaries in all zips and installers for that App. There is no way to override this behaviour -- if you don't want two binaries to be considered part of the same App, you should use separate Packages.


## Multiple Packages In A Workspace

Alright here's where things get a bit more complicated and you need to make a decision on how exactly you plan to develop and release the packages that make up your project. Up until now we've been assuming you have a single package in your workspace, but now we're going to deal with more.

How cargo-dist interprets multiple packages is actually fairly simple:

* Each Package that defines binaries is considered an "App" with completely independent zips/installers
* Each Package that doesn't define binaries is wholy irrelevant and ignored

If a Package defines binaries but you want cargo-dist to ignore it just like it does with library-only packages (i.e. because the binaries are for local testing), you can do that with either:

* [`publish = false` in that Package's Cargo.toml][publish-config]
* [`dist = false` in that Package's `[package.metadata.dist]`][dist-config]

Now here's the really important question you need to answer: **how do you want to announce new versions of your packages?**


## Announcement Tags

When you push a Git Tag to your repository, cargo-dist's CI will try to create a single Announcement (A Github Release) for that tag. When you only have one Package that's a completely unambiguous operation. When you have multiple Packages we now need some way to disambiguate what you actually meant.

1 Git Tag = 1 cargo-dist Announcement = 1 Github Release

cargo-dist supports two forms of Announcement which you can select with the format of your Git Tag:

* Unified Announcement: `v{VERSION}` selects all packages with the given version (v1.0.0, v0.1.0-prerelease, ...)
* Singular Announcement: `{PACKAGE-NAME}-v{VERSION}` selects only the given package (my-app-v1.0.0, my-app-v1.0.0-prerelease, ...)

These two modes support the following workflows:

* Releasing a workspace with only one App (either mode works but Unified is Best)
* Releasing a workspace where all Apps are versioned in lockstep (Unified)
* Releasing an individual App in a workspace with its own independent versioning (Singular)
* Releasing several Apps in a workspace at once, but all independently (Push multiple Singular tags at once)

> NOTE: Although you *could* use extremely careful versioning in conjuction with Unified Announcements to release a weird subset of the packages in your workspace, you really *shouldn't* because the Github Releases will be incoherent (v0.1.0 has these random packages, v0.2.0 has these other random packages... huh?), and you're liable to create painful tag collisions.

> NOTE: Normally cargo-dist will error out if the Announcement Tag selects no Apps, because it exists to build and distribute Apps and you just asked it to do nothing (which is probably a mistake). This would however create annoying CI errors if you just wanted to tag Individual Releases for your libraries. To make this more pleasant, **cargo-dist will produce a very minimal build-less Announcement (and therefore Github Release) if you explicitly request a Singular Announcement that matches a library-only package**. This feature is kind of half-baked, please let us know what you want to happen in this situation!

**The need for a coherent Announcement Tag is so important that cargo-dist commands like "build" and "manifest" will error out if one isn't provided and it can't be guessed.** If that happens you may need to pass an explicit `--tag=...` flag to disambiguate. Being this strict helps catch problems before you push to CI.




[simple-guide]: ./simple-guide.html
[workspace]: https://doc.rust-lang.org/cargo/reference/workspaces.html
[package]: https://doc.rust-lang.org/cargo/appendix/glossary.html#package
[virtual-workspace]: https://doc.rust-lang.org/cargo/reference/workspaces.html#virtual-workspace
[crate]: https://doc.rust-lang.org/book/ch07-01-packages-and-crates.html
[crates-io]: https://crates.io/
[bins]: https://doc.rust-lang.org/cargo/reference/cargo-targets.html#binaries
[publish-config]: TODO://link-to-publish-config
[dist-config]: TODO://link-to-dist-config