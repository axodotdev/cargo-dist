# Artifacts

cargo-dist's primary role is to produce "Artifacts", either to use locally or to upload as part of a release announcement. Here's a listing of the supported kinds!


## executable-zip

executable-zips are the primary output of cargo-dist: a zip (or tarball) containing prebuilt executables/binaries for an app, along with additional static files like READMEs, LICENSES, and CHANGELOGs.

When you [tell us to build an App for a platform][] we will make an executable-zip for it. Universal installers will fetch and unpack executable zips from wherever you uploaded them (currently Github Releases).

You can modify what files get included with the [include][config-include] and [auto-includes][config-auto-includes] configs.

Currently we always make .zip on windows and .tar.xz elsewhere. [This will be made configurable][extension-issue].

Some notes on how we build your executables:

* We currently always build `--workspace` [to keep things consistent][workspace-hacks]
* We currently [always build with `--profile=dist`][dist-profile]
* We currently [always build with default features][features-issue]
* When targeting windows-msvc we will unconditionally [append "-Ctarget-feature=+crt-static"][crt-static-rfc] to your RUSTFLAGS, which should just be the default for rustc but isn't for legacy reasons
* We don't yet [support cross-compilation][cross-issue]. We'll faithfully attempt the compile by passing `--target` to cargo as instructed but it will probably just fail.
    * [linux-musl is slated for a future version][musl-issue]

## symbols

This feature is currently disabled [pending a rework][rework-symbols], but basically we want to save your debuginfo in the form of pdbs, dSYMs, etc. This should automatically happen as a side-effect of requiring executable-zips with the appropriate build settings to generate them.


## installers

Most other kinds of artifact are referred to as "installers", because they generally exist as ways of getting the binaries that were made for the executable-zips. See [installers config][] for details.

Currently supported installers include:

* "shell": a shell script that fetches and installs executable-zips for the current platform
* "powershell": a powershell script that fetches and installs executable-zips for the current platform

In the future installers should hopefully include things like: 

* [Windows MSIs][msi-installer-issue] (one per windows platform)
* [MacOS DMG][dmg-installer-issue] (one per macos platform)
* [NPM Packages][npm-package-issue] (universal)
* Various system package managers

(We should really flesh this section out more and make it the canonical listing of installers other places link to!)


[rework-symbols]: https://github.com/axodotdev/cargo-dist/issues/136
[config-targets]: ./config.md#targets
[config-installers]: ./config.md#installers
[config-include]: ./config.md#include
[config-auto-includes]: ./config.md#auto-includes
[msi-installer-issue]: https://github.com/axodotdev/cargo-dist/issues/23
[npm-package-issue]: https://github.com/axodotdev/cargo-dist/issues/100
[dmg-installer-issue]: https://github.com/axodotdev/cargo-dist/issues/24
[arm64-apple-issue]: https://github.com/axodotdev/cargo-dist/issues/133
[musl-issue]: https://github.com/axodotdev/cargo-dist/issues/75
[extension-issue]: https://github.com/axodotdev/cargo-dist/issues/17
[cross-issue]: https://github.com/axodotdev/cargo-dist/issues/74
[features-issue]: https://github.com/axodotdev/cargo-dist/issues/22
[crt-static-rfc]: https://rust-lang.github.io/rfcs/1721-crt-static.html
[dist-profile]: ./simple-guide.md#the-dist-profile
[workspace-hacks]: https://docs.rs/cargo-hakari/latest/cargo_hakari/about/index.html#what-are-workspace-hack-crates
