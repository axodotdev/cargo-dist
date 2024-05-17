import * as core from "@actions/core";

import { cleanTargetDir } from "./cleanup";
import { CacheConfig } from "./config";
import { getCacheProvider, reportError } from "./utils";

process.on("uncaughtException", (e) => {
  core.error(e.message);
  if (e.stack) {
    core.error(e.stack);
  }
});

async function run() {
  const cacheProvider = getCacheProvider();

  if (!cacheProvider.cache.isFeatureAvailable()) {
    setCacheHitOutput(false);
    return;
  }

  try {
    var cacheOnFailure = core.getInput("cache-on-failure").toLowerCase();
    if (cacheOnFailure !== "true") {
      cacheOnFailure = "false";
    }
    core.exportVariable("CACHE_ON_FAILURE", cacheOnFailure);
    core.exportVariable("CARGO_INCREMENTAL", 0);

    const config = await CacheConfig.new();
    config.printInfo(cacheProvider);
    core.info("");

    core.info(`... Restoring cache ...`);
    const key = config.cacheKey;
    // Pass a copy of cachePaths to avoid mutating the original array as reported by:
    // https://github.com/actions/toolkit/pull/1378
    // TODO: remove this once the underlying bug is fixed.
    const restoreKey = await cacheProvider.cache.restoreCache(config.cachePaths.slice(), key, [config.restoreKey]);
    if (restoreKey) {
      const match = restoreKey === key;
      core.info(`Restored from cache key "${restoreKey}" full match: ${match}.`);
      if (!match) {
        // pre-clean the target directory on cache mismatch
        for (const workspace of config.workspaces) {
          try {
            await cleanTargetDir(workspace.target, [], true);
          } catch {}
        }

        // We restored the cache but it is not a full match.
        config.saveState();
      }

      setCacheHitOutput(match);
    } else {
      core.info("No cache found.");
      config.saveState();

      setCacheHitOutput(false);
    }
  } catch (e) {
    setCacheHitOutput(false);

    reportError(e);
  }
  process.exit();
}

function setCacheHitOutput(cacheHit: boolean): void {
  core.setOutput("cache-hit", cacheHit.toString());
}

run();
