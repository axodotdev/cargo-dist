# PowerShell Script Installer

> since 0.0.3

This provides a powershell script (my-app-installer.ps1) which detects the current platform, fetches the best possible [archive][] from your [Artifact URL][artifact-url], copies the binary into your [install-path][config-install-path], and attempts to add that path to the user's PATH (see the next section for details).

This kind of installer is ideal for bootstrapping setup on a fairly bare-bones system.

An "installer hint" will be provided that shows how to install via `irm | iex` (the windows equivalent of `curl | sh`), like so:

```sh
powershell -c "irm https://github.com/axodotdev/cargo-dist/releases/download/v0.0.5/cargo-dist-v0.0.5-installer.ps1 | iex"
```

Limitations/Caveats:

* Requires a well-defined [Artifact URL][artifact-url]
* Currently only really designed for "native windows", and won't detect other platforms properly
* [Cannot detect situations where musl-based builds are appropriate][issue-musl] (static or dynamic)
* Relies on the user's installation of `tar` and `Expand-Archive` to unpack the files
* Relies on the the user's installation of `Net.Webclient` to fetch the files
* [Will throw out all files except for the binary, so the binary can't rely on assets included in the archive][issue-unpack-all]
* Cannot run any kind of custom install logic

On the scale of Windows (where many people are still running Windows 7) commands like "Expand-Archive" and "tar" are in fact relatively new innovations. Any system that predates 2016 (PowerShell 5.0) certainly has no hope of working. I believe that someone running Windows 10 is basically guaranteed to work, and anything before that gets sketchier.

In an ideal world most of these caveats improve (except for maybe the requirement of PowerShell >= 5.0 which is not pleasant to push past).


## Adding things to PATH

Here is a more fleshed out description of how the powershell installer attempts to add the [install-path][config-install-path] to the user's PATH, and the limitations of that process.

The most fundamental limitation is that installers fundamentally cannot edit the PATH of the currently running shell (it's a parent process). Powershell does not have an equivalent of `source`, so to the best of our knowledge restarting the shell is the only option (which if using Windows Terminal seems to mean opening a whole new window, tabs aren't good enough). As such, it benefits an installer to try to install to a directory that will already be on PATH (such as [CARGO_HOME][cargo home]). ([rustup also sends a broadcast WM_SETTINGCHANGE message](https://github.com/rust-lang/rustup/blob/bcfac6278c7c2f16a41294f7533aeee2f7f88d07/src/cli/self_update/windows.rs#L397-L409), but we couldn't find any evidence that this does anything useful.)

The process we use to add [install-path][config-install-path] to the user's PATH is roughly the same process that rustup uses (hopefully making us harmonious with running rustup before/after one of our installer scripts). In the following description we will use `$install-path` as a placeholder for the path computed at install-time where the binaries get installed. Its actual value will likely look something like `C:\Users\axo\.myapp` or `C:\Users\.cargo\bin`.

* we load from the registry `HKCU:\Environment`'s "Path" Item
* we check if `$install-path` is contained within it already
* if not, we prepend it and write the value back
    * prepending is used to ideally override system-installed binaries, as that is assumed to be desired when explicitly installing with not-your-system-package-manager
* if we edited the registry, we prompt the user to restart their shell




[issue-irm-iex]: https://github.com/axodotdev/oranda/issues/393
[issue-musl]: https://github.com/axodotdev/cargo-dist/issues/75
[issue-unpack-all]: https://github.com/axodotdev/cargo-dist/issues/307

[config-install-path]: ../reference/config.md#install-path

[archive]: ../artifacts/archives.md
[artifact-url]: ../reference/artifact-url.md

[cargo home]: https://doc.rust-lang.org/cargo/guide/cargo-home.html
