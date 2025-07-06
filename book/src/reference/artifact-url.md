# Artifact URL

<!-- toc -->

[Fetching installers](../installers/index.md) need to know where to download the [actual binaries](../artifacts/archives.md) from, so dist needs to be aware of a base Artifact URL that it can derive download URLs from when it builds those kinds of installers.

Currently artifact URLs are derived from [hosting providers](./config.md#hosting) and [source hosts](#source-hosts). Both must be well-defined for fetching installers to be enabled. Here is the behaviour of each hosting provider in more excruciating detail:


## github

This host uploads your artifacts to a GitHub Release. Currently this functionality is restricted to [the "github" CI backend](../ci/index.md). Because dist is fully in control of the uploading of your artifacts, it can automatically compute the Artifact URL for you, as:

```text
{repo_url}/releases/download/{tag}
```

Where `repo_url` is [your source host repo_url](#source-hosts), and `tag` is the git tag of the release.

For example, if we want the linux build of axolotlsay 0.1.0, we have:

```
* Cargo.toml "repository": `https://github.com/axodotdev/axolotlsay/`
* git tag: `v0.1.0`
* artifact url: `https://github.com/axodotdev/axolotlsay/releases/download/v0.1.0/`
* download: `https://github.com/axodotdev/axolotlsay/releases/download/v0.1.0/axolotlsay-x86_64-unknown-linux-gnu.tar.gz`
```


### Linking GitHub Latest

dist doesn't use this, but it's good for you to know: GitHub Releases lets you hotlink the files of "the latest release". This is useful for writing your own docs, as you can set them and forget them. dist specifically avoids putting version numbers in artifact names so that these kinds of URLs can be used.

The schema is (LOOK CLOSELY, IT IS NOT THE OBVIOUS SCHEMA, GITHUB DID THIS WEIRD):

```
{repo_url}/releases/latest/download/
```

Example:

```
https://github.com/axodotdev/cargo-dist/releases/latest/download/dist-manifest-schema.json
```


## Other

Future releases [will expose a more general mechanism for specifying artifact download URLs](https://github.com/axodotdev/cargo-dist/issues/236).


## Source Hosts

Regardless of what [hosting providers](./config.md#hosting) you ask for, dist will complain if you don't have a properly defined source host, which is a fancy way of saying we need a URL to your git repo. Currently the only supported Source Host is `github.com`, but we [would like to support more](https://github.com/axodotdev/cargo-dist/issues/48).

Most Cargo projects already set a Source Host: it's just [your `[package].repository` URL](./config.md#repository).

dist will parse this value and produce 3 values for your source host: owner, project, and repo_url. Here's an example:

```
* Cargo.toml "repository": https://github.com/axodotdev/axolotlsay.git
* owner: axodotdev
* project: axolotlsay
* repo_url: https://github.com/axodotdev/axolotlsay/
```

(Note that in the above example the `repo_url` is not the verbatim `repository`; we support various common variations and will normalize them away for you!)

**For safety reasons, dist will refuse to accept a Source Host unless all packages in your workspace that define `repository` can be parsed to the exact same Source Host. Having inconsistent/outdated repository URLs is a very common issue. This check *does not* respect publish=false or dist=false!**
