const {
  createWriteStream,
  existsSync,
  mkdirSync,
  mkdtemp,
  rmSync,
} = require("fs");
const { join, sep } = require("path");
const { spawnSync } = require("child_process");
const { tmpdir } = require("os");
const { pipeline } = require("node:stream");

const https = require("node:https");
const http = require("node:http");

const tmpDir = tmpdir();
const DOWNLOAD_MAX_ATTEMPTS = 4;
const DOWNLOAD_RETRY_BASE_DELAY_MS = 1000;
const TRANSIENT_NETWORK_ERROR_CODES = new Set([
  "EAI_AGAIN",
  "ECONNABORTED",
  "ECONNREFUSED",
  "ECONNRESET",
  "EHOSTUNREACH",
  "ENETUNREACH",
  "ERR_STREAM_PREMATURE_CLOSE",
  "ETIMEDOUT",
]);

const error = (msg) => {
  console.error(msg);
  process.exit(1);
};

function httpError(statusCode, message) {
  const err = new Error(message);
  err.statusCode = statusCode;
  return err;
}

function isTransientDownloadError(err) {
  const message = err && err.message ? err.message : "";
  const statusMatch = /\bHTTP (\d{3})\b/.exec(message);
  const statusCode = Number(err && err.statusCode) || Number(statusMatch?.[1]);

  if (statusCode) {
    return (
      statusCode === 408 ||
      statusCode === 429 ||
      (statusCode >= 500 && statusCode < 600)
    );
  }

  return (
    TRANSIENT_NETWORK_ERROR_CODES.has(err && err.code) ||
    /socket hang up|network|timed? ?out|aborted|premature close/i.test(message)
  );
}

function sleep(delayMs) {
  return new Promise((resolve) => setTimeout(resolve, delayMs));
}

async function downloadWithRetry(
  operation,
  {
    maxAttempts = DOWNLOAD_MAX_ATTEMPTS,
    baseDelayMs = DOWNLOAD_RETRY_BASE_DELAY_MS,
    sleepFn = sleep,
    onRetry = () => {},
  } = {},
) {
  for (let attempt = 1; ; attempt++) {
    try {
      return await operation();
    } catch (err) {
      if (attempt >= maxAttempts || !isTransientDownloadError(err)) {
        throw err;
      }

      const delayMs = baseDelayMs * 2 ** (attempt - 1);
      onRetry(err, attempt, delayMs);
      await sleepFn(delayMs);
    }
  }
}

function getProxyForUrl(urlString) {
  const url = new URL(urlString);
  const isHttps = url.protocol === "https:";

  const noProxy = process.env.NO_PROXY || process.env.no_proxy || "";
  if (noProxy === "*") return null;
  if (noProxy) {
    const hostname = url.hostname.toLowerCase();
    const noProxyList = noProxy.split(",").map((s) => s.trim().toLowerCase());
    for (const entry of noProxyList) {
      if (hostname === entry || hostname.endsWith("." + entry)) {
        return null;
      }
    }
  }

  const proxyEnv = isHttps
    ? process.env.HTTPS_PROXY || process.env.https_proxy
    : process.env.HTTP_PROXY || process.env.http_proxy;

  if (!proxyEnv) return null;

  const proxyUrl = new URL(proxyEnv);

  let auth = null;
  if (proxyUrl.username || proxyUrl.password) {
    auth = `${proxyUrl.username}:${proxyUrl.password}`;
  }

  return {
    hostname: proxyUrl.hostname,
    port: proxyUrl.port || (proxyUrl.protocol === "https:" ? 443 : 80),
    auth: auth,
  };
}

function connectThroughProxy(proxy, target) {
  return new Promise((resolve, reject) => {
    const headers = {};
    if (proxy.auth) {
      headers["Proxy-Authorization"] =
        "Basic " + Buffer.from(proxy.auth).toString("base64");
    }

    const connectReq = http.request({
      hostname: proxy.hostname,
      port: proxy.port,
      method: "CONNECT",
      path: `${target.hostname}:${target.port || 443}`,
      headers,
    });
    connectReq.on("connect", (res, socket) => {
      if (res.statusCode === 200) {
        resolve(socket);
      } else {
        socket.destroy();
        reject(
          httpError(
            res.statusCode,
            `Proxy CONNECT failed with HTTP ${res.statusCode}`,
          ),
        );
      }
    });
    connectReq.on("error", reject);
    connectReq.end();
  });
}

