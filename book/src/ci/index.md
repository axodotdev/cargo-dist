# CI

All of the [distribute functionality][distribute] of cargo-dist depends on some kind of CI integration to provide things like [file hosting][artifact-url], secret keys, and the ability to spin up multiple machines.

A CI backend can be enabled with [the ci config][config-ci]. cargo-dist's core CI job can be [customized][ci-customization] using several extra features.



## Supported CI Providers

* [github][]: use GitHub Actions and uploads to GitHub Releases




## A quick tour of the CI process

The CI process is divided into several stages which happen in order. Understanding these steps will help you follow the release process and, if necessary, debug failures.

1. plan: cargo-dist calculates which builds to run, and which platforms to build for, and enumerates the files that builds are expected to produce. The output of this step is saved and shared between steps and is also included in the final release as `dist-manifest.json`.
2. build-local-artifacts: cargo-dist builds native binaries and produces tarballs.
3. build-global-artifacts: cargo-dist builds platform-independent artifacts such as installers.
4. host: cargo-dist decides whether to proceed with publishing a release and uploading artifacts.
5. publish: Artifacts are uploaded and, if used, the Homebrew formula is released.
6. announce: The release is created with its final non-draft contents.




## Future CI Providers

The following CI providers have been requested, and we're open to supporting them, but we have no specific timeline for when they will be implemented. Providing additional info/feedback on them helps us prioritize the work:

* [gitlab](https://github.com/axodotdev/cargo-dist/issues/48)
* [travis](https://github.com/axodotdev/cargo-dist/issues/273)




[ci-customization]: ../ci/customizing.md
[config-ci]: ../reference/config.md#ci

[github]: ./github.md

[artifact-url]: ../reference/artifact-url.md
[distribute]: ../introduction.md#distributing
