# Checksums

By default dist will generate a matching checksum file for each [archive][] it generates. The default checksum is sha256, so for instance `my-app-x86_64-pc-windows-msvc.zip` will also come with `my-app-x86_64-pc-windows-msvc.zip.sha256` that tools like `sha256sum` can use. This can be configured with [the checksum config][config-checksum].

[Fetching installers][fetching-installers] can also use these checksums (or ones baked into them) to validate the integrity of the files they download. With https and unsigned checksums the security benefit is minimal, but it can catch more boring problems like data corruption.

The homebrew installer actually ignores your checksum setting and always uses sha256 hashes that are baked into it, as required by homebrew itself.

Updating the other fetching installers to use these checksums is [still a work in progress][issue-checksum-backlog].



[issue-checksum-backlog]: https://github.com/axodotdev/cargo-dist/issues/439

[config-checksum]: ../reference/config.md#checksum

[archive]: ../artifacts/archives.md
[fetching-installers]: ../installers/index.md#fetching-installers