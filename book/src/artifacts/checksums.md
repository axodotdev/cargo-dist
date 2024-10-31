# Checksums

By default dist will generate a matching checksum file for each [archive][] it generates. The default checksum is sha256, so for instance `my-app-x86_64-pc-windows-msvc.zip` will also come with `my-app-x86_64-pc-windows-msvc.zip.sha256` that tools like `sha256sum` can use. This can be configured with [the checksum config][config-checksum].

[Fetching installers][fetching-installers] can also use these checksums (or ones baked into them) to validate the integrity of the files they download. With https and unsigned checksums the security benefit is minimal, but it can catch more boring problems like data corruption.

The homebrew installer actually ignores your checksum setting and always uses sha256 hashes that are baked into it, as required by homebrew itself.

Updating the other fetching installers to use these checksums is [still a work in progress][issue-checksum-backlog].

> since 0.24.0

cargo-dist also generates a "unified" checksum file, like `sha256.sum`, which contains the checksums for all the archives it has generated, in a format that can be checked with `sha256sum -c`, for example.

Individual checksums will be deprecated in a future version in favor of that unified checksum file.

Although you can [pick other checksum algorithms][config-checksum], since you can only pick one, be aware that not every macOS/Linux/Windows system may have tools installed that are able to check `blake2b`, for example.

[issue-checksum-backlog]: https://github.com/axodotdev/cargo-dist/issues/439

[config-checksum]: ../reference/config.md#checksum

[archive]: ../artifacts/archives.md
[fetching-installers]: ../installers/index.md#fetching-installers
