# Version 0.9.0 (2024-12-19)

This release adds support for `XDG_CONFIG_HOME` as the location for install
receipts. If this variable is set and the receipt is located within this path,
it overrides the default location of `$HOME/.config` (Mac and Linux) or
`%LOCALAPPDATA%` (Windows). Install receipts will be created in this path when
running installers created by dist 0.27.0 or later if `XDG_CONFIG_HOME` is set.

This release also adds infrastructure to support app renaming when running as
a library. There are two new features:

* It's now possible to load receipts for alternate app names, not just the one
  that a given instance of `AxoUpdater` was instantiated for. This can be done
  by running `AxoUpdater::load_receipt_for(app_name)`.
* It's now possible to change the name a given `AxoUpdater` instance is for.
  This can be done by running `AxoUpdater::set_name(app_name)`. This can
  override the name that was loaded from an app receipt.

For example, if your app is changing from `oldname` to `newname`, you might set
up `AxoUpdater` like this:

```rust
// Instantiate the updater class with the new app name
let mut updater = AxoUpdater::new_for("newname");

// First, try to check for a "newname" receipt
// (this might be a post-rename release)
if updater.load_receipt_as("newname").is_err() {
    // If that didn't work, try again as "oldname"
    if updater
        .load_receipt_as("oldname")
        .map(|updater| updater.set_name("newname"))
        .is_err()
    {
        eprintln!("Unable to load install receipt!");
    }
}
```

# Version 0.8.2 (2024-12-03)

This release adds `x86_64-pc-windows-gnu` to the list of targets for which we
publish binaries. It also contains a few small changes to the library:

* The new `AxoUpdater::VERSION` constant exposes axoupdater's version.
* The `AxoUpdater::install_prefix_root` method is now public.

# Version 0.8.1 (2024-10-31)

This release fixes an issue with the previous release in which
`{app_name}_INSTALLER_GITHUB_BASE_URL` wouldn't respect the port specified by
the user.

# Version 0.8.0 (2024-10-31)

This release adds support for overriding the GitHub API URL using new environment variables:

* `{app_name}_INSTALLER_GITHUB_BASE_URL`
* `{app_name}_INSTALLER_GHE_BASE_URL`

