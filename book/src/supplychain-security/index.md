# Supply-chain security

As software supplychain security concerns and requirements grow, `dist` is
committed to making compliance with policies and regulations as turnkey as possible.

If you have an integration you are looking for [file an issue](https://github.com/axodotdev/cargo-dist/issues/new) or
[join our Discord](https://discord.gg/ry3f3HZXWN).

## Signing

* [Windows Codesigning](./signing/windows.md)
* [macOS Codesigning](./signing/macos.md)
* [🔜 Linux Codesigning](https://github.com/axodotdev/cargo-dist/issues/120)
* [🔜 Sigstore Signing](https://github.com/axodotdev/cargo-dist/issues/120)
* [🔜 Windows Trusted Signing](https://github.com/axodotdev/cargo-dist/issues/1122)


## Attestation

* [GitHub Attestation](./attestations/github.md)


## SBOMs and Dependency Managers

### cargo-cyclonedx

dist can optionally generate a [CycloneDX][CycloneDX]-format Software Bill of Materials (SBOM) for Rust projects using the [cargo-cyclonedx][cargo-cyclonedx] tool. This data is stored as a standalone `bom.xml` file which is distributed alongside your binaries in your tarballs. Users can validate that SBOM file using [any compatible CycloneDX tool](https://cyclonedx.org/tool-center/). For more information about using this feature, see [the config documentation](../reference/config.html#cargo-cyclonedx).

### cargo-auditable

[cargo-auditable][cargo-auditable] can optionally be used to embed dependency information into your Rust binaries, making it possible for users to check your binaries for the full dependency tree they were built from along with their precise versions. This information can then be checked later to scan your binary for any known vulnerabilities using the [cargo-audit][cargo-audit] tool. For more information about using this feature, see [the config documentation](../reference/config.html#cargo-auditable).

## Software identification

dist can optionally generate an [OmniBOR artifact ID][omnibor] for software artifacts using the [omnibor-cli][omnibor-cli] tool. These identifiers are reproducible and unique to a specific version of your software. For more information about using this feature, see [the config documentation](../reference/config.html#omnibor).

[CycloneDX]: https://cyclonedx.org
[cargo-audit]: https://github.com/rustsec/rustsec/tree/main/cargo-audit#cargo-audit-bin-subcommand
[cargo-auditable]: https://github.com/rust-secure-code/cargo-auditable
[cargo-cyclonedx]: https://cyclonedx.org
[omnibor]: https://omnibor.io
[omnibor-cli]: https://github.com/omnibor/omnibor-rs/tree/main/omnibor-cli
