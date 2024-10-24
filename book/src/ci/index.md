# CI

All of the [distribute functionality][distribute] of dist depends on some kind of CI integration to provide things like [file hosting][artifact-url], secret keys, and the ability to spin up multiple machines.

dist enables CI for you by default the first time you `dist init`. dist's core CI job can be [customized][ci-customization] using several extra features.



## Supported CI Providers

* github: uses GitHub Actions and uploads to GitHub Releases


## Future CI Providers

The following CI providers have been requested, and we're open to supporting them, but we have no specific timeline for when they will be implemented. Providing additional info/feedback on them helps us prioritize the work:

* [gitlab](https://github.com/axodotdev/cargo-dist/issues/48)
* [travis](https://github.com/axodotdev/cargo-dist/issues/273)


## Advanced configuration

The default CI configuration covers most users' needs. For more advanced needs, we have an extensive guide on how to [customize][ci-customization] your CI pipeline.


## A quick tour of the CI process

The CI process is divided into several stages which happen in order. Understanding these steps will help you follow the release process and, if necessary, debug failures.

1. plan: dist calculates which builds to run, and which platforms to build for, and enumerates the files that builds are expected to produce. The output of this step is saved and shared between steps and is also included in the final release as `dist-manifest.json`.
2. build-local-artifacts: dist builds native binaries and produces tarballs.
3. build-global-artifacts: dist builds platform-independent artifacts such as installers.
4. host: dist decides whether to proceed with publishing a release and uploading artifacts.
5. publish: Artifacts are uploaded and, if used, the Homebrew formula is released.
6. announce: The release is created with its final non-draft contents.

## Outputs to watch out for

The most important output of your build is your release, but there's more advanced information in the logs for users who need it.

### Checking what your build linked against

> since 0.4.0

Although most Rust builds are statically linked and contain their own Rust dependencies, some crates will end up dynamically linking against system libraries. It's useful to know what your software picked up&mdash;sometimes this will help you catch things you may not have intended, like dynamically linking to OpenSSL, or allow you to check for package manager-provided libraries your users will need to have installed in order to be able to run your software.

dist provides a linkage report during your CI build in order to allow you to check for this. For macOS and Linux, it's able to categorize the targets it linked against to help you gauge whether or not it's likely to cause problems for your users. To view this, check the detailed view of your CI build and consult the "Build" step from the `upload-local artifacts` jobs.

This feature is defined for advanced users; most users won't need to use it. It's most useful for developers with specialized build setups who want to ensure that their binaries will be safe for all of their users. A few examples of users who may need to use it:

* Users with custom runners with extra packages installed beyond what's included in the operating system;
* Users who have installed extra packages using dist's system dependency feature;
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


[ci-customization]: ../ci/customizing.md
[config-ci]: ../reference/config.md#ci

[artifact-url]: ../reference/artifact-url.md
[distribute]: ../introduction.md#distributing
