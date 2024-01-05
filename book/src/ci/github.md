# GitHub CI

> since 0.0.3

<!-- toc -->

The GitHub CI backend provides a "Release" Workflow that is triggered by pushing a tagged commit to your repository. It uses the tag to determine which packages you're trying to publish, and builds and uploads them to a GitHub Release.


## Setup

cargo-dist is currently very eager to setup the GitHub CI backend, so it's pretty easy to do! Most likely it was automatically setup the first time you ran `cargo dist init`. If you followed [the way-too-quickstart][quickstart], then you should also have it setup.


### Setup Step 1: set "repository" in your Cargo.toml

You probably already have it set, but if you don't, now's the time to do it. We need to know [the URL of your GitHub repository][artifact-url] for several features, and the next step will fail without it.


### Setup Step 2: run init and enable GitHub CI

Run `cargo dist init` on your project.

If you did the previous step, you should get prompted to "enable Github CI and Releases?", with the default answer being "yes". Choose yes.

You will also get prompted to "check your release process in pull requests?", with the default answer being "plan - run 'cargo dist plan' on PRs (recommended)". Choose that option.

Once init completes, some changes will be made to your project, **check all of them in**:

* `ci = ["github"]` should be added to `[workspace.metadata.dist]`
* `./github/workflows/release.yml` should be created, this is your Release Workflow


### Setup Step 3: you're done! (time to test)

