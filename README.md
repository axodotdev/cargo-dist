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

