# Workspace Structure

All dist projects, regardless of how many packages they may contain, are
considered workspaces. A minimalist dist project would be a workspace that
contains a single package.

Dist has two types of configuration:

- **Workspace**:
- **Package**: 

The vast majority of configuration for dist is workspace configuration. Because
we understand language-specific config manifests like `Cargo.toml` and
`package.json`, package configuration ususally does not require any additional
files.

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
"shadow" or change a value that is present in your package manifest- you can
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

When you ask cargo-dist to release a version of your workspace (by specifying
a git tag containing the version), we will release all "distable" packages with the
given version.

(editor's note: we could alternatively structure this as 'all "non-empty" and "distable"
packages', but coloquially having a single word that covers both has been useful...)

The set of distable packages isn't just "all the packages in your dist workspace"
because we support natively importing entire language-specific workspaces, which may
include tons of libraries you aren't interested in, or example/test applications.

By default we assume a package is distable, and then run through a set of criteria
to try to disqualify it:

* if the package is "empty" it's not distable
	* by default we check for whether the package defines binaries but enabling 
	  "package cdylibs/cstaticlibs" makes us also check for those
* if the package has `dist=false` set, it's not distable
  * if dist isn't specified, Cargo's `publish` field will be inherited, with a
    default value of "true". Setting dist=true can therefore be used to ignore
    the Cargo `publish=false` setting.
    
Note that this means that dist=true is *not* sufficient to make a package distable,
as empty packages are still ignored (maybe that should be an error though, because
there's no good reason to specify dist=true globally?). (Similarly it should probably
be an error to import a workspace/package where all packages aren't distable...
except see the next paragraph.)

If you instead try to release a *specific package* (by specifying a git tag that
refers to a package in the workspace), distability will be completely
ignored. If the package is empty this can put cargo-dist in a special mode where it
doesn't do builds and just e.g. makes a GitHub Release for the library.
The only mechanism to avoid this (if you want to tag packages and not have cargo-dist
care) is to use the tag-namespace feature.
(extra corner case: in the "c project with one package" case you don't need workspace.members either)
