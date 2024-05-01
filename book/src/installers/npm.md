# npm Installer

> since 0.0.6

This provides a tarball containing an npm package (npm-package.tar.gz) which when installed into an npm project: detects the current platform, fetches the best possible [archive][] from your [artifact URL][artifact-url], and copies the binary into your node_modules. This can be used to install the binaries like any other npm package, or to run them with `npx`.

This kind of installer is ideal for making a native Rust tool available to JS devs.

An "installer hint" will be provided that shows how to install via `npm` like so:

```sh
npm install @axodotdev/cargodisttest@0.2.0
```

## Quickstart

To setup your homebrew installer you need to create a custom tap and enable the installer. This is broken up into parts because a project administrator may need to be involved in part 1, while part 2 can be done by anyone.


### Part 1: Creating an npm account and optional scope and authenticating GitHub Actions

1. Create an account on [npmjs.com](https://www.npmjs.com/signup).
1. (Optionally) If you would like to publish a "scoped" package (aka `@mycorp/pkg`) you'll need to [create an npm organization](https://www.npmjs.com/org/create).
1. (Optionally) If you'd like, you can also update your current user to an org so you can publish packages like `@myuser/pkg`.
2. Create an npm granular access token:

    - Expiration: The default is 30 days. You can pick what works for you and your team. (NOTE: If you really want a token that does not expire you can use a Classic Token but we expect that option to eventually be fully deprecated in the near future.)
    - Packages and scopes: Read and write
        - Select packages: All packages (NOTE: because the package does not yet exist, you must pick this. However, you can (and probably should!) update this to scope the token to a single package after publish. This is sadly a limitation of the npm token system.)
    - Organizations: No access

3. Add the token as a [GitHub Actions Secret](https://docs.github.com/en/actions/security-guides/encrypted-secrets) called `NPM_TOKEN` to the repository your are publishing from.

### Part 2: Enabling The npm Installer

1. run `cargo dist init` on your project
2. when prompted to pick installers, enable "npm"
3. this should trigger a prompt for your optional scope (`@axodotdev`)

...that's it! If this worked, your config should now contain the following entries:

```toml
[workspace.metadata.dist]
# "..." indicates other installers you may have selected
installers = ["...", "npm", "..."]
# if you did not provide a scope, this won't be present
scope = "@axodotdev"
publish-jobs = ["npm"]
```

Next make sure that `description` and `homepage` are set in your Cargo.toml. These
fields are optional but make for better npm packages.

```toml
[package]
description = "a CLI for learning to distribute CLIs in rust"
homepage = "https://github.com/axodotdev/axolotlsay"
```

## Renaming npm packages

> coming soon [cargo-dist#983](https://github.com/axodotdev/cargo-dist/issues/983)

## Limitations and Caveats

* [Cannot detect situations where musl-based builds are appropriate][issue-musl] (static or dynamic)
* [Relies on nodejs's builtin gzip support to unpack the files, which only works with .tar.gz][issue-unpacking]

As a result of the `.tar.gz` limitation, `cargo dist init` will prompt you to change [windows-archive][config-windows-archive] and [unix-archive][config-unix-archive] to ".tar.gz" if you enable the npm installer, erroring if you decline.

[issue-musl]: https://github.com/axodotdev/cargo-dist/issues/75
[issue-unpacking]: https://github.com/axodotdev/cargo-dist/issues/226