See [the quickstart's testing guide][testing] for the various testing options.

The easiest testing option for this is to open a pull-request for everything you checked in -- it should run the `plan` step of your release CI as part of the PR.



## Advanced Usage

Here are some more advanced things you can do with GitHub CI.


### Build and upload artifacts on every pull request

> since 0.3.0

By default, cargo-dist will run the plan step on every pull request but won't perform a full release build. If these builds are turned on, the resulting pull request artifacts won't be uploaded to a release but will be available as a download from within the CI job. To enable this, select the "upload" option from the "check your release process in pull requests" question in `cargo-dist-init` or set [the `pr-run-mode` key][config-pr-run-mode] to `"upload"` in `Cargo.toml`'s cargo-dist config. For example:

```toml
pr-run-mode = "upload"
```


### Bring your own release

> since 0.2.0

By default, cargo-dist will want to create its own GitHub Release and set the title/body with things like your CHANGELOG/RELEASES and some info about how to install the release. However if you have your own process for generating the contents of GitHub Release, we support that.

If you set [`create-release = false`](../reference/config.md#create-release) in your cargo-dist config, cargo-dist will assume a draft Github Release for the current git tag already exists with the title/body you want, and just upload artifacts to it. At the end of a successful publish it will undraft the GitHub Release for you.



#### Limitations

* Currently, the only supported package managers are Apt (Linux), Chocolatey (Windows) and Homebrew (macOS).
* GitHub currently only provides x86_64 macOS runners. When you request packages, the Intel versions will always be installed regardless of build targets. While Apple Silicon builds can use CLI tools installed this way, you will not be able to build software for Apple Silicon if it requires C libraries from Homebrew.



### Hand-editing release.yml

> since 0.3.0

The happy-path of cargo-dist has us completely managing release.yml, and since 0.3.0 we will actually consider it an error for there to be any edits or out of date information in release.yml.

If there's something that cargo-dist can't do that makes you want to hand-edit the file, we'd love to hear about it so that you can stay on the happy-path!

However we know you sometimes really need to do those hand-edits, so there is a way to opt into it. If you [set `allow-dirty = ["ci"]` in your cargo-dist config][config-allow-dirty], cargo-dist will stop trying to update the file and stop checking if it's out of date.

Although you're not "using cargo-dist wrong" if you do this, **be aware that you are losing access to a lot of the convenience and UX benefits of cargo-dist**. Every piece of documentation that says "just run cargo dist init" may not work correctly, as a new feature may require the CI template to be updated. Even things as simple as "updating cargo-dist" will stop working.

We have put a lot of effort into minimizing those situations, with `plan` increasingly being responsible for dynamically computing what the CI should do, but that's not perfect, and there's no guarantees that future versions of cargo-dist won't completely change the way CI is structured.


### Fiddly build task settings

> since 0.0.1

Here's a grab-bag of more random settings you probably don't want to use, but exist in case you need them.

By default cargo-dist lets all the build tasks keep running even if one of them fails, to try to get you as much as possible when things go wrong. [`fail-fast = true` can be set to disable this][config-fail-fast].

By default cargo-dist breaks build tasks onto more machines than strictly necessary to create the maximum opportunities for concurrency and to increase fault-tolerance. For instance if you want to build for both arm64 macOS and x64 macOS, that *could* be done on the same machine, but we put it on two machines so they can be in parallel and succeed/fail independently. [`merge-tasks = true` can be set to disable this][config-merge-tasks].



### Checking what your build linked against

> since 0.4.0

Although most Rust builds are statically linked and contain their own Rust dependencies, some crates will end up dynamically linking against system libraries. It's useful to know what your software picked up&mdash;sometimes this will help you catch things you may not have intended, like dynamically linking to OpenSSL, or allow you to check for package manager-provided libraries your users will need to have installed in order to be able to run your software.

cargo-dist provides a linkage report during your CI build in order to allow you to check for this. For macOS and Linux, it's able to categorize the targets it linked against to help you gauge whether or not it's likely to cause problems for your users. To view this, check the detailed view of your CI build and consult the "Build" step from the `upload-local artifacts` jobs.

This feature is defined for advanced users; most users won't need to use it. It's most useful for developers with specialized build setups who want to ensure that their binaries will be safe for all of their users. A few examples of users who may need to use it:

* Users with custom runners with extra packages installed beyond what's included in the operating system;
* Users who have installed extra packages using cargo-dist's system dependency feature;
* Users whose cargo buildsystems include extra C dependencies.

The report is divided into categories to help you make sense of where these libraries are from and what it might mean for your users. These categories are:

* System: Libraries that come with your operating system. On Linux, these packages are all provided by the system's package manager, and the linkage report includes information about which package includes each library. Some of these packages will be included in the base OS, and will be safe to rely on, while you'll need to ensure your users have others. If you're using standard base images like GitHub Actions's and haven't installed additional packages using apt, the packages in this list should be preinstalled for your users. On macOS, these packages are shipped with the operating system and not managed by a package manager; you can always rely on these being there within the same version of macOS.
* Homebrew (macOS only): Libraries that are provided by the Homebrew package manager for macOS. These packages are not installed by default, so your users will need to have them installed in order to be able to use your software.
* Public (unmanaged): Libraries which are present in public locations, but which are not managed or provided by the system or a package manager. Because these are not standard parts of the operating system, your users will be unlikely to have them.
* Frameworks (macOS only): Frameworks, a special type of library provided by macOS. Frameworks installed in the `/System` directory come with the operating system and are available to all users.
* Other: A catch-all category for any libraries which don't fall in the previous categories.

Here's an example of what a linkage report looks like for a Linux binary;

```
axolotlsay (x86_64-unknown-linux-gnu):

┌────────────────────┬─────────────────────────────────────────────────┐
│ Category           ┆ Libraries                                       │
╞════════════════════╪═════════════════════════════════════════════════╡
│ System             ┆ /lib/x86_64-linux-gnu/libgcc_s.so.1 (libgcc-s1) │
│                    ┆ /lib/x86_64-linux-gnu/libpthread.so.0 (libc6)   │
│                    ┆ /lib/x86_64-linux-gnu/libc.so.6 (libc6)         │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Homebrew           ┆                                                 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Public (unmanaged) ┆                                                 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Frameworks         ┆                                                 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Other              ┆                                                 │
└────────────────────┴─────────────────────────────────────────────────┘
```

#### Limitations

While the linkage report can be run locally, the report for Linux artifacts can only be run on Linux.

The Windows report is currently unable to provide information about the sources of libraries.


[config-fail-fast]: ../reference/config.md#fail-fast
[config-merge-tasks]: ../reference/config.md#merge-tasks
[config-allow-dirty]: ../reference/config.md#allow-dirty
[config-pr-run-mode]: ../reference/config.md#pr-run-mode

[artifact-url]: ../reference/artifact-url.md#github
[quickstart]: ../way-too-quickstart.md
[testing]: ../way-too-quickstart.md#test-it-out
