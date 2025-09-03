# MacOS Artifact Signing

> since 0.22.0

cargo-dist can automatically codesign Mac executables using Apple's builtin tooling.

## Quickstart

### Part 1: Provision a certificate and set up your repository with it

<!-- TODO: Document the process of obtaining and exporting the signature. -->

3. **Export the certificate to disk**

    Locate your certificate within Keychain, then right-click and select "Export". Ensure that you've selected the "Personal Information Exchange (.p12)" format at the bottom of the export window. Once you've selected a filename, Keychain will prompt you for a password to protect the exported item. Select a secure password, *and ensure remember it* - you'll need this for the next step.

4. **Encode the certificate via base64**

    In order to add the certificate to your GitHub secrets in a future step, we'll need to convert it to a text-based format. To do that, we'll use base64. In your terminal, run the following:

    ```sh
    base64 < PATH_TO_YOUR_CERT
    ```

    (Instead of typing the path to your certificate, you can also drag and drop it onto your terminal after typing `base64 <`.)

    Copy *the full text* that was generated; you'll need it in the next step.

5. **Add [GitHub Secrets](https://docs.github.com/en/actions/security-guides/encrypted-secrets) to your repository**

    You'll need the following three secrets:

    - `CODESIGN_IDENTITY`: the identity in the certificate
    - `CODESIGN_CERTIFICATE_PASSWORD`: this is the base64-encoded certificate from Step 4
    - `CODESIGN_CERTIFICATE_PASSWORD`: this is the password from Step 3

### Part 2: Enable macOS signing with cargo-dist

1. **Configure cargo-dist to codesign**

    Add the following to your `Cargo.toml` or `dist.toml`:

    ```toml
    [workspace.metadata.dist]
    macos-sign = true
    ```

2. **Run `cargo dist init` on your project**

    You've already fully configured the feature, we're just ensuring your config changes are applied and checked.
