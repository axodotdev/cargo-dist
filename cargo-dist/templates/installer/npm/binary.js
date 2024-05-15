const { Package } = require("./binary-install");
const os = require("os");
const cTable = require("console.table");
const libc = require("detect-libc");
const HttpsProxyAgent = require("https-proxy-agent");

const error = (msg) => {
  console.error(msg);
  process.exit(1);
};

const {
  name,
  artifactDownloadUrl,
  supportedPlatforms,
} = require("./package.json");

const builderGlibcMajorVersion = 2;
const builderGlibcMInorVersion = 31;

const getPlatform = () => {
  const rawOsType = os.type();
  const rawArchitecture = os.arch();

  // We want to use rust-style target triples as the canonical key
  // for a platform, so translate the "os" library's concepts into rust ones
  let osType = "";
  switch (rawOsType) {
    case "Windows_NT":
      osType = "pc-windows-msvc";
      break;
    case "Darwin":
      osType = "apple-darwin";
      break;
    case "Linux":
      osType = "unknown-linux-gnu";
      break;
  }

  let arch = "";
  switch (rawArchitecture) {
    case "x64":
      arch = "x86_64";
      break;
    case "arm64":
      arch = "aarch64";
      break;
  }

  if (rawOsType === "Linux") {
    if (libc.familySync() == "musl") {
      osType = "unknown-linux-musl-dynamic";
    } else if (libc.isNonGlibcLinuxSync()) {
      console.warn(
        "Your libc is neither glibc nor musl; trying static musl binary instead",
      );
      osType = "unknown-linux-musl-static";
    } else {
      let libcVersion = libc.versionSync();
      let splitLibcVersion = libcVersion.split(".");
      let libcMajorVersion = splitLibcVersion[0];
      let libcMinorVersion = splitLibcVersion[1];
      if (
        libcMajorVersion != builderGlibcMajorVersion ||
        libcMinorVersion < builderGlibcMInorVersion
      ) {
        // We can't run the glibc binaries, but we can run the static musl ones
        // if they exist
        console.warn(
          "Your glibc isn't compatible; trying static musl binary instead",
        );
        osType = "unknown-linux-musl-static";
      }
    }
  }

  // Assume the above succeeded and build a target triple to look things up with.
  // If any of it failed, this lookup will fail and we'll handle it like normal.
  let targetTriple = `${arch}-${osType}`;
  let platform = supportedPlatforms[targetTriple];

  if (!platform) {
    error(
      `Platform with type "${rawOsType}" and architecture "${rawArchitecture}" is not supported by ${name}.\nYour system must be one of the following:\n\n${Object.keys(
        supportedPlatforms,
      ).join(",")}`,
    );
  }

  return platform;
};

const getPackage = () => {
  const platform = getPlatform();
  const url = `${artifactDownloadUrl}/${platform.artifactName}`;
  let binary = new Package(name, url, platform.bins);

  return binary;
};

const install = (suppressLogs) => {
  if (!artifactDownloadUrl || artifactDownloadUrl.length === 0) {
    console.warn("in demo mode, not installing binaries");
    return;
  }
  const package = getPackage();
  const proxy = new HttpsProxyAgent(package.url);

  return package.install(proxy, suppressLogs);
};

const run = (binaryName) => {
  const package = getPackage();
  const proxy = configureProxy(package.url);

  // If no binaryName is specified, and there's one that matches the app, use that
  if (!binaryName && package.binaries[name]) {
    binaryName = name;
  }

  // If no binaryName is specified and there's only one binary, use that
  const binNames = Object.keys(package.binaries);
  if (!binaryName && binNames.length == 1) {
    binaryName = binNames[0];
  }

  package.run(binaryName, proxy);
};

module.exports = {
  install,
  run,
  getPackage,
};