function download(urlString, maxRedirects) {
  if (maxRedirects === undefined) maxRedirects = 5;
  return new Promise((resolve, reject) => {
    if (maxRedirects < 0) {
      return reject(new Error("Too many redirects"));
    }

    const parsed = new URL(urlString);
    const isHttps = parsed.protocol === "https:";
    const mod = isHttps ? https : http;
    const proxy = getProxyForUrl(urlString);

    const doRequest = (extraOptions) => {
      const options = Object.assign(
        {
          hostname: parsed.hostname,
          port: parsed.port || (isHttps ? 443 : 80),
          path: parsed.pathname + parsed.search,
          method: "GET",
          headers: { "User-Agent": "cargo-dist-npm-installer" },
        },
        extraOptions || {},
      );

      if (proxy && !isHttps) {
        // HTTP through HTTP proxy: request the full URL via the proxy
        options.hostname = proxy.hostname;
        options.port = proxy.port;
        options.path = urlString;
        if (proxy.auth) {
          options.headers["Proxy-Authorization"] =
            "Basic " + Buffer.from(proxy.auth).toString("base64");
        }
      }

      const req = mod.request(options, (res) => {
        if (
          res.statusCode >= 300 &&
          res.statusCode < 400 &&
          res.headers.location
        ) {
          res.resume();
          const nextUrl = new URL(res.headers.location, urlString).toString();
          return download(nextUrl, maxRedirects - 1).then(resolve, reject);
        }
        if (res.statusCode < 200 || res.statusCode >= 300) {
          res.resume();
          return reject(
            httpError(
              res.statusCode,
              `HTTP ${res.statusCode} from ${urlString}`,
            ),
          );
        }
        resolve(res);
      });
      req.on("error", reject);
      req.end();
    };

    if (proxy && isHttps) {
      connectThroughProxy(proxy, parsed).then(
        (socket) => doRequest({ socket, agent: false }),
        reject,
      );
    } else {
      doRequest();
    }
  });
}

function downloadToFile(urlString, path) {
  return download(urlString).then((res) => {
    return new Promise((resolve, reject) => {
      pipeline(res, createWriteStream(path), (err) => {
        if (err) {
          reject(err);
        } else {
          resolve();
        }
      });
    });
  });
}

class Package {
  constructor(platform, name, url, filename, zipExt, binaries) {
    let errors = [];
    if (typeof url !== "string") {
      errors.push("url must be a string");
    } else {
      try {
        new URL(url);
      } catch (e) {
        errors.push(e);
      }
    }
    if (name && typeof name !== "string") {
      errors.push("package name must be a string");
    }
    if (!name) {
      errors.push("You must specify the name of your package");
    }
    if (binaries && typeof binaries !== "object") {
      errors.push("binaries must be a string => string map");
    }
    if (!binaries) {
      errors.push("You must specify the binaries in the package");
    }

    if (errors.length > 0) {
      let errorMsg =
        "One or more of the parameters you passed to the Binary constructor are invalid:\n";
      errors.forEach((error) => {
        errorMsg += error;
      });
      errorMsg +=
        '\n\nCorrect usage: new Package("my-binary", "https://example.com/binary/download.tar.gz", {"my-binary": "my-binary"})';
      error(errorMsg);
    }

    this.platform = platform;
    this.url = url;
    this.name = name;
    this.filename = filename;
    this.zipExt = zipExt;
    this.installDirectory = join(__dirname, "node_modules", ".bin_real");
    this.binaries = binaries;

    if (!existsSync(this.installDirectory)) {
      mkdirSync(this.installDirectory, { recursive: true });
    }
  }

