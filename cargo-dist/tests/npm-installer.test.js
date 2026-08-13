const assert = require("node:assert/strict");
const { mkdtempSync, readFileSync, rmSync } = require("node:fs");
const http = require("node:http");
const { tmpdir } = require("node:os");
const { join } = require("node:path");
const { once } = require("node:events");
const test = require("node:test");

const {
  downloadToFile,
  downloadWithRetry,
  isTransientDownloadError,
} = require("../templates/installer/npm/binary-install");

test("classifies transient download failures", () => {
  for (const statusCode of [408, 429, 500, 502, 503, 504, 599]) {
    const err = new Error(`HTTP ${statusCode}`);
    err.statusCode = statusCode;
    assert.equal(isTransientDownloadError(err), true, `HTTP ${statusCode}`);
  }

  for (const code of [
    "EAI_AGAIN",
    "ECONNRESET",
    "ETIMEDOUT",
    "ERR_STREAM_PREMATURE_CLOSE",
  ]) {
    const err = new Error(code);
    err.code = code;
    assert.equal(isTransientDownloadError(err), true, code);
  }

  assert.equal(isTransientDownloadError(new Error("HTTP 404")), false);
  assert.equal(isTransientDownloadError(new Error("HTTP 600")), false);
});

test("retries transient failures with exponential backoff", async () => {
  const delays = [];
  let attempts = 0;

  const result = await downloadWithRetry(
    async () => {
      attempts++;
      if (attempts < 4) {
        const err = new Error("HTTP 503");
        err.statusCode = 503;
        throw err;
      }
      return "downloaded";
    },
    {
      sleepFn: async (delayMs) => delays.push(delayMs),
    },
  );

  assert.equal(result, "downloaded");
  assert.equal(attempts, 4);
  assert.deepEqual(delays, [1000, 2000, 4000]);
});

test("does not retry permanent failures", async () => {
  let attempts = 0;

  await assert.rejects(
    downloadWithRetry(async () => {
      attempts++;
      const err = new Error("HTTP 404");
      err.statusCode = 404;
      throw err;
    }),
    /HTTP 404/,
  );

  assert.equal(attempts, 1);
});

test("retries HTTP and mid-stream failures for the complete transfer", async () => {
  const contents = Buffer.from("complete artifact contents");
  let attempts = 0;
  const server = http.createServer((request, response) => {
    attempts++;

    if (attempts === 1) {
      response.writeHead(503);
      response.end("temporarily unavailable");
      return;
    }

    response.writeHead(200, { "Content-Length": contents.length });
    if (attempts === 2) {
      const socket = response.socket;
      response.write(contents.subarray(0, 5));
      setImmediate(() => socket.destroy());
      return;
    }

    response.end(contents);
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");

  const directory = mkdtempSync(join(tmpdir(), "cargo-dist-npm-test-"));
  const outputPath = join(directory, "artifact.tar.gz");

  try {
    const { port } = server.address();
    await downloadWithRetry(
      () => downloadToFile(`http://127.0.0.1:${port}/artifact`, outputPath),
      { sleepFn: async () => {} },
    );

    assert.equal(attempts, 3);
    assert.deepEqual(readFileSync(outputPath), contents);
  } finally {
    server.close();
    await once(server, "close");
    rmSync(directory, { recursive: true, force: true });
  }
});
