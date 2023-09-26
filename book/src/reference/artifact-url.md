# Artifact URL

[Fetching installers](../installers/index.md) need to know where to download the [actual binaries](../artifacts/archives.md) from, so cargo-dist needs to be aware of a base Artifact URL that it can derive download URLs from when it builds those kinds of installers.

## GitHub

Currently this functionality is restricted to [the "github" ci backend](../ci/github.md), which uploads your artifacts to a GitHub Release. Because cargo-dist is fully in control of the uploading of your artifacts, it can automatically compute the Artifact URL for you, as:

```text
{repo_url}/releases/download/{tag}
```

Where `repo_url` is the value of `repository` set in your Cargo.toml, and `tag` is the git tag of the release. For safety reasons, cargo-dist will refuse to define repo_url (and therefore the Artifact URL) unless all packages in your workspace that define `repository` agree on the value and have the format of `https://github.com/{owner}/{project}` (although we'll do some cleanups like trailing slashes or `.git`).

For example, if we want the linux build of axolotlsay 0.1.0, we have:

```
* Cargo.toml "repository": `https://github.com/axodotdev/axolotlsay/`
* git tag: `v0.1.0`
* artifact url: `https://github.com/axodotdev/axolotlsay/releases/download/v0.1.0/`
* download: `https://github.com/axodotdev/axolotlsay/releases/download/v0.1.0/axolotlsay-x86_64-unknown-linux-gnu.tar.gz`
```


## Other

Future releases [will expose a more general mechanism for specifying artifact download URLs](https://github.com/axodotdev/cargo-dist/issues/236).