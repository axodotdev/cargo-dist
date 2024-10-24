# npm Installer

> since 0.0.6

dist can automatically build and publish [npm](https://www.npmjs.com/) packages for your applications. Users can install your application with an expression like `npm install -g @axodotdev/axolotlsay`, or immediately run it with `npx @axodotdev/axolotlsay`.

The npm package will [fetch][artifact-url] your prebuilt [archives](../artifacts/archives.md) and install your binaries to node_modules, exposing them as commands ("bins") of the package.
If the package [unambiguously has one true command](https://docs.npmjs.com/cli/v7/commands/npx#description), then the package can be run without specifying one.

Note that this is *not* (yet) a feature for publishing an npm package in your workspace. The package described here is generated as part of your release process.

An "installer hint" will be provided that shows how to install via `npm` like so:

```sh
npm install @axodotdev/cargodisttest@0.2.0
```

## Quickstart

To setup your npm installer you need to create an npm access token and enable the installer. This is broken up into parts because a project administrator may need to be involved in part 1, while part 2 can be done by anyone.


### Part 1: Creating an npm account and optional scope and authenticating GitHub Actions

1. Create an account on [npmjs.com](https://www.npmjs.com/signup).
1. (Optionally) If you would like to publish a "scoped" package (aka `@mycorp/pkg`) you'll need to [create an npm organization](https://www.npmjs.com/org/create).
2. Go to your npm account settings and create a granular access token:

    - Expiration: The default is 30 days. You can pick what works for you and your team. (NOTE: If you really want a token that does not expire you can use a Classic Token but we expect that option to eventually be fully deprecated in the near future.)
    - Packages and scopes: Read and write
        - Select packages: All packages (NOTE: because the package does not yet exist, you must pick this. However, you can (and probably should!) update this to scope the token to a single package after publish. This is sadly a limitation of the npm token system.)
    - Organizations: No access

3. Add the token as a [GitHub Actions Secret](https://docs.github.com/en/actions/security-guides/encrypted-secrets) called `NPM_TOKEN` to the repository your are publishing from.


### Part 2: Enabling The npm Installer

1. run `dist init` on your project
2. when prompted to pick installers, enable "npm"
3. this should trigger a prompt for your optional scope (`@axodotdev`)

...that's it! If this worked, your config should now contain the following entries:

```toml
[workspace.metadata.dist]
# "..." indicates other installers you may have selected
installers = ["...", "npm", "..."]
# if you did not provide a scope, this won't be present
npm-scope = "@axodotdev"
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

> since 0.14.0

By default the name of the npm package will be the name of the package that defines it (your Cargo package). If for whatever reason you don't want that to be the case, then you can change it with the [npm-package setting](../reference/config.md#npm-package).

So with these settings:

```toml
[package]
name = "axolotlsay"

[package.metadata.dist]
npm-scope = "@axodotdev"
npm-package = "cli"
```

You'll end up publish the binaries in "axolotlsay" to an npm package called "@axodotdev/cli".


[artifact-url]: ../reference/artifact-url.md
