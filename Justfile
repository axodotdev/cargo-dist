
build-all-plaforms:
  #!/bin/bash -eux
  export AXOASSET_XZ_LEVEL=1
  cargo build
  ./target/debug/dist init --yes
  ./target/debug/dist build --artifacts all --target aarch64-apple-darwinx,x86_64-apple-darwin,x86_64-pc-windows-msvc,x86_64-unknown-linux-musl

patch-sh-installer:
  #!/usr/bin/env node
  const fs = require('fs');
  const installerUrl = process.env.INSTALLER_DOWNLOAD_URL || 'https://dl.bearcove.cloud/dump/dist-cross';
  const installerPath = './target/distrib/cargo-dist-installer.sh';

  const content = fs.readFileSync(installerPath, 'utf8');
  const lines = content.split('\n');
  let modified = false;

  const newLines = [];
  for (const line of lines) {
    if (line.includes('export INSTALLER_DOWNLOAD_URL')) {
      continue;
    }
    if (line.includes('set -u') && !modified) {
      modified = true;
      newLines.push(line);
      newLines.push(`export INSTALLER_DOWNLOAD_URL=${installerUrl} # patched by Justfile in dist repo, using dist_url_override feature`);
      continue;
    }
    newLines.push(line);
  }

  fs.writeFileSync(installerPath, newLines.join('\n'));

  if (modified) {
    console.log('\x1b[32m%s\x1b[0m', `✅ ${installerPath} patched successfully!`);
    console.log('\x1b[36m%s\x1b[0m', `🔗 Pointing to: ${installerUrl}`);
  } else {
    console.log('\x1b[31m%s\x1b[0m', '❌ Error: Could not find line with "set -u" in installer script');
  }

patch-ps1-installer:
  #!/usr/bin/env node
  const fs = require('fs');
  const installerUrl = process.env.INSTALLER_DOWNLOAD_URL || 'https://dl.bearcove.cloud/dump/dist-cross';
  const installerPath = './target/distrib/cargo-dist-installer.ps1';

  const content = fs.readFileSync(installerPath, 'utf8');
  const lines = content.split('\n');
  let modified = false;

  const newLines = [];
  for (const line of lines) {
    if (line.includes('$env:INSTALLER_DOWNLOAD_URL = ')) {
      continue;
    }
    if (line.includes('$app_name = ') && !modified) {
      modified = true;
      newLines.push(`$env:INSTALLER_DOWNLOAD_URL = "${installerUrl}" # patched by Justfile in dist repo, using dist_url_override feature`);
      newLines.push(line);
      continue;
    }
    newLines.push(line);
  }

  fs.writeFileSync(installerPath, newLines.join('\n'));

  if (modified) {
    console.log('\x1b[32m%s\x1b[0m', `✅ ${installerPath} patched successfully!`);
    console.log('\x1b[36m%s\x1b[0m', `🔗 Pointing to: ${installerUrl}`);
  } else {
    console.log('\x1b[31m%s\x1b[0m', '❌ Error: Could not find line with "cargo-dist = " in installer script');
  }

dump:
  #!/bin/bash -eux
  just build-all-plaforms
  just patch-sh-installer
  just patch-ps1-installer
  mc mirror --overwrite ./target/distrib ${DIST_TARGET:-bearcove/dump/dist-cross}
