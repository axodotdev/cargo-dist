# axocli

[![crates.io](https://img.shields.io/crates/v/axocli.svg)](https://crates.io/crates/axocli)
[![docs](https://docs.rs/axocli/badge.svg)](https://docs.rs/axocli)
[![Rust CI](https://github.com/axodotdev/axocli/workflows/Rust%20CI/badge.svg?branch=main)](https://github.com/axodotdev/axocli/actions/workflows/ci.yml)

Common code for setting up a CLI App and handling errors/printing.


## Example

See [examples/axoapp.rs](https://github.com/axodotdev/axocli/blob/main/examples/axoapp.rs) for a walkthrough/example.

Some various interesting example invocations to play with: 

```sh
# clap help
cargo run --example axoapp -- --help

# success
cargo run --example axoapp -- 5
cargo run --example axoapp -- 5 --output-format=json

# normal error
cargo run --example axoapp -- 2
cargo run --example axoapp -- 2 --output-format=json

# panic (setting RUST_BACKTRACE=1 here is also interesting)
cargo run --example axoapp -- 0
cargo run --example axoapp -- 0 --output-format=json

# clap error
cargo run --example axoapp --
```


## What Does It Do?

First off, it handles setting up:

* a tracing subscriber (currently the kind that cargo-dist uses, more work needed for oranda's use)
* a miette formatter (so we can have a shared Look And Feel)
* a panic handler (to get more control over output if the app panics)
* (not implemented but likely in the future) a tokio runtime

It also handles printing top-level errors, notably including a json_errors mode where the error gets formatted to json and printed to stdout, while still printing a human-friendly version to stderr. This is done for both an error returned from real_main and panics. So anything expecting machine-readable output from our apps will not freak out and get something unparseable when things error/panic. It will also set the process exit code on error (with std::process::exit, on the assumption that all cleanup was done when we returned/panicked out of real_main).

It also exposes the json diagnostic formatting machinery so you can Write them wherever or turn them into serde_json::Values. This is useful for
returning a larger result with diagnostics nested inside of it (say, for reporting warnings).


## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or [apache.org/licenses/LICENSE-2.0](https://www.apache.org/licenses/LICENSE-2.0))
* MIT license ([LICENSE-MIT](LICENSE-MIT) or [opensource.org/licenses/MIT](https://opensource.org/licenses/MIT))

at your option.
