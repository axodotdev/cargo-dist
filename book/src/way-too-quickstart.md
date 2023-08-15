# Way-Too-Quickstart

<!-- toc -->

> TLDR: cargo-dist is a souped up version of `cargo build` which handles building tarballs/zips and [installers][]. It also knows how to generate Github CI for orchestrating itself and uploading its output to a new Github Release. You can use cargo-dist if you don't care about that CI stuff, but this guide assumes that you do.
>
> This quickstart is a bit *too* quick because there's some important nuances to "announcing and building releases" that depend on the way you like to structure and version your workspace. We will blatantly ignore those nuances and show you the Happiest Happy Path (a workspace with one crate that defines a binary). Checkout [the guide][guide] for more details on what you should *actually* do.



## Setup

Setting up just requires you to [install cargo-dist][install] and then run `cargo dist init` in your [Cargo workspace][workspace]. This command interactively walks you through configuration options, and can be run again whenever you want to change your settings. Since this is the *way-too*-quickstart, we pass `--yes` to auto-accept all defaults!

```sh
# install tools (build from source is the most portable option)
cargo install cargo-dist

# setup cargo-dist in your project (--yes to accept defaults)
cargo dist init --yes
git add .
git commit -am "wow shiny new cargo-dist CI!"
```

The one-time setup will add a decent default configuration to your root Cargo.toml and generate CI for orchestrating itself in `.github/workflows/release.yml`. If the CI file isn't created, this probably means you don't have `repository = "https://github.com/..."` consistently set in your Cargo.toml(s).



## Test Locally

When testing out cargo-dist locally, the two biggest things you might be interested in are:

1. build for the current platform (`cargo dist build`)
2. check what CI will build (`cargo dist plan`)




### Build For The Current Platform

```sh
cargo dist build
```

![Running "cargo dist build" on a project, resulting in the application getting built and bundled into a .zip, and an "installer.ps1" script getting generated. Paths to these files are printed along with some metadata.][quickstart-build]

The [build command][build] will by default try to build things for the computer you're running it on. So if you run it on linux you might get a `tar.xz` containing your binary and an installer shell script, but if you run it on windows you might get a `zip` and an installer *power*shell script.

cargo-dist will then spit out paths to the files it created, so you can inspect their contents and try running them (**note that installer scripts probably won't be locally runnable, because they will try to fetch their binaries from Github**).

See [artifact modes][artifact-modes] for more advanced details on selecting what things cargo-dist should build.




### Check What CI Will Build

```sh
cargo dist plan
```

![Running "cargo dist plan" on a project, producing a full printout of the tarballs/zips that will be produced for all platforms (mac, linux, windows), and all installers (shell, powershell)][quickstart-build]

The [plan command][plan] should be running the exact same logic that cargo-dist's generated CI will run, but without actually building anything. This lets you quickly check what cutting a new release will produce. It will also try to catch any inconsistencies that could make the CI error out.




## Cut A Release (Trigger Github CI)

cargo-dist largely doesn't care about the details of how you prepare your release, and intentionally doesn't provide tools to streamline it (see the next section for some recommendations). All it cares about is you getting your main branch into the state you want, and then pushing a properly formatted git tag like "v0.1.0". Here's a super bare-bones release process:

```sh
# <manually update the version of your crate, run tests, etc>

# commit and push to main (can be done with a PR)
git commit -am "chore: Release version 0.1.0"
git push

# publish to crates.io (optional)
cargo publish

# actually push the tag up (this triggers cargo-dist's CI)
git tag v0.1.0
git push --tags
```

The important parts are that you update the crates you want to release/announce to the desired version and push a git tag with that version. (prefixed with `v`!)

At this point you're done! The generated CI script should pick up the ball and create a Github Release with all your builds over the next few minutes!




### Streamlining Cutting A Release

You may have noticed "cut a release" still has a lot of tedious work. That's because we recommend using [cargo-release][] to streamline the last step, which in *simple workspaces* will do exactly the same thing as above (but more robustly):

```sh
# install tools
cargo install cargo-release

# cut a release
cargo release 0.1.0
```

(I left off the `--execute` flag from `cargo-release` so you won't accidentally break anything if you really did just copy paste that 😇)

For more details on using cargo-release with cargo-dist, see [the guide for that][cargo-release-guide].


[cargo-release]: https://github.com/crate-ci/cargo-release
[guide]: ./guide.md
[install]: ./install.md
[cargo-release-guide]: ./cargo-release-guide.md
[workspace]: https://doc.rust-lang.org/cargo/reference/workspaces.html
[quickstart-build]: ./img/quickstart-build.png
[quickstart-plan]: ./img/quickstart-plan.png
[artifact-modes]: ./concepts.md#artifact-modes-selecting-artifacts
[installers]: ./installers.md
[build]: ./cli.md#cargo-dist-build
[plan]: ./cli.md#cargo-dist-plan
