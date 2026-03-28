// Tests that the npm installer correctly forwards proxy authentication
// credentials when HTTPS_PROXY contains username:password.
//
// This exercises the actual installer code path end-to-end:
// 1. Starts a local HTTP CONNECT proxy requiring Basic auth
// 2. Sets HTTPS_PROXY=http://testuser:testpass@127.0.0.1:<port>
// 3. Patches package.json so binary.js has a real HTTPS download URL
// 4. Requires binary.js (which sets up whatever proxy mechanism it uses)
// 5. Calls install() — the same function npm postinstall runs
// 6. Checks whether the proxy actually received Proxy-Authorization
//
// On the buggy code (axios-proxy-builder): FAILS — credentials are dropped
// On the fixed code (undici EnvHttpProxyAgent): PASSES — credentials are sent

"use strict";

const http = require("http");
const fs = require("fs");
const os = require("os");
const path = require("path");

// Kill the test if it hangs
setTimeout(() => {
  console.error("FAIL: Test timed out");
  process.exit(1);
}, 15000);

async function main() {
  let authReceived = false;

  // Start a minimal CONNECT proxy that inspects the Proxy-Authorization header
  const proxy = http.createServer();
  proxy.on("connect", (req, clientSocket) => {
    const auth = req.headers["proxy-authorization"];
    if (auth) {
      const decoded = Buffer.from(
        auth.replace("Basic ", ""),
        "base64",
      ).toString();
      if (decoded === "testuser:testpass") {
        authReceived = true;
      }
    }
    // Always reject — we only need to observe whether auth was sent
    clientSocket.write("HTTP/1.1 502 Bad Gateway\r\n\r\n");
    clientSocket.end();
  });

  await new Promise((resolve) => proxy.listen(0, "127.0.0.1", resolve));
  const { port } = proxy.address();

  // Set proxy env vars BEFORE requiring binary.js (which may set up
  // a global proxy dispatcher at module load time)
  process.env.HTTPS_PROXY = `http://testuser:testpass@127.0.0.1:${port}`;
  process.env.HTTP_PROXY = process.env.HTTPS_PROXY;
  process.env.NO_PROXY = "";

  // Patch package.json so install() has a valid HTTPS download URL
  // and a supportedPlatforms entry matching this machine
  const pkgPath = path.join(__dirname, "package.json");
  const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf8"));

  // Build the target triple the same way binary.js does
  const rawOs = os.type();
  const rawArch = os.arch();
  let osType = "";
  switch (rawOs) {
    case "Windows_NT": osType = "pc-windows-msvc"; break;
    case "Darwin": osType = "apple-darwin"; break;
    case "Linux": osType = "unknown-linux-gnu"; break;
  }
  let arch = "";
  switch (rawArch) {
    case "x64": arch = "x86_64"; break;
    case "arm64": arch = "aarch64"; break;
  }
  const triple = `${arch}-${osType}`;

  pkg.artifactDownloadUrls = ["https://example.com/releases"];
  pkg.supportedPlatforms = {
    [triple]: {
      artifactName: "fake-artifact.tar.gz",
      bins: { "fake-bin": "fake-bin" },
      zipExt: ".tar.gz",
    },
  };
  fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2));

  // Copy binary.js and binary-install.js into this directory so
  // require("./binary") works (they live under npm/ in the template)
  const npmDir = path.join(__dirname, "npm");
  if (fs.existsSync(npmDir)) {
    for (const file of ["binary.js", "binary-install.js"]) {
      const src = path.join(npmDir, file);
      if (fs.existsSync(src)) {
        fs.copyFileSync(src, path.join(__dirname, file));
      }
    }
  }

  // The installer's error() helper calls process.exit(1) on download failure,
  // which would kill the process before we can check the proxy. Intercept it.
  const realExit = process.exit;
  process.exit = () => {};

  // NOW require binary.js — this is the actual installer code under test.
  // On the fixed version, this calls setGlobalDispatcher(new EnvHttpProxyAgent())
  // at module load time, configuring proxy for all subsequent fetch() calls.
  // On the buggy version, this just loads axios-proxy-builder.
  const { install } = require("./binary");

  // Call install() — exactly what the npm postinstall hook does.
  // This will attempt to download from https://example.com/releases/fake-artifact.tar.gz
  // which must go through our proxy (since HTTPS_PROXY is set).
  try {
    await install(true);
  } catch {
    // Expected to fail (proxy rejects, or download fails)
  }

  // Restore process.exit for our own use
  process.exit = realExit;

  await new Promise((resolve) => proxy.close(resolve));

  if (authReceived) {
    console.log("PASS: Proxy received authentication credentials");
    process.exit(0);
  } else {
    console.error(
      "FAIL: Proxy did NOT receive authentication credentials\n" +
        "The installer sent a CONNECT request to the proxy without " +
        "Proxy-Authorization.\nThis means authenticated proxies " +
        "(HTTPS_PROXY=http://user:pass@host:port) are broken.",
    );
    process.exit(1);
  }
}

main().catch((e) => {
  console.error("Test error:", e);
  process.exit(1);
});
