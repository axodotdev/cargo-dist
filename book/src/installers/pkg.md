# pkg Installer

> Since 0.22.0

<!-- toc -->

This guide will walk you through setting up a [bundling][] macOS `pkg` installer, which is the native graphical installer format on macOS. It assumes you've already done initial setup of cargo-dist, as described in [the way-too-quickstart][quickstart], and now want to add a pkg to your release process.

## Setup

### Setup Step 1: run init and enable "pkg"

Rerun `cargo dist init` and when it prompts you to choose installers, enable "pkg". After you've selected "pkg", you'll be asked for two pieces of information:

- An "identifier": this is a unique identifier for your application in reverse-domain name format. For more information, see [Apple's documentation for `CFBundleIdentifier`](https://developer.apple.com/library/archive/documentation/CoreFoundation/Conceptual/CFBundles/BundleTypes/BundleTypes.html#//apple_ref/doc/uid/10000123i-CH101-SW1).
- The install location: by default, the pkg installer will place your software in `/usr/local` on the user's system. You can specify an alternate location if you prefer.

Once init completes, some changes will be made to your project, **check all of them in**:

1. `installers = ["pkg]"]` will be added to `[workspace.metadata.dist]`
2. `[package.metadata.dist.mac-pkg-config]` will be added to your packages with distable binaries.

### Setup Step 2: you're done! (time to test)

See [the quickstart's testing guide][testing] for the various testing options.

If the above steps worked, `cargo dist plan` should now include a pkg for each Mac platform you support. You can create an installer by running `cargo dist build` on a Mac; it will be placed next to your software in the `target/distrib` folder, and can be installed just by double-clicking it.

[quickstart]: ../quickstart/index.md
[testing]: ../quickstart/rust.md#test-it-out
[bundling]: ./index.md#bundling-installers
