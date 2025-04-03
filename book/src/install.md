# Install

<!-- toc -->

Surprise! The tool for prebuilt shippable binaries has way too many ways to install it!
Whichever way you choose to install it, it should be invocable as `dist ...`.


## Pre-built binaries

We provide several options to access pre-built binaries for a variety of platforms. If you would like to manually download a pre-built binary, checkout [the latest release on GitHub](https://github.com/astral-sh/cargo-dist/releases/latest).

This is an unofficial fork, so only the shell script installers are supported.

### Installer scripts

#### macOS and Linux (not NixOS):

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/astral-sh/cargo-dist/releases/latest/download/cargo-dist-installer.sh | sh
```

#### Windows PowerShell:

```sh
powershell -c "irm https://github.com/astral-sh/cargo-dist/releases/latest/download/cargo-dist-installer.ps1 | iex"
```


[Rust]: https://rust-lang.org
[cargo]: https://doc.rust-lang.org/cargo/index.html
[installed the Rust toolchain (`rustup`)]: https://rustup.rs/
