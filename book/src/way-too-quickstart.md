# Way-Too-Quickstart

This quickstart is a bit *too* quick because there's some important nuances to "announcing and building releases" that depend on the way you like to structure and version your workspace. This section will blatantly ignore those nuances and show you the Happiest Happy Path. Checkout [the guide][guide] for more details on what you should *actually* do.

There are [many ways to install cargo-dist][install]. For simplicity we'll use `cargo install` as that will work everywhere (*stares at NixOS users and people who think they should run desktop RISC-V*):

```sh
# install tools
cargo install cargo-dist

# one-time setup
cargo dist init --ci=github
git add .
git commit -am "wow shiny new cargo-dist CI!"

# cut a release like you normally would

# <manually update the version of your crate, run tests, etc>
# then:
git commit -am "chore: Release version 0.1.0"
git tag v0.1.0
cargo publish
git push
git push --tags
```

The one-time setup will add a decent default configuration to your root Cargo.toml and generate CI for orchestrating itself in `.github/workflows/release.yml`.

The important parts of "cut a release" are that you update the crates you want to release/announce to the desired version and push a git tag with that version. (prefixed with `v`!)

At this point you're done! The generated CI script should pick up the ball and create a Github Release with all your builds over the next few minutes!



## Streamlining Cutting A Release

You may have noticed "cut a release" still has a lot of tedious work. That's because we recommend using [cargo-release][] to streamline the last step, which in *simple workspaces* will do exactly the same thing as above (but more robustly):

```sh
# install tools
cargo install cargo-dist
cargo install cargo-release

# one-time setup
cargo dist init --ci=github
git add .
git commit -am "wow shiny new cargo-dist CI!"

# cut a release
cargo release 0.1.0
```

(I left off the `--execute` flag from `cargo-release` so you won't accidentally break anything if you really did just copy paste that 😇)

For more details on using cargo-release with cargo-dist, see [the guide for that][cargo-release-guide].


[cargo-release]: https://github.com/crate-ci/cargo-release
[guide]: ./guide.md
[install]: ./install.md
[cargo-release-guide]: ./cargo-release-guide.md
