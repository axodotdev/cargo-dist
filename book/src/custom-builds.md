# Custom Builds

> since 0.5.0

When releasing software in languages other than Rust or JavaScript, you'll need to tell dist how to build it &mdash; there are more buildsystems than stars in the sky, and dist can't know how to run all of them (or how to figure out what to release from them).

This guide assumes you've already initialized the dist config; check the [quickstart guide][quickstart-everyone-else] for how to get started.

## Examples

* [example npm project](https://github.com/axodotdev/axolotlsay-js)
* [example C project](https://github.com/axodotdev/cargo-dist-c-example)

### Understanding build commands

Build commands are the core difference between these builds and Rust builds. Since we don't have Cargo to rely on to tell us how to build your package, it's up to you to tell us how instead.

As an example, let's imagine a C program with a simple makefile-based buildsystem. Its `dist.toml` looks something like this:

```toml
[package]
# Your app's name
name = "my_app"
# The current version; make sure to keep this up to date!
version = "0.1.0"
# The URL to the git repository; this is used for publishing releases
repository = "https://github.com/example/example"
# The executables produced by your app
binaries = ["main"]
# The build command dist runs to produce those binaries
build-command = ["make"]
```

All you need to run to build this program is `make`, so we specified `build-command = ["make"]`. If your app has a more complex build that will require multiple commands to run, it may be easier for you to add a build script to your repository. In that case, `build-command` can simply be a reference to executing it:

```toml
build-command = ["./build.sh"]
```

We expose a special environment variable called `CARGO_DIST_TARGET` into your build. It contains a [Rust-style target triple][target-triple] for the platform we expect your build to build for. Depending on the language of the software you're building, you may need to use this to set appropriate cross-compilation flags. For example, when dist is building for an Apple Silicon Mac, we'll set `aarch64-apple-darwin` in order to allow your build to know when it should build for aarch64 even if the host is x86_64.

On macOS, we expose several additional environment variables to help your buildsystem find dependencies. In the future, we may add more environment variables on all platforms.

* `CFLAGS`/`CPPFLAGS`: Flags used by the C preprocessor and C compiler while building.
* `LDFLAGS`: Flags used by the C linker.
* `PKG_CONFIG_PATH`/`PKG_CONFIG_LIBDIR`: Paths for `pkg-config` to help it locate packages.
* `CMAKE_INCLUDE_PATH`/`CMAKE_LIBRARY_PATH`: Paths for `cmake` to help it locate packages' configuration files.

[cargo-toml]: https://doc.rust-lang.org/cargo/reference/manifest.html
[quickstart-everyone-else]: ./quickstart/everyone-else.md
[spdx]: https://spdx.org/licenses
[target-triple]: https://doc.rust-lang.org/nightly/rustc/platform-support.html
[toml]: https://en.wikipedia.org/wiki/TOML
