# axoupdater

axoupdater provides an autoupdater program designed for use with [cargo-dist](https://opensource.axo.dev/cargo-dist/). It can be used either as a standalone program, or as a library within your own program. It supports releases hosted on either [GitHub Releases](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases) or [Axo Releases (in beta)](https://axo.dev).

In order to be able to check information about an installed program, it uses the install receipts produced by cargo-dist since version [0.10.0 or later](https://github.com/axodotdev/cargo-dist/releases/tag/v0.10.0). These install receipts are JSON files containing metadata about the currently-installed version of an app and the version of cargo-dist that produced it; they can be found in `~/.config/APP_NAME` (Linux, Mac) or `%LOCALAPPDATA%\APP_NAME` (Windows).

## Standalone use

When built as a standalone commandline app, axoupdater does exactly one thing: check if the user is using the latest version of the software it's built for, and perform an update if not. Rather than being hardcoded for a specific application, the updater's filename is used to determine what app to update. For example, if axoupdater is installed under the filename `axolotlsay-update`, then it will try to fetch updates for the app named `axolotlsay`. This means you only need to build axoupdater once, and can deploy it for many apps without rebuilding.

In an upcoming release, cargo-dist will support generating and installing the updater for your users as an optional feature.

## Library use

axoupdater can also be used as a library within your own applications in order to let you check for updates or perform an automatic update within your own apps. Here's a few examples of how that can be used.

To check for updates and notify the user:

```rust
if AxoUpdater::new_for("axolotlsay").load_receipt()?.is_update_needed_sync()? {
    eprintln!("axolotlsay is outdated; please upgrade!");
}
```

To automatically perform an update if the program isn't up to date:

```rust
if AxoUpdater::new_for("axolotlsay").load_receipt()?.run_sync()? {
    eprintln!("Update installed!");
} else {
    eprintln!("axolotlsay already up to date");
}
```

To use the blocking versions of the methods, make sure to enable the `"blocking"` feature on this dependency in your `Cargo.toml`. Asynchronous versions of `is_update_needed()` and `run()` are also provided:

```rust
if AxoUpdater::new_for("axolotlsay").load_receipt()?.run().await? {
    eprintln!("Update installed!");
} else {
    eprintln!("axolotlsay already up to date");
}
```

## GitHub Actions and Rate Limits in CI

By default, axoupdater uses unauthenticated GitHub API calls when fetching release information. This is reliable in normal use, but it's much more likely to run into rate limits in the highly artificial environment of a CI test. Axoupdater provides a way to supply a GitHub API token in order to opt into a higher rate limit; if you find your app being rate limited in CI, you may want to opt into it. Cargo-dist uses this in its own tests. Here's a simple example of how you can integrate it into your own app.

We recommend using an environment variable for token configuration so that you don't have to adjust how you call your app at the commandline in tests. We also recommend picking an environment variable name that's specific to your application; it's not uncommon for users to have stale or expired `GITHUB_TOKEN` tokens in their environment, and using that name may cause your app to behave unexpectedly.

First, wherever you construct an updater client, add a check for the environment variable and, if set, pass its value to the `set_github_token()` method:

```rust
if let Ok(token) = std::env::var("YOUR_APP_GITHUB_TOKEN") {
    updater.set_github_token(&token);
}
```

A sample of how cargo-dist uses this can be found [here](https://github.com/axodotdev/cargo-dist/blob/80f2e19e5aa79b7b1f64beb62ceb07aa71566707/cargo-dist/src/main.rs#L599-L601).

Then, in your CI configuration, assign that variable to the value of the `GITHUB_TOKEN` secret that's automatically assigned by GitHub Actions:

```yaml
env:
  YOUR_APP_GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

A sample in cargo-dist's CI configuration can be found [here](https://github.com/axodotdev/cargo-dist/blob/80f2e19e5aa79b7b1f64beb62ceb07aa71566707/.github/workflows/ci.yml#L82-L85).

## Crate features

By default, axoupdater is built with support for both GitHub and Axo releases. If you're using it as a library in your program, and you know ahead of time which backend you're using to host your release assets, you can disable the other library in order to reduce the size of the dependency tree.

## Building

To build as a standalone binary, follow these steps:

- Run `cargo build --release`
- Rename `target/release/axoupdater` to `APPNAME-update`, where `APPNAME` is the name of the app you want it to upgrade.

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or [apache.org/licenses/LICENSE-2.0](https://www.apache.org/licenses/LICENSE-2.0))
- MIT license ([LICENSE-MIT](LICENSE-MIT) or [opensource.org/licenses/MIT](https://opensource.org/licenses/MIT))

at your option.
