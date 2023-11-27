# Generic Builds

> since 0.5.0

Although cargo-dist was originally designed specifically for Cargo-based builds, we've introduced experimental features to allow you to use it to build and distribute any software written in any language. This feature is currently a *prerelease*: details may change before it becomes stable.

## Examples

* [example npm project](https://github.com/axodotdev/axolotlsay-js)
* [example C project](https://github.com/axodotdev/cargo-dist-c-example)

## Configuration

In order for cargo-dist to recognize your application, it requires a [TOML][toml] configuration file named `dist.toml`. This file is similar to Cargo's [`Cargo.toml`][cargo-toml], so users who are already familiar with Cargo should feel comfortable right away. Many of `dist.toml`'s fields are identical to `Cargo.toml`, but there are a few extra fields specific to this file.

`dist.toml` has two mandatory sections: `package`, which you write yourself and which contains information about your application; and `dist`, which contains cargo-dist's configuration and which `cargo-dist init` generates for you.

To get started, write a `dist.toml` containing just a `package` section. A simple one looks like this:

```toml
[package]
# Your app's name
name = "my_app"
# The current version; make sure to keep this up to date!
version = "0.1.0"
# The URL to the git repository; this is used
repository = "https://example.com"
# The executables your app produces
binaries = ["main"]
# The build command cargo-dist runs to produce those binaries
build-command = ["make"]
```

## Quickstart

Once you've produced a configuration file, you can run `cargo dist init` and let cargo-dist generate its own configuration. From here, the build and usage process looks very much like the normal cargo-dist setup; for more information, check the [main quickstart documentation][quickstart].

### Understanding build commands

Build commands are the core difference between these builds and regular cargo-dist. Since we don't have Cargo to rely on to tell us how to build your package, it's up to you to tell us how instead.

As an example, the above application is a C program with a simple makefile-based buildsystem. All you need to run to build this program is `make`, so we specified `build-command = ["make"]`. If your app has a more complex build that will require multiple commands to run, it may be easier for you to add a build script to your repository. In that case, `build-command` can simply be a reference to executing it:

```toml
build-command = ["./build.sh"]
```

We expose a special environment variable called `CARGO_DIST_TARGET` into your build. It contains a [Rust-style target triple][target-triple] for the platform we expect your build to build for. Depending on the language of the software you're building, you may need to use this to set appropriate cross-compilation flags. For example, when cargo-dist is building for an Apple Silicon Mac, we'll set `aarch64-apple-darwin` in order to allow your build to know when it should build for aarch64 even if the host is x86_64.

On macOS, we expose several additional environment variables to help your buildsystem find dependencies. In the future, we may add more environment variables on all platforms.

* `CFLAGS`/`CPPFLAGS`: Flags used by the C preprocessor and C compiler while building.
* `LDFLAGS`: Flags used by the C linker.
* `PKG_CONFIG_PATH`/`PKG_CONFIG_LIBDIR`: Paths for `pkg-config` to help it locate packages.
* `CMAKE_INCLUDE_PATH`/`CMAKE_LIBRARY_PATH`: Paths for `cmake` to help it locate packages' configuration files.

### Mandatory package fields

These package fields are mandatory for cargo-dist to be able to build your package:

* `name`: Your application's name.
* `version`: The application's version. Currently, this must be in a [Semver](https://semver.org)-compatible format.
* `repository`: The URL to a git repository containing your application's source code.
* `binaries`: An array of one or more executables your application's build will produce. The strings within this array are paths relative to your application's build directory; for example, if you produce a binary named `main` within the `./src` directory, you can specify `["src/main"]`.
* `build-command`: The command cargo-dist should run in order to build your application. This is an array of one or more strings; the first string is the command cargo-dist will run, and any subsequent strings are arguments to pass to that command.

### Optional package fields

All of these fields and their definitions are identical to the ones defined by [`Cargo.lock`][cargo-lock].

* `cstaticlibs`: An array of one or more C static libraries (`.a` files) produced by your application's build.
* `cdynamiclibs`: An array of one or more C dynamic libraries produced by your application's build.
* `changelog`: The path to the application's changelog within its source code. This will be used for the text of release announcements.
* `documentation`: The URL to where the application's documentation can be accessed.
* `description`: A human-readable description of the application.
* `readme`: The path to the application's README within its source code.
* `authors`: An array containing the names of the application's developers.
* `license`: The application's license, as an [SPDX identifier][spdx].
* `license-files`: An array containing a list of one or more license files within the source code.

[cargo-toml]: https://doc.rust-lang.org/cargo/reference/manifest.html
[quickstart]: /book/way-too-quickstart.html
[spdx]: https://spdx.org/licenses
[target-triple]: https://doc.rust-lang.org/nightly/rustc/platform-support.html
[toml]: https://en.wikipedia.org/wiki/TOML
