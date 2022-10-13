# cargo-dist

[![crates.io](https://img.shields.io/crates/v/cargo-dist.svg)](https://crates.io/crates/cargo-dist)
![Rust CI](https://github.com/axodotdev/cargo-dist/workflows/Rust%20CI/badge.svg?branch=main)

`cargo build` but For Building Final Distributable Artifacts.

This may or may not include:

* building your executable with the Good Settings (opt + split debuginfo, strip binary...)
* packaging the executable and assets up for distribution (tar.gz, zip, dmg, ...)
* producing an installer?
* producing standalone debuginfo files? (pdb, dysm, sym, ...)
* prodicing a machine-readable manifest of the generated artifacts?
* code signing?



# Contributing

## Updating Snapshots

To update snapshots, you will need to install and use [cargo-insta](https://crates.io/crates/cargo-insta)

```
> cargo install cargo-insta
...

> cargo insta review
...
```

If you know you like the changes, just use `cargo insta accept` to auto-apply all changes.



## Cutting Releases

Releases are performed with a single invocation of [cargo-release](https://crates.io/crates/cargo-release).


```
> cargo install cargo-release
...

> cargo release X.Y.Z
...
```

**NOTE: to actually perform the release, you must pass --execute, cargo-release defaults to dry-runs because it's very dangerous! The --execute flag is intentionally omitted from the example to avoid accidents.

This will automatically:

* Update the version in the Cargo.toml
* `git commit -m "vX.Y.Z release"` (or something similar)
* `git tag "vX.Y.Z"
* `cargo publish`
* `git push origin --tags`
* TODO: update RELEASES.md to make the current release notes the latest release and start a new "upcoming releases" section

The push of a commit with a tag of this form will in turn trigger github CI actions to create a new Github Release, build artifacts, and upload those artifacts to the release.
