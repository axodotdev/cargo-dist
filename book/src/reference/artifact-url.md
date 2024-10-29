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


## axodotdev

It host uploads your artifacts to an axo Release. This functionality can be used with any CI backend. A [source host](#source-hosts) must be defined that matches your axo Releases account. Because axo Releases and dist were built to work together and enable More Advanced Features, the process of getting an Artifact URL out of this hosting provider is more complex.

[Most of the details are handled by gazenot, the axo Releases client. See its docs for way too many details.](https://github.com/axodotdev/gazenot#gazenot)

The TL;DR is that axo Releases wants to randomly generate an Artifact URL for us, and since we need to bake those URLs into the artifacts we build (installers that fetch binaries), we need to get that URL as soon as possible (in the "plan" step).

Getting this URL is a networked side-effect that requires an authentication token, so when you locally run `dist build` or `dist plan` we will just use a mock URL.

The command that gets A Real Artifact URL from axo Releases is `dist host --steps=create`. This is conceptually identical to `dist plan`, but indicates "yes I am ok with doing side-effectful networking to get an Artifact URL to upload things to". This command will write a `dist-manifest.json` to your output directory that subsequent commands like `dist build --artifacts=global` will read back in to get the currently active artifact url. If you do this locally, you will need to use `cargo clean` to make dist forget the URL.

This random URL ("Set URL") will work forever and will get baked into various outputs. However, it's not the URL you want to show end-users when telling them to install your software. Once released, you will also have access to the more end-user-friendly "Release URL" and "Latest URL":

* A **Set URL** (`https://myuser.artifacts.axodotdev.host/myapp/ax_UJl_tKCujZwxKL1n_K7TM`) is the permanent randomly
  generated URL for downloading files. It will be embedded in things like the bodies of things like `my-app-installer.sh`,
  but ideally it **should never** be presented to end-users in things like `curl | sh https://...` expressions.
* A **Release URL** (`https://myuser.artifacts.axodotdev.host/myapp/v1.0.0/`) is a permanent stable-format URL for
  downloading files from a Release('s ArtifactSet). This is typically what should be presented in curl-sh expressions.
  This URL may go dead if a Release is never Announced.
* A **Latest (Release) URL** (`https://myuser.artifacts.axodotdev.host/myapp/latest/`) is a mutable-destination
  stable-format URL for downloading "whatever the latest Release('s ArtifactSet) is". This URL is appropriate for
  linking in random docs which you don't want to update every time there's a release.

The default schemas for these URLs are:

```
* set: https://{owner}.artifacts.axodotdev.host/{project}/{RANDOM_ID}/
* release: https://{owner}.artifacts.axodotdev.host/{project}/{tag}/
* latest: https://{owner}.artifacts.axodotdev.host/{project}/latest/
```

Where `owner` and `project` are [your source host](#source-hosts), and `tag` is the git tag of the release.

[See gazenot's docs for what this extra complexity potentially enables](https://github.com/axodotdev/gazenot#gazenot).


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
