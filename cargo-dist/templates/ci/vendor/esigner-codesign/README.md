# GitHub Action for CodeSigner by SSL.com

## Usage

<!-- start usage -->
```yaml
- name: Sign Artifact with CodeSignTool
  uses: sslcom/esigner-codesign@develop
  with:
    # CodeSignTool Commands:
    # - get_credential_ids: Output the list of eSigner credential IDs associated with a particular user.
    # - credential_info: Output key and certificate information related to a credential ID.
    # - sign: Sign and timestamp code object.
    # - batch_sign: Sign and timestamp multiple code objects with one OTP.
    # - hash: Pre-compute hash(es) for later use with batch_hash_sign command.
    # - batch_sign_hash: Sign hash(es) pre-computed with hash command.
    command: sign

    # SSL.com account username..
    username: ${{secrets.ES_USERNAME}}

    # SSL.com account password.
    password: ${{secrets.ES_PASSWORD}}

    # Credential ID for signing certificate.
    credential_id: ${{secrets.CREDENTIAL_ID}}

    # OAuth TOTP Secret (https://www.ssl.com/how-to/automate-esigner-ev-code-signing)
    totp_secret: ${{secrets.ES_TOTP_SECRET}}

    # Path of code object to be signed.
    # Supported File Types: acm, ax, bin, cab, cpl, dll, drv, efi, exe, mui, ocx, scr, sys, tsp, msi, ps1, ps1xml, js, vbs, wsf, jar
    file_path: ${GITHUB_WORKSPACE}/test/src/build/HelloWorld.jar

    # Input directory for code objects to be signed, have hashes computed, or pick unsigned files and corresponding hashes for signing.
    dir_path: ${GITHUB_WORKSPACE}/test/src/build

    # Directory where signed code object(s) will be written.
    output_path: ${GITHUB_WORKSPACE}/artifacts

    # Scans your file for any possible malware in order to avoid code compromise and prevents signing of code if malware is detected.
    # On batch_sign command: If you are getting 'Error: hash needs to be scanned first before submitting for signing: <hash_value>', you can set this value to true
    malware_block: false
    
    # Overrides the input file after signing, if this parameter is set and no -output_dir_path parameter
    override: false
    
    # This variable are optional, and specify the environment name. If omitted, the environment name will be set to PROD and use production code_sign_tool.properties file. For signing artifact with demo account, the environment name will be set to TEST.
    environment_name: TEST
    
    # Clean log files after code signing operations
    clean_logs: true

    # Maximumx JVM heap size
    jvm_max_memory: 1024M

    # Code signing method. Default is v1. Supported values: v1, v2
    signing_method: v1
```
<!-- end usage -->
