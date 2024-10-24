# Workspace Structure

All dist projects, regardless of how many packages they may contain, are
considered workspaces. A minimalist dist project would be a workspace that
contains a single package.

This guide walks through the structure of a dist workspace, but if you're more interested in seeing one in action, you can skip ahead to the [simple workspace guide][simple-guide].

Dist has two types of configuration:

- **Workspace**: Configuration that should apply to any projects in a single repo
- **Package**: Configuration that should apply to one specific project

The vast majority of configuration for dist is workspace configuration. Because
we understand language-specific config manifests like `Cargo.toml` and
`package.json`, package configuration ususally does not require any additional
files. Dedicated package configuration, in `dist.toml` files, is typically only needed when using dist's ["custom builds"][custom-builds] mode.

## Workspace configuration

A workspace file may be:

* `dist-workspace.toml`
* (deprecated in 0.23.0) `Cargo.toml` (Rust users only, goes in `[workspace.metadata.dist]`)

### Specifying The Members Of Your Workspace

Create a `workspace.members` field in your `dist-workspace.toml` that points to
an array of strings. Each string is prefaced with a type:

- `npm`: this indicates a package that is specified by a `package.json`
- `cargo`: this indicates a package (or nested workspace) that is specified by a `Cargo.toml`. You do not need to specify cargo workspace members individually, you can simply refer to the workspace.
- `dist`: this indicates a package that is specified by a `dist.toml`

For example:

```toml
[workspace]
members = [
  "npm:path/to/npm/packagejson/dir/",
  "cargo:path/to/workspace/cargotoml/dir/",
  "dist:path/to/distoml/dir/"
]
```

## Package configuration

A package file may be:

* `dist.toml`
* `dist-workspace.toml`
* `Cargo.toml` (for a Rust package)
* `package.json` (for an npm package)

In the case of a `Cargo.toml` and `package.json`, we'll do our best to find basic package
info like package name, version, repository, binaries among the native language-specific config.

However these files do not natively support dist-specific config, so you may
need to place a dist config *next* to them to specify additional dist-specific
config. If we see the language-specific package file, we will automatically
check for a neighbouring dist config and merge its contents in. If you wish to
"shadow" or change a value that is present in your package manifest -- you can
use the dist config file to "override" it.

For a `Cargo.toml` you can instead use `[package.metadata.dist]`. However to
override values in the `[package]` field, you would need to create a dist
config file..


## Specifying The Members Of Your Workspace

If you have a pure Cargo project, and are using `[workspace.metadata.dist]`, you don't
need to specify project members at all -- we'll just find all the packages with our
native understanding of Cargo.

For everyone else, your dist-workspace.toml will need to contain a `workspace.members` field enumerating paths to all your packages -- although again for a Cargo member you
don't actually enumerate the packages, you just point to the Cargo workspace
and we'll find all the packages in that workspace:

```toml
[workspace]
members = [
  "npm:path/to/npm/packagejson/dir/",
  "cargo:path/to/workspace/cargotoml/dir/",
  "dist:path/to/distoml/dir/"
]
```

## Which Packages Are Distable

When you ask dist to release a version of your workspace (by specifying a
version, either with a tag or via workflow dispatch), we will release all "distable" packages with the
given version.

The set of distable packages isn't just "all the packages in your dist workspace"
because we support natively importing entire language-specific workspaces, which may
include tons of libraries you aren't interested in, or example/test applications.

By default we assume a package is distable, and then run through a set of criteria
to try to disqualify it:

* If the package is "empty", it's not distable
	* By default, we check for whether the package defines binaries
    * If you have enabled `cdylibs/cstaticlibs` we check for those as well
* If the package has `dist=false` set, it's not distable
  * For a cargo project, If dist isn't specified, the `publish` field in
    `Cargo.toml` will be inherited, with a default value of `true`. Setting
    `dist=true` can therefore be used to ignore `publish=false` in `Cargo.toml`.

[custom-builds]: ../custom-builds.md
[simple-guide]: ./simple-guide.md