For more information, see the [dist installer docs](https://opensource.axo.dev/cargo-dist/book/installers/usage.html#artifact-location).

- impl
  - @gaborbernat [Allow changing the GitHub API base URL via the INSTALLER_DOWNLOAD_URL env var](https://github.com/axodotdev/axoupdater/pull/199)
  - @mistydemeo [feat: use new custom env vars](https://github.com/axodotdev/axoupdater/pull/201)

# Version 0.7.3 (2024-10-22)

This release contains improvements on Windows, ensuring that temporary files and
files from older versions are correctly cleaned up.

# Version 0.7.2 (2024-09-11)

This release fixes a bug that caused axoupdater to return a confusing error
message if it attempted to load an install receipt containing a reference to an
install path which no longer exists.

# Version 0.7.1 (2024-08-28)

This release improves compatibility with certain Windows configurations by setting the execution policy before running the new installer. A similar change is shipped in cargo-dist 0.21.2.

This release also contains a forward-looking change to ensure compatibility with installers produced by future versions of cargo-dist ([#169](https://github.com/axodotdev/axoupdater/pull/169)).

# Version 0.7.0 (2024-07-25)

This release improves debugging for users who use axoupdater as a crate and who
disable printing stdout/stderr from the installer. If the installer runs but
fails, we now return a new error type which contains the stderr/stdout and exit
status from the underlying installer; this can be used by callers to help
identify what failed.

This release also introduces a debugging feature for the standalone installer.
It's now possible to override which installer to use by setting the
`AXOUPDATER_INSTALLER_PATH` environment variable to the path on disk of the
installer to use. A similar feature was already available to library users
using the `AxoUpdater::configure_installer_path` method.


# Version 0.6.9 (2024-07-18)

This release fixes a bug in which axoupdater could pick the wrong installer when handling releases containing more than one app.


# Version 0.6.8 (2024-07-05)

This release updates cargo-dist.


# Version 0.6.7 (2024-07-05)

This release adds an experimental opt-in tls_native_roots feature.


# Version 0.6.6 (2024-06-12)

This release updates several dependencies.

# Version 0.6.5 (2024-05-30)

This release makes us prefer creating temporary files nested under the install directory, avoiding issues with renaming files across filesystems, in cases where the system tempdir is on a separate logic drive.


# Version 0.6.4 (2024-05-14)

This release contains two bugfixes for the previous release:

* Improved path handling in `check_receipt_is_for_this_executable`.
* Fixed an issue where checking cargo-dist versions from the receipt would fail if the cargo-dist version was a prerelease.

# Version 0.6.3 (2024-05-14)

This release removes a temporary workaround for an upstream cargo-dist bug, removing an ambiguity in install-receipts that pointed at a dir named "bin" for cargo-dist 0.15.0 and later.

# Version 0.6.2 (2024-05-09)

This release fixes a bug which could prevent fetching release information from
GitHub for repositories with under 30 releases ([#106](https://github.com/axodotdev/axoupdater/pull/106)).

# Version 0.6.1 (2024-05-02)

This release reexports the `Version` type to simplify calling `set_current_version`.

# Version 0.6.0 (2024-05-01)

This release contains several new features:

- It's now possible to specify the path to install to via the new `set_install_dir` method. This is especially useful in cases where no install receipt will be loaded, since this value is required for performing full updates.
- It's now possible to skip querying for new versions and force updates to always be performed; this is done by calling `always_update(bool)` on the updater. This is useful in cases where the version of the installed copy of the software to be updated isn't known, or when using axoupdater to perform a first-time install.
- It's now possible to specify a GitHub token via the `set_github_token` method. This is useful when the repo to query is private, or in order to opt into the higher rate limit granted to authenticated requests. AxoUpdater uses this in its own tests. The standalone `axoupdater` executable uses this feature by reading optional tokens specified in the `AXOUPDATER_GITHUB_TOKEN` environment variable.

# Version 0.5.1 (2024-04-16)

This release relaxes the range of the axoasset dependency.

# Version 0.5.0 (2024-04-11)

This release contains a few new features targeted primarily at testing environments.

- A new feature enables forcing axoupdater to call a custom installer at a specified path on disk instead of downloading a new release. This is only expected to be useful for testing. ([#77](https://github.com/axodotdev/axoupdater/pull/77))
- It's now possible to query for a new release without performing an update. ([#78](https://github.com/axodotdev/axoupdater/pull/78))
- It's now possible to manually specify the release source used for querying new releases without needing to read it from an install receipt. ([#81](https://github.com/axodotdev/axoupdater/pull/81))

# Version 0.4.1 (2024-04-10)

This is a minor patch release to preserve more http error info in cases where GitHub is flaking out ([#80](https://github.com/axodotdev/axoupdater/pull/80)).

# Version 0.4.0 (2024-04-08)

This release contains a few new features and fixes:

- Pagination has been implemented for the GitHub API, making it possible to query for specific releases older than the 30 most recent versions. ([#70](https://github.com/axodotdev/axoupdater/pull/70)
- Improved version parsing and handling has been adding, ensuring that axoupdater will no longer try to pick an older stable version if the user is already running on a newer release. ([#72](https://github.com/axodotdev/axoupdater/pull/72))
- Added a test helper to simplify end-to-end self-updater tests for users of the axoupdater library. ([#76](https://github.com/axodotdev/axoupdater/pull/76))

# Version 0.3.6 (2024-04-05)

This is a minor bugfix release. It updates the ordering of axo releases queries to reflect changes to the deployed API.

# Version 0.3.5 (2024-04-05)

This is a minor bugfix release. It makes us try to temporarily rename the current executable on windows, in case we're about to overwrite it.

# Version 0.3.4 (2024-04-04)

This is a minor bugfix release. It fixes an issue which would cause Windows updates to fail if the parent process is PowerShell Core.

# Version 0.3.3 (2024-03-21)

This is a minor bugfix release. It relaxes the reqwest dependency, which had been bumped to 0.12.0 in the previous release. It will now accept either 0.11.0 or any later version.

# Version 0.3.2 (2024-03-21)

This is a minor bugfix release:

* more robust behaviour when paired with installers built with cargo-dist 0.12.0 (not yet released)
* fix for an issue on windows where the installer would never think the receipt matched the binary

# Version 0.3.1 (2024-03-18)

This is a minor bugfix release which fixes loading install receipts which contain UTF-8 byte order marks.

# Version 0.3.0 (2024-03-08)

This release contains several bugfixes and improvements:

- `axoupdater` now compares the path to which the running program was installed to the path it locates in the install receipt, and declines to upgrade if they're not equivalent. This fixes an issue where a user who had installed a copy with an installer which produced an install receipt and a second copy from a package manager would be prompted to upgrade even on the package manager-provided version.
- The `run()` and `run_sync()` methods now provide information on the upgrade that they performed. If the upgrade was performed, it returns the old and new versions and the tag that the new version was built from.
- It's now possible to silence stdout and stderr from the underlying installer when using `axoupdater` as a library.

# Version 0.2.0 (2024-03-06)

This release makes a breaking change to the library API. `run()` and `is_update_needed()` are now both async methods; new `run_sync()` and `is_update_needed_sync()` methods have been added which replicate the old behaviour. This should make it easier to incorporate the axoupdater library into asynchronous applications, especially applications which already use tokio.

To use the blocking methods, enable the `blocking` feature when importing this crate as a library.

# Version 0.1.0 (2024-03-01)

This is the initial release of axoupdater, including both the standalone binary and the library for embedding in other binaries.