  exists() {
    for (const binaryName in this.binaries) {
      const binRelPath = this.binaries[binaryName];
      const binPath = join(this.installDirectory, binRelPath);
      if (!existsSync(binPath)) {
        return false;
      }
    }
    return true;
  }

  install(suppressLogs = false) {
    if (this.exists()) {
      if (!suppressLogs) {
        console.error(
          `${this.name} is already installed, skipping installation.`,
        );
      }
      return Promise.resolve();
    }

    try {
      rmSync(this.installDirectory, { recursive: true, force: true });
    } catch {
      // ignore - directory may not exist
    }

    mkdirSync(this.installDirectory, { recursive: true });

    if (!suppressLogs) {
      console.error(`Downloading release from ${this.url}`);
    }

    let tempDirectory;
    return new Promise((resolve, reject) => {
      mkdtemp(`${tmpDir}${sep}`, (err, directory) => {
        if (err) {
          reject(err);
        } else {
          resolve(directory);
        }
      });
    })
      .then((directory) => {
        tempDirectory = directory;
        const tempFile = join(directory, this.filename);
        return downloadWithRetry(() => downloadToFile(this.url, tempFile), {
          onRetry: (err, attempt, delayMs) => {
            if (!suppressLogs) {
              console.error(
                `Download attempt ${attempt} failed (${err.message}); retrying in ${delayMs}ms...`,
              );
            }
          },
        }).then(() => tempFile);
      })
      .then((tempFile) => {
        let result;
        if (/\.tar\.*/.test(this.zipExt)) {
          result = spawnSync("tar", [
            "xf",
            tempFile,
            // The tarballs are stored with a leading directory
            // component; we strip one component in the
            // shell installers too.
            "--strip-components",
            "1",
            "-C",
            this.installDirectory,
          ]);
        } else if (this.zipExt == ".zip") {
          if (this.platform.artifactName.includes("windows")) {
            // Windows does not have "unzip" by default on many installations, instead
            // we use Expand-Archive from powershell
            result = spawnSync("powershell.exe", [
              "-NoProfile",
              "-NonInteractive",
              "-Command",
              `& {
                        param([string]$LiteralPath, [string]$DestinationPath)
                        Expand-Archive -LiteralPath $LiteralPath -DestinationPath $DestinationPath -Force
                    }`,
              tempFile,
              this.installDirectory,
            ]);
          } else {
            result = spawnSync("unzip", [
              "-q",
              tempFile,
              "-d",
              this.installDirectory,
            ]);
          }
        } else {
          throw new Error(`Unrecognized file extension: ${this.zipExt}`);
        }

        if (result.status == 0) {
          return;
        }
        if (result.error) {
          throw result.error;
        }

        throw new Error(
          `An error occurred extracting the artifact: stdout: ${result.stdout}; stderr: ${result.stderr}`,
        );
      })
      .finally(() => {
        if (tempDirectory) {
          try {
            rmSync(tempDirectory, { recursive: true, force: true });
          } catch {
            // ignore temporary directory cleanup failures
          }
        }
      })
      .then(() => {
        if (!suppressLogs) {
          console.error(`${this.name} has been installed!`);
        }
      })
      .catch((e) => {
        error(`Error fetching release: ${e.message}`);
      });
  }

  run(binaryName) {
    const promise = !this.exists() ? this.install(true) : Promise.resolve();

    promise
      .then(() => {
        const [, , ...args] = process.argv;

        const options = { cwd: process.cwd(), stdio: "inherit" };

        const binRelPath = this.binaries[binaryName];
        if (!binRelPath) {
          error(`${binaryName} is not a known binary in ${this.name}`);
        }
        const binPath = join(this.installDirectory, binRelPath);
        const result = spawnSync(binPath, args, options);

        if (result.error) {
          error(result.error);
        }

        process.exit(result.status);
      })
      .catch((e) => {
        error(e.message);
      });
  }
}

module.exports = {
  Package,
  downloadToFile,
  downloadWithRetry,
  isTransientDownloadError,
};
