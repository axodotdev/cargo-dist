# Install

<!-- toc -->

Surprise! The tool for prebuilt shippable binaries has way too many ways to install it!
Whichever way you choose to install it, it should be invocable as `cargo dist ...`. If you insist on invoking the binary directly as `cargo-dist` you must still add the extra `dist` arg and invoke it as `cargo-dist dist ...` (a quirk of the way cargo invokes subcommands).


## Pre-built binaries

We provide several options to access pre-built binaries for a variety of platforms. If you would like to manually download a pre-built binary, checkout [the latest release on GitHub](https://github.com/axodotdev/cargo-dist/releases/latest).

### Installer scripts

#### macOS and Linux (not NixOS):

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/axodotdev/cargo-dist/releases/latest/download/cargo-dist-installer.sh | sh
```

#### Windows PowerShell:

```sh
powershell -c "irm https://github.com/axodotdev/cargo-dist/releases/latest/download/cargo-dist-installer.ps1 | iex"
```

### Package managers

#### Homebrew

```sh
brew install axodotdev/tap/cargo-dist
```

#### Pacman (Arch Linux)

Arch Linux users can install `cargo-dist` from the [extra repository](https://archlinux.org/packages/extra/x86_64/cargo-dist/) using [pacman](https://wiki.archlinux.org/title/Pacman):

```sh
pacman -S cargo-dist
```

### Other Options

#### cargo-binstall

```sh
cargo binstall cargo-dist
```

## Build From Source

For users who need to install cargo-dist on platforms that we do not yet provide pre-built binaries for, you will need to build from source.
`cargo-dist` is written in [Rust] and uses [cargo] to build. Once you've [installed the Rust toolchain (`rustup`)], run:

```sh
cargo install cargo-dist --locked
```

[Rust]: https://rust-lang.org
[cargo]: https://doc.rust-lang.org/cargo/index.html
[installed the Rust toolchain (`rustup`)]: https://rustup.rs/


