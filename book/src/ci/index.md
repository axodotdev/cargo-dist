# CI

All of the [distribute functionality][distribute] of cargo-dist depends on some kind of CI integration to provide things like [file hosting][artifact-url], secret keys, and the ability to spin up multiple machines.

A CI backend can be enabled with [the ci config][config-ci]



## Supported CI Providers

* [github][]: use GitHub Actions and uploads to GitHub Releases




## Future CI Providers

The following CI providers have been requested, and we're open to supporting them, but we have no specific timeline for when they will be implemented. Providing additional info/feedback on them helps us prioritize the work:

* [gitlab](https://github.com/axodotdev/cargo-dist/issues/48)
* [travis](https://github.com/axodotdev/cargo-dist/issues/273)





[config-ci]: ../reference/config.md#ci

[github]: ./github.md

[artifact-url]: ../reference/artifact-url.md
[distribute]: ../introduction.md#distributing