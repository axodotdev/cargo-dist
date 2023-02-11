# cargo-dist-schema

[![crates.io](https://img.shields.io/crates/v/cargo-dist-schema.svg)](https://crates.io/crates/cargo-dist) [![docs](https://docs.rs/cargo-dist-schema/badge.svg)](https://docs.rs/cargo-dist-schema)
![Rust CI](https://github.com/axodotdev/cargo-dist/workflows/Rust%20CI/badge.svg?branch=main)

Schema reporting/parsing for cargo-dist's `dist-manifest.json`.

This can be used to parse the machine-readable manifests produced by cargo-dist. Ideally it should be forward and backward compatible with newer and older versions of the manifests.

This compatibility is fairly important as one tool may need to look at releases spread over *years*. Also cargo-dist is self-hosting from previous releases, so when looking at cargo-dist's own releases there will always be (at least) an off-by-one in the manifest and the tool that manifest describes.

The bootstrapping chain officially starts at:

* [v0.0.1-prerelease1](https://github.com/axodotdev/cargo-dist/releases/tag/v0.0.1-prerelease1) was the first **unpublished** version, built with a temporary copy of itself
* [v0.0.1-prerelease2](https://github.com/axodotdev/cargo-dist/releases/tag/v0.0.1-prerelease2) will be the first **published** version, built with 0.0.1-prerelease1
* [v0.0.1](https://github.com/axodotdev/cargo-dist/releases/tag/v0.0.1) will be the first version built from another published version

From there the bootstrap chain should ideally just follow published versions.