# Generic Builds: A Quickstart Guide

<!-- toc -->

> This guide covers cargo-dist's *generic builds* support: that is, building for any language other than Rust. If you're looking to build Rust software specifically, check the [Rust quickstart guide][quickstart].

So you've written a piece of software and you'd like to distribute it, but managing CI and installers is hard. cargo-dist's generic build support lets you access all the same building and distribution features you get from Cargo-based builds in any language. This guide will help you get up and running as quickly as possible.

## Setup and configuration

Once you've [installed][install] cargo-dist, you're ready to get started. Prepping your app for cargo-dist requires just a little bit of configuration.

cargo-dist uses a custom configuration format called `dist.toml`, written in the [TOML][toml] format. cargo-dist can manage most of your settings for you, but we'll need to write a little bit of information to tell cargo-dist about your software and what it needs to expect.

To start, create a file named `dist.toml` in the root of your repository. The top of the file needs a field named `[package]` containing some basic metadata about your project, which looks like this:

```toml
[package]
# The name of your package; cargo-dist will use this in your installers and announcements
name = "quickstart-example"
# (Optional) Descriptive text about your package; some installers will present this to users
description = "This is a description of your package"
# The current version of your package - you'll update this with every release
version = "1.0.0"
# (Optional) Your package's license
license = "GPL-3.0-only"
# The URL to package's git repository
repository = "https://github.com/example/example"
# A list of all binaries your package will build and install
binaries = ["quickstart-example"]
# A command cargo-dist should run that will build your project
build-command = ["make"]
```

Once you've created this file, we can ask cargo-dist to generate the rest of its configuration for us: just run `cargo dist init`, and answer all the questions it asks you. (If you don't care about those questions yet and want to Just Get Building, you can run `cargo dist init --yes` and let it pick the defaults.) In the future, any time you want to update these settings, you can just rerun `cargo dist init`. You'll also want to run this command any time you want to update which version of cargo-dist builds your package.

Just to really emphasize that: `cargo dist init` is designed to be rerun over and over, and will preserve your settings while handling any necessary updates and migrations. Always Be Initing.

Once you've run `init`, check your `dist.toml`: cargo-dist has added a bunch of new settings with all the choices you made. If you chose to turn on GitHub CI, you'll also see that it's created a `.github/workflows/release.yml` for you: this will be run every time you create a release of your software.

## I just want to see the builds

Now that we're all set up, you can run a build locally and see what it looks like. Just run `cargo dist build`, and cargo-dist will go ahead and run your build process for you. Your binaries, installers, and tarballs will all get placed in the `target/distrib` directory inside your project where you can take a look at them and see if they look right. (Your installers won't work at this stage, since they'll try to pull things down from GitHub releases that you haven't created yet, but everything else should work fine.)

If you don't actually want to *run* your build yet, but just want to see what cargo-dist *would* build, you can run the `cargo dist plan` command. This will print a nice convenient tree showing all of the archives and installers it'll produce, including what files go inside those archives.

## Your first release

Once you're satisfied all your binaries and installers look right, it's time to create your first release! First, make sure you've committed the files that cargo-dist generated for you.

cargo-dist releases work on git tags: any time you push a new tag, cargo-dist will go looking for things to build and create a release from them. To create a release, just update the version in your `dist.toml`, then create a git tag with the same version. For example, if this is the first release of your software, you could set the version to `0.1.0` and run `git tag v0.1.0` to create that tag. Then, just `git push --tags` to push that new tag to your repo, and cargo-dist will get building.

To watch the release process, head to the "Actions" tab in your repo's menubar. You should see an in-progress build churning away. Once that builds completes, cargo-dist will automatically create a new release in the "Releases" tab on the right, and you're done! You can run any of the installation commands in the release body to pull down your software and give it a try.

[install]: ./install.md
[quickstart]: ../way-too-quickstart.md
[toml]: https://toml.io/en/
