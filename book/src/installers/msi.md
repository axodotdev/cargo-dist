# msi Installer

> Since 0.3.0

<!-- toc -->

This guide will walk you through setting up a [bundling][] Windows msi installer. It assumes you've already done initial setup of dist, as described in [the way-too-quickstart][quickstart], and now want to add an msi to your release process.

(Just a bit of a warning, this stuff works but there's a few rough edges, please let us know if you run into any issues!)


## Setup

We'll start with the bare-minimum and then explain what we did and how to modify it afterwards.


### Setup Step 1: set "authors" in your Cargo.toml

msi requires you to specify a "manufacturer" for you application, which is by default sourced from the "authors" field in you Cargo.toml. **If you don’t have that field set, the next step will error out.** If you have an authors entry like `My Cool Company <support@mycoolcompany.org>` then the manufacturer will be "My Cool Company".


### Setup Step 2: run init and enable "msi"

Rerun `dist init` and when it prompts you to choose installers, enable "msi".

Once init completes, some changes will be made to your project, **check all of them in**:

1. `installers = ["msi"]` will be added to `[workspace.metadata.dist]`
2. `[package.metadata.wix]` will be added to your packages with distable binaries. This is your msi-specific config. For now don't worry about it.
3. `wix/main.wxs` will be created for each of your packages with distable binaries. This is a template for your msi. For now assume this file is completely managed by dist, and can't be hand-edited.


### Setup Step 3: you're done! (time to test)

See [the quickstart's testing guide][testing] for the various testing options.

If the above steps worked, `dist plan` should now include an msi for each Windows platform you support.

`dist build` is a bit trickier. Not only do you have to be on Windows to get an msi built, you also need to have the [WiX v3 toolchain][wix3] installed (WiX v4 isn't yet supported). If you don't the build will just error out. In GitHub CI the WiX v3 toolchain is pre-installed, so using PR testing is recommended.

The resulting msi should include the following functionality:

* (optional) EULA dialog
* A menu that lets you choose where to install and whether to add it to PATH
    * Default install location is `%ProgramFiles%\{app_name}\` (e.g. `C:\Program Files\axolotlsay\`)
    * Default is to add the install location to PATH
    * Currently the only files that will be included are the app's binaries in a `bin` subdir
* If rerun, you will get an uninstall/reinstall menu
* If a newer version is run, it will automatically uninstall the old version
* If an older version is run, it will report that a newer version is installed and exit
* The application will appear in the Windows "Add or remove programs" menu and can be uninstalled from there

Certain licenses in your Cargo.toml like "Apache" or "MIT" (but *not* dual MIT/Apache) will get an auto-generated EULA that's just agreeing to the software license -- we know, that's not how software licenses work, but people seem to like to do it. See the section on advanced usage for how to set a more useful EULA.



## How It Works

As you may suspect from the setup, we rely on the industry standard [WiX v3 toolchain][wix3] to generate your msi installers (WiX v4 isn't yet supported). The `main.wxs` format is its xml-based templating system. Some of the information about your app is baked into this template (binaries, descriptions, licenses...), while other information is sourced at build time (mostly the version).

If the template ever desyncs from the values it was generated from, commands like `dist plan` (and therefore your pull request CI) will error out and ask you to rerun `dist init` to regenerate it.

The values we added to `[package.metadata.wix]` are:

* `upgrade-guid = "SOME-RANDOM-GUID"` (since 0.3.0)
* `path-guid = "SOME-OTHER-RANDOM-GUID"` (since 0.3.0)
* `license = false` (since 0.5.0)
* `eula = false` (since 0.5.0)

The two GUIDs are used by Windows to determine that two MSIs with different versions refer to the same application and install location, which is required for it to properly handle things like upgrades. They are persisted in your Cargo.toml to keep them stable across regenerations of `main.wxs`.

The license/EULA settings are there to disable the auto-license/EULA feature of cargo-wix. That feature *would* look at your package's license and potentially turn it into a EULA agreement. While this is a thing some folks want, most of our users aren't interested in getting their end-users to "agree to the MIT License". You can opt back into auto-EULAs by setting both of those to `true` (if you just delete the keys dist will keep adding them back as `false`).

**All of the logic for generating wxs files is part of [cargo-wix][], which dist includes as a library.** It's a great project we happily contribute to, although some TLC is still needed to make the integration perfect (some of its warnings/errors may mention its own CLI's flags, and those sure won't work if you pass them to dist). The `[package.metadata.wix]` config is purely cargo-wix's, see [their docs for all the knobs it exposes][cargo-wix].



## Advanced Usage

There are two paths for advanced usage: managed and unmanaged. We recommend the managed approach, but the unmanaged approach is there for true power users.

### Managed Advanced Usage

If you want dist to be able to keep your `main.wxs` consistent with the definitions in your Cargo.tomls, then all you have available is the knobs exposed in `[package.metadata.wix]` -- see [cargo-wix's docs for details][cargo-wix].

### Unmanaged Advanced Usage

If you're not worried about keeping `main.wxs` consistent, then you can choose to dive deep into the full power of [WiX v3][wix3] by adding `allow-dirty = ["msi"]` to your dist config. Once you do this dist will stop trying to update it, and won't check if it's out of date.

At that point you can make whatever hand-edits you want to main.wxs, as long as you still use the variables that cargo-wix injects into the template at build-time for things like versions and binary paths.

See [WiX v3's docs][wix3] for all the things their format supports.



[quickstart]: ../quickstart/index.md
[testing]: ../quickstart/rust.md#test-it-out
[bundling]: ./index.md#bundling-installers

[cargo-wix]: https://volks73.github.io/cargo-wix/cargo_wix/
[wix3]: https://wixtoolset.org/docs/wix3/