# cargo-dist-schema

[![crates.io](https://img.shields.io/crates/v/cargo-dist-schema.svg)](https://crates.io/crates/cargo-dist) [![docs](https://docs.rs/cargo-dist-schema/badge.svg)](https://docs.rs/cargo-dist-schema)
![Rust CI](https://github.com/axodotdev/cargo-dist/workflows/Rust%20CI/badge.svg?branch=main)

Schema reporting/parsing for cargo-dist's `dist-manifest.json`, which is the result you get from `--output-format=json` when running `cargo dist build` or `cargo dist plan`.

[Read the schema and docs here!][https://axodotdev.github.io/cargo-dist/book/schema.html]

This can be used to parse the machine-readable manifests produced by cargo-dist. Ideally it should be forward and backward compatible with newer and older versions of the manifests.

This compatibility is fairly important as one tool may need to look at releases spread over *years*. Also cargo-dist is self-hosting from previous releases, so when looking at cargo-dist's own releases there will always be (at least) an off-by-one in the manifest and the tool that manifest describes.

There are currently two epochs to dist-manifest.json:

* epoch 1: <= 0.0.2
* epoch 2: >= 0.0.3-prerelease10

Epoch 1 was initial experimentation, and Epoch 2 made some breaking changes once we had a better sense for the constraints of the design. Most notable artifacts were pull into a top-level Object that Releases simply refer to by key. This makes it possible for different releases to share an Artifact (such as debuginfo/symbol files for shared binaries).

(The gap between the two epochs is a fuzzy zone with inadvisable-to-use prerelease copies of cargo-dist. 0.0.3-prerelease10 is where things got solidified and should be the same as 0.0.3 proper.)

All tooling from Epoch 2 only supports Epoch 2.



## A Brief Aside On Self-Hosting/Bootstrapping

cargo-dist's CI is self-hosting using previous releases of itself. Ostensibly there's a coherent bootstrapping chain of releases, but around the Epoch boundary things get a bit wonky. You can always build from source with just `cargo build` so it's not exactly a big deal.

But for my own edification:

* [v0.0.1-prerelease1](https://github.com/axodotdev/cargo-dist/releases/tag/v0.0.1-prerelease1) was the first **unpublished** version, built with a temporary copy of itself
* [v0.0.1-prerelease2](https://github.com/axodotdev/cargo-dist/releases/tag/v0.0.1-prerelease2) will be the first **published** version, built with 0.0.1-prerelease1
* [v0.0.1](https://github.com/axodotdev/cargo-dist/releases/tag/v0.0.1) will be the first version built from another published version

The awkward thing is that if we want the release for cargo-dist to include a feature new to itself, we need intermediate prereleases for the feature to "catch up". As such stable releases are basically never built from the previous stable release.