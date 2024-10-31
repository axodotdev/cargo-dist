# Archives

Archives are the primary output of dist: a single file (zip or tarball) containing prebuilt executables/binaries for an app, along with additional static files like READMEs, LICENSEs, and CHANGELOGs. The docs previously referred to these as "executable-zips", so if you ever see that term floating around, this is what's being talked about.

When you [tell us to build an app][apps] for [a platform][config-targets] we will always make an archive for it.

[Fetching installers][fetching-installers] will fetch and unpack archives from wherever you [uploaded them][artifact-url]. [Bundling installers][bundling-installers] will use an exact copy of the binary stored in the archive, but may differ on other included files.



## Auto-Detected Files

We will always auto-detect READMEs, LICENSES, and CHANGELOGs with the following logic (described more below):

* README: [package.readme][config-package-readme], or find `README*`
* LICENSE: [package.license-file][config-package-license-file], or find `LICENSE*`/`UNLICENSE*`
* CHANGELOG: find `CHANGELOG*`/`RELEASES*`

"Find `XYZ*`" means we will look for a file whose name starts with "XYZ" in the same directory as the Cargo.toml for a package that defines the app. If no such file is found, we will also search for it in the same directory as the workspace's Cargo.toml (so packages "inherit" these files from the workspace).

It is generally assumed that a directory only contains one of each kind of file. If multiple possible matches are in the same directory we will arbitrarily pick the first one we saw, so don't rely on that.

Auto-detected files are first and foremost [auto-included into the archive](#archive-contents), however they can also be used for other things. For instance, the autodetected CHANGELOG is fed into our CHANGELOG features.



## Archive Contents

The "root" of an archive is either the actual root directory of the archive (zips); or a directory with the same name as the archive, but without the extension (tarballs). This difference is for compatibility/legacy reasons, and can be smoothed away by unpacking tarballs with tar's `--strip-components=1`.

An app's archive always includes its binaries at the root.

By default [auto-detected files](#auto-detected-files) for a package are auto-included into its archives at the root of the package. The [auto-includes][config-auto-includes] config controls this behaviour.

The [include][config-include] can be used to manually add specific files/directories to the root of the archive.



## Archive Formats

Archives can be zips or tarballs (gz, xz, or zstd).

By default we make .zip on windows and .tar.xz elsewhere, but this can be configured with [windows-archive][config-windows-archive] and [unix-archive][config-unix-archive] features.




## Build Flags

We currently [always build with `--profile=dist`][dist-profile]

By default we build with `--workspace` [to keep things consistent][workspace-hacks], but this can be configured with the [precise-builds config][config-precise-builds] (see those docs for details on when precise-builds will be force-enabled).

By default we build your packages with default features, but this can be configured with the [features][config-features], [default-features][config-default-features], and [all-features][config-all-features] configs.

When targeting windows-msvc we will unconditionally [append "-Ctarget-feature=+crt-static"][crt-static] to your RUSTFLAGS, which should just be the default for rustc but isn't for legacy reasons.

We don't really [support cross-compilation][issue-cross], but we'll faithfully attempt the compile by telling rustup to install the toolchain and passing `--target` to cargo as instructed -- it will probably just fail. On macOS cross-compiles between Intel and Apple Silicon will work. [linux-musl is slated for a future version][issue-musl].



## Code Signing

"Code Signing" is a very overloaded term, with wildly varying implementations that accomplish different goals. For instance, Linux users are currently very big on [sigstore][issue-sigstore] as a fairly turn-key code signing solution, but [neither Windows nor macOS][issue-native-sign] acknowledge its existence (and likely never will, as the benefits of sigstore completely defeat the stated purpose of code signing requirements on those platforms).

Roughly speaking, codesigning can be broken up into "Is this app made by the developer?" and "Can I trust apps made by this developer?". Tools like sigstore are focused on the former, while Windows/macOS only care about the latter. They want you to pay some money and jump through administrative hoops. They also expect you to pay completely different groups and go through completely different hoops, so each platform requires a completely different solution.


[config-package-readme]: ../reference/config.md#readme
[config-package-license-file]: ../reference/config.md#license-file
[config-windows-archive]: ../reference/config.md#windows-archive
[config-unix-archive]: ../reference/config.md#unix-archive
[config-precise-builds]: ../reference/config.md#precise-builds
[config-default-features]: ../reference/config.md#default-features
[config-all-features]: ../reference/config.md#all-features
[config-features]: ../reference/config.md#features
[config-include]: ../reference/config.md#include
[config-auto-includes]: ../reference/config.md#auto-includes
[config-targets]:  ../reference/config.md#targets

[issue-musl]: https://github.com/axodotdev/cargo-dist/issues/75
[issue-cross]: https://github.com/axodotdev/cargo-dist/issues/74
[issue-sigstore]: https://github.com/axodotdev/cargo-dist/issues/120
[issue-native-sign]: https://github.com/axodotdev/cargo-dist/issues/21

[apps]: ../reference/concepts.md#defining-your-apps
[fetching-installers]: ../installers/index.md#fetching-installers
[bundling-installers]: ../installers/index.md#bundling-installers
[artifact-url]: ../reference/artifact-url.md
[dist-profile]: ../workspaces/simple-guide.md#the-dist-profile

[crt-static]: https://rust-lang.github.io/rfcs/1721-crt-static.html
[workspace-hacks]: https://docs.rs/cargo-hakari/latest/cargo_hakari/about/index.html#what-are-workspace-hack-crates
