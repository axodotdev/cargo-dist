# Homebrew Installer

> since 0.2.0

This provides a [Homebrew](https://brew.sh) formula which allows users to `brew install` your package. Since it installs to a location on the user's `PATH`, it provides a simple and convenient installation method for users who already have Homebrew available. When published to a [tap](https://docs.brew.sh/Taps) (package repository), this gives your users an easy way to both install your package and to keep it up to date using `brew update` and `brew upgrade`. It fetches the same prebuilt macOS binaries as the shell installer.

cargo-dist can, optionally, publish your formula to a tap repository for you on every release. To enable this, add a `tap` field to your `Cargo.toml` pointing to a GitHub repository that you control and add `homebrew` to the `publish-jobs` field. The repository name must start with `homebrew-`. For example:

```toml
tap = "axodotdev/homebrew-formulae"
publish-jobs = ["homebrew"]
```

Since 0.11.0, cargo-dist can, optionally, also customize your Homebrew formula name.
By default, your formula will be named using the app name (in Rust, this is the crate
name). If you are overriding the bin name, you may want to make your Homebrew formula
match- you can do so with config like this:

```toml
[package]
name = "myappname"
version = "0.666.0"
default-run = "mybinname"

[[bin]]
name = "mybinname"
path = "src/main.rs"

tap = "axodotdev/homebrew-formulae"
publish-jobs = ["homebrew"]
formula = "mybinname"
```

In order to write to a tap GitHub repository, cargo-dist needs a [personal access token](https://github.com/settings/tokens/new?scopes=repo) with the `repo` scope exposed as `HOMEBREW_TAP_TOKEN`. For more information on GitHub Actions secrets, [consult this documentation](https://docs.github.com/en/actions/security-guides/encrypted-secrets).

Limitations/Caveats:

* Does not support creating a formula which builds from source
* Does not support Linuxbrew (Homebrew on Linux)
* Does not support [Cask][issue-cask] for more convenient GUI app installation



[issue-cask]: https://github.com/axodotdev/cargo-dist/issues/309
