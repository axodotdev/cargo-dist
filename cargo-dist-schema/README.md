# cargo-dist-schema

[![crates.io](https://img.shields.io/crates/v/cargo-dist-schema.svg)](https://crates.io/crates/cargo-dist) [![docs](https://docs.rs/cargo-dist-schema/badge.svg)](https://docs.rs/cargo-dist-schema)
![Rust CI](https://github.com/astral-sh/cargo-dist/workflows/Rust%20CI/badge.svg?branch=main)

Schema reporting/parsing for dist's `dist-manifest.json`, which is the result you get from `--output-format=json` when running `cargo dist build` or `cargo dist plan`.

[Read our documentation here!](https://opensource.axo.dev/cargo-dist/book/)

This can be used to parse the machine-readable manifests produced by dist. Ideally it should be forward and backward compatible with newer and older versions of the manifests.

This compatibility is fairly important as one tool may need to look at releases spread over *years*. Also dist is self-hosting from previous releases, so when looking at dist's own releases there will always be (at least) an off-by-one in the manifest and the tool that manifest describes.

There are currently 3 epochs to dist-manifest.json:

* epoch 1 <= 0.0.2
* 0.0.3-prerelease9 <= epoch2 <= 0.0.6-prerelease.6
* 0.0.3-prerelease.8 <= epoch3

Epoch 1 was initial experimentation, and is no longer supported.

Epoch 2 made some breaking changes once we had a better sense for the constraints of the design. Most notable artifacts were pull into a top-level Object that Releases simply refer to by key. This makes it possible for different releases to share an Artifact (such as debuginfo/symbol files for shared binaries). The version gap between Epoch 1 and 2 is a fuzzy zone with inadvisable-to-use prerelease copies of dist. We believe 0.0.3-prerelease9 is where things got solidified and should be the same as 0.0.3 proper.

Epoch 3 has the exact same format but we removed versions from artifact id names, changing the format of URLs. This largely only affects dist's ability to fetch itself, and created a single transitory release (0.0.6-prerelease.7) which is unable to fetch itself, because it was built with an epoch2 version (0.0.5). This version is intentionally not published on crates.io. The CI was manually updated to use the right URLs to bootstrap 0.0.6-prerelease.8, which is fully in epoch3.

## A Brief Aside On Self-Hosting/Bootstrapping

dist's CI is self-hosting using previous releases of itself. Ostensibly there's a coherent bootstrapping chain of releases, but around the Epoch boundary things get a bit wonky. You can always build from source with just `cargo build` so it's not exactly a big deal.

But for my own edification:

* [v0.0.1-prerelease1](https://github.com/axodotdev/cargo-dist/releases/tag/v0.0.1-prerelease1) was the first **unpublished** version, built with a temporary copy of itself
* [v0.0.1-prerelease2](https://github.com/axodotdev/cargo-dist/releases/tag/v0.0.1-prerelease2) will be the first **published** version, built with 0.0.1-prerelease1
* [v0.0.1](https://github.com/axodotdev/cargo-dist/releases/tag/v0.0.1) will be the first version built from another published version

The awkward thing is that if we want the release for dist to include a feature new to itself, we need intermediate prereleases for the feature to "catch up". As such stable releases are basically never built from the previous stable release.https://github.com/axodotdev/cargo-dist/commit/8a417f239ef8f8e3ab66c46cf7c3d26afaba1c87
