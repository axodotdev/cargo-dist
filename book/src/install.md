# Install

Surprise! The tool for prebuilt shippable binaries has way too many ways to install it!
Whichever way you choose to install it, it should be invocable as `cargo dist ...`. If you insist on invoking the binary directly as `cargo-dist` you must still add the extra `dist` arg and invoke it as `cargo-dist dist ...` (a quirk of the way cargo invokes subcommands).

## Install Prebuilt Binaries With cargo-binstall

```sh
cargo binstall cargo-dist
```

## Build From Source With Cargo

```sh
cargo install cargo-dist --locked --profile=dist
```

(`--profile=dist` is the profile we build our shippable binaries with, it's optional.)


## Install From The AUR

Arch Linux users can install `cargo-dist` from the [AUR](https://aur.archlinux.org/packages?O=0&SeB=nd&K=cargo-dist&outdated=&SB=p&SO=d&PP=50&submit=Go) using an [AUR helper](https://wiki.archlinux.org/title/AUR_helpers). For example:

```sh
paru -S cargo-dist
```

## Download Prebuilt Binaries From Github Releases

[See The Latest Release](https://github.com/axodotdev/cargo-dist/releases/latest)!

## Use The Installer Scripts

**NOTE: these installer scripts are currently under-developed and will place binaries in `$HOME/.cargo/bin/` without properly informing Cargo of the change, resulting in `cargo uninstall cargo-dist` and some other things not working. They are however suitable for quickly bootstrapping cargo-dist in temporary environments (like CI) without any other binaries being installed.**

Linux and macOS:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/axodotdev/cargo-dist/releases/latest/download/cargo-dist-installer.sh | sh
```

Windows PowerShell:

```sh
irm https://github.com/axodotdev/cargo-dist/releases/latest/download/cargo-dist-installer.ps1 | iex
```