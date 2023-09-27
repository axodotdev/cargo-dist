# npm Installer

> since 0.0.6

This provides a tarball containing an npm package (npm-package.tar.gz) which when installed into an npm project: detects the current platform, fetches the best possible [archive][] from your [artifact URL][artifact-url], and copies the binary into your node_modules. This can be used to install the binaries like any other npm package, or to run them with `npx`.

This kind of installer is ideal for making a native Rust tool available to JS devs.

An "installer hint" will be provided that shows how to install via `npm` like so:

```sh
npm install @axodotdev/cargodisttest@0.2.0
```

**cargo-dist does not publish the package for you, you need to do that manually once the tarball is built.** Conveniently, npm supports publishing from a url-to-a-tarball directly, and since 0.0.7 we make our tarballs look like "proper" npm package tarballs, so you can just do this:

```sh
npm publish URL_TO_TARBALL
```

You can find the URL to the tarball at the bottom of the Github Release, inside the collapsible "assets" dropdown (*-npm-package.tar.gz). The format of the url is:

```text
<repo>/releases/download/<tag>/<app-name>-npm-package.tar.gz
```

Example:

https://github.com/axodotdev/oranda/releases/download/v0.0.3/oranda-npm-package.tar.gz

If you're cutting a stable release (not-prerelease), you can use the "latest" URL format:

https://github.com/axodotdev/oranda/releases/latest/download/oranda-npm-package.tar.gz

In the future we may [introduce more streamlined CI-based publishing workflows][issue-npm-ci].

[You can set the @scope the package is published under with the npm-scope cargo-dist config][config-npm-scope].

We will otherwise do our best to faithfully translate [any standard Cargo.toml values you set][cargo-manifest] to an equivalent in the npm package.json format (name, version, authors, description, homepage, repository, keywords, categories...).

The package will also include an npm-shrinkwrap.json file for the npm packages the installer uses, this is the same as package-lock.json but "really for reals I want this to be respected even if it's installed into another project". Note that [cargo install similarly disrespects Cargo.lock unless you pass --locked][install-locked].




## Limitations and Caveats

* Requires a well-defined [artifact URL][artifact-url]
* [Cannot detect situations where musl-based builds are appropriate][issue-musl] (static or dynamic)
* [Relies on nodejs's builtin gzip support to unpack the files, which only works with .tar.gz][issue-unpacking]
* Cannot run any kind of custom install logic

As a result of the `.tar.gz` limitation, `cargo dist init` will prompt you to change [windows-archive][config-windows-archive] and [unix-archive][config-unix-archive] to ".tar.gz" if you enable the npm installer, erroring if you decline.




[issue-npm-ci]: https://github.com/axodotdev/cargo-dist/issues/245
[issue-musl]: https://github.com/axodotdev/cargo-dist/issues/75
[issue-unpacking]: https://github.com/axodotdev/cargo-dist/issues/226

[config-windows-archive]: ../reference/config.md#windows-archive
[config-unix-archive]: ../reference/config.md#unix-archive
[config-npm-scope]: ../reference/config.md#npm-scope

[archive]: ../artifacts/archives.md
[artifact-url]: ../reference/artifact-url.md

[cargo-manifest]: https://doc.rust-lang.org/cargo/reference/manifest.html
[install-locked]: https://doc.rust-lang.org/cargo/commands/
