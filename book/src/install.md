# Install

<!-- toc -->

Surprise! The tool for prebuilt shippable binaries has way too many ways to install it!
Whichever way you choose to install it, it should be invocable as `cargo dist ...`. If you insist on invoking the binary directly as `cargo-dist` you must still add the extra `dist` arg and invoke it as `cargo-dist dist ...` (a quirk of the way cargo invokes subcommands).


## Use The Installer Scripts

macOS and Linux (not NixOS, Alpine, or Asahi):

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/axodotdev/cargo-dist/releases/latest/download/cargo-dist-installer.sh | sh
```

Windows PowerShell:

```sh
irm https://github.com/axodotdev/cargo-dist/releases/latest/download/cargo-dist-installer.ps1 | iex
```

## Build From Source With Cargo

```sh
cargo install cargo-dist --locked
```


## Install Prebuilt Binaries With cargo-binstall

```sh
cargo binstall cargo-dist
```

## Installation on Arch Linux

Arch Linux users can install `cargo-dist` from the [extra repository](https://archlinux.org/packages/extra/x86_64/cargo-dist/) using [pacman](https://wiki.archlinux.org/title/Pacman):

```sh
pacman -S cargo-dist
```

## Download Prebuilt Binaries From Github Releases

[See The Latest Release](https://github.com/axodotdev/cargo-dist/releases/latest)!
