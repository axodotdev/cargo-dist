import * as core from "@actions/core";
import * as glob from "@actions/glob";
import crypto from "crypto";
import fs from "fs";
import fs_promises from "fs/promises";
import os from "os";
import path from "path";
import * as toml from "smol-toml";

import { getCargoBins } from "./cleanup";
import { CacheProvider, exists, getCmdOutput } from "./utils";
import { Workspace } from "./workspace";

const HOME = os.homedir();
export const CARGO_HOME = process.env.CARGO_HOME || path.join(HOME, ".cargo");

const STATE_CONFIG = "RUST_CACHE_CONFIG";
const HASH_LENGTH = 8;

export class CacheConfig {
  /** All the paths we want to cache */
  public cachePaths: Array<string> = [];
  /** The primary cache key */
  public cacheKey = "";
  /** The secondary (restore) key that only contains the prefix and environment */
  public restoreKey = "";

  /** The workspace configurations */
  public workspaces: Array<Workspace> = [];

  /** The cargo binaries present during main step */
  public cargoBins: Array<string> = [];

  /** The prefix portion of the cache key */
  private keyPrefix = "";
  /** The rust version considered for the cache key */
  private keyRust = "";
  /** The environment variables considered for the cache key */
  private keyEnvs: Array<string> = [];
  /** The files considered for the cache key */
  private keyFiles: Array<string> = [];

  private constructor() {}

  /**
   * Constructs a [`CacheConfig`] with all the paths and keys.
   *
   * This will read the action `input`s, and read and persist `state` as necessary.
   */
  static async new(): Promise<CacheConfig> {
    const self = new CacheConfig();

    // Construct key prefix:
    // This uses either the `shared-key` input,
    // or the `key` input combined with the `job` key.

    let key = core.getInput("prefix-key") || "v0-rust";

    const sharedKey = core.getInput("shared-key");
    if (sharedKey) {
      key += `-${sharedKey}`;
    } else {
      const inputKey = core.getInput("key");
      if (inputKey) {
        key += `-${inputKey}`;
      }

      const job = process.env.GITHUB_JOB;
      if (job) {
        key += `-${job}`;
      }
    }

    self.keyPrefix = key;

    // Construct environment portion of the key:
    // This consists of a hash that considers the rust version
    // as well as all the environment variables as given by a default list
    // and the `env-vars` input.
    // The env vars are sorted, matched by prefix and hashed into the
    // resulting environment hash.

    let hasher = crypto.createHash("sha1");
    const rustVersion = await getRustVersion();

    let keyRust = `${rustVersion.release} ${rustVersion.host}`;
    hasher.update(keyRust);
    hasher.update(rustVersion["commit-hash"]);

    keyRust += ` (${rustVersion["commit-hash"]})`;
    self.keyRust = keyRust;

    // these prefixes should cover most of the compiler / rust / cargo keys
    const envPrefixes = ["CARGO", "CC", "CFLAGS", "CXX", "CMAKE", "RUST"];
    envPrefixes.push(...core.getInput("env-vars").split(/\s+/).filter(Boolean));

    // sort the available env vars so we have a more stable hash
    const keyEnvs = [];
    const envKeys = Object.keys(process.env);
    envKeys.sort((a, b) => a.localeCompare(b));
    for (const key of envKeys) {
      const value = process.env[key];
      if (envPrefixes.some((prefix) => key.startsWith(prefix)) && value) {
        hasher.update(`${key}=${value}`);
        keyEnvs.push(key);
      }
    }

    self.keyEnvs = keyEnvs;

    key += `-${digest(hasher)}`;

    self.restoreKey = key;

    // Construct the lockfiles portion of the key:
    // This considers all the files found via globbing for various manifests
    // and lockfiles.

    // Constructs the workspace config and paths to restore:
    // The workspaces are given using a `$workspace -> $target` syntax.

    const workspaces: Array<Workspace> = [];
    const workspacesInput = core.getInput("workspaces") || ".";
    for (const workspace of workspacesInput.trim().split("\n")) {
      let [root, target = "target"] = workspace.split("->").map((s) => s.trim());
      root = path.resolve(root);
      target = path.join(root, target);
      workspaces.push(new Workspace(root, target));
    }
    self.workspaces = workspaces;

    let keyFiles = await globFiles(".cargo/config.toml\nrust-toolchain\nrust-toolchain.toml");
    const parsedKeyFiles = []; // keyFiles that are parsed, pre-processed and hashed

    hasher = crypto.createHash("sha1");

    for (const workspace of workspaces) {
      const root = workspace.root;
      keyFiles.push(
        ...(await globFiles(
          `${root}/**/.cargo/config.toml\n${root}/**/rust-toolchain\n${root}/**/rust-toolchain.toml`,
        )),
      );

      const workspaceMembers = await workspace.getWorkspaceMembers();

      const cargo_manifests = sort_and_uniq(workspaceMembers.map(member => path.join(member.path, "Cargo.toml")));

      for (const cargo_manifest of cargo_manifests) {
        try {
          const content = await fs_promises.readFile(cargo_manifest, { encoding: "utf8" });
          // Use any since TomlPrimitive is not exposed
          const parsed = toml.parse(content) as { [key: string]: any };

          if ("package" in parsed) {
            const pack = parsed.package;
            if ("version" in pack) {
              pack["version"] = "0.0.0";
            }
          }

          for (const prefix of ["", "build-", "dev-"]) {
            const section_name = `${prefix}dependencies`;
            if (!(section_name in parsed)) {
              continue;
            }
            const deps = parsed[section_name];

            for (const key of Object.keys(deps)) {
              const dep = deps[key];

              try {
                if ("path" in dep) {
                  dep.version = "0.0.0";
                  dep.path = "";
                }
              } catch (_e) {
                // Not an object, probably a string (version),
                // continue.
                continue;
              }
            }
          }

          hasher.update(JSON.stringify(parsed));

          parsedKeyFiles.push(cargo_manifest);
        } catch (e) { // Fallback to caching them as regular file
          core.warning(`Error parsing Cargo.toml manifest, fallback to caching entire file: ${e}`);
          keyFiles.push(cargo_manifest);
        }
      }

      const cargo_lock = path.join(workspace.root, "Cargo.lock");
      if (await exists(cargo_lock)) {
        try {
          const content = await fs_promises.readFile(cargo_lock, { encoding: "utf8" });
          const parsed = toml.parse(content);

          if (parsed.version !== 3 || !("package" in parsed)) {
            // Fallback to caching them as regular file since this action
            // can only handle Cargo.lock format version 3
            core.warning('Unsupported Cargo.lock format, fallback to caching entire file');
            keyFiles.push(cargo_lock);
            continue;
          }

          // Package without `[[package]].source` and `[[package]].checksum`
          // are the one with `path = "..."` to crates within the workspace.
          const packages = (parsed.package as any[]).filter((p: any) => "source" in p || "checksum" in p);

          hasher.update(JSON.stringify(packages));

          parsedKeyFiles.push(cargo_lock);
        } catch (e) { // Fallback to caching them as regular file
          core.warning(`Error parsing Cargo.lock manifest, fallback to caching entire file: ${e}`);
          keyFiles.push(cargo_lock);
        }
      }
    }
    keyFiles = sort_and_uniq(keyFiles);

    for (const file of keyFiles) {
      for await (const chunk of fs.createReadStream(file)) {
        hasher.update(chunk);
      }
    }

    let lockHash = digest(hasher);

    keyFiles.push(...parsedKeyFiles);
    self.keyFiles = sort_and_uniq(keyFiles);

    key += `-${lockHash}`;
    self.cacheKey = key;

    self.cachePaths = [CARGO_HOME];
    const cacheTargets = core.getInput("cache-targets").toLowerCase() || "true";
    if (cacheTargets === "true") {
      self.cachePaths.push(...workspaces.map((ws) => ws.target));
    }

    const cacheDirectories = core.getInput("cache-directories");
    for (const dir of cacheDirectories.trim().split(/\s+/).filter(Boolean)) {
      self.cachePaths.push(dir);
    }

    const bins = await getCargoBins();
    self.cargoBins = Array.from(bins.values());

    return self;
  }

  /**
   * Reads and returns the cache config from the action `state`.
   *
   * @throws {Error} if the state is not present.
   * @returns {CacheConfig} the configuration.
   * @see {@link CacheConfig#saveState}
   * @see {@link CacheConfig#new}
   */
  static fromState(): CacheConfig {
    const source = core.getState(STATE_CONFIG);
    if (!source) {
      throw new Error("Cache configuration not found in state");
    }

    const self = new CacheConfig();
    Object.assign(self, JSON.parse(source));
    self.workspaces = self.workspaces.map((w: any) => new Workspace(w.root, w.target));

    return self;
  }

  /**
   * Prints the configuration to the action log.
   */
  printInfo(cacheProvider: CacheProvider) {
    core.startGroup("Cache Configuration");
    core.info(`Cache Provider:`);
    core.info(`    ${cacheProvider.name}`);
    core.info(`Workspaces:`);
    for (const workspace of this.workspaces) {
      core.info(`    ${workspace.root}`);
    }
    core.info(`Cache Paths:`);
    for (const path of this.cachePaths) {
      core.info(`    ${path}`);
    }
    core.info(`Restore Key:`);
    core.info(`    ${this.restoreKey}`);
    core.info(`Cache Key:`);
    core.info(`    ${this.cacheKey}`);
    core.info(`.. Prefix:`);
    core.info(`  - ${this.keyPrefix}`);
    core.info(`.. Environment considered:`);
    core.info(`  - Rust Version: ${this.keyRust}`);
    for (const env of this.keyEnvs) {
      core.info(`  - ${env}`);
    }
    core.info(`.. Lockfiles considered:`);
    for (const file of this.keyFiles) {
      core.info(`  - ${file}`);
    }
    core.endGroup();
  }

  /**
   * Saves the configuration to the state store.
   * This is used to restore the configuration in the post action.
   */
  saveState() {
    core.saveState(STATE_CONFIG, this);
  }
}

/**
 * Checks if the cache is up to date.
 *
 * @returns `true` if the cache is up to date, `false` otherwise.
 */
export function isCacheUpToDate(): boolean {
  return core.getState(STATE_CONFIG) === "";
}

/**
 * Returns a hex digest of the given hasher truncated to `HASH_LENGTH`.
 *
 * @param hasher The hasher to digest.
 * @returns The hex digest.
 */
function digest(hasher: crypto.Hash): string {
  return hasher.digest("hex").substring(0, HASH_LENGTH);
}

interface RustVersion {
  host: string;
  release: string;
  "commit-hash": string;
}

async function getRustVersion(): Promise<RustVersion> {
  const stdout = await getCmdOutput("rustc", ["-vV"]);
  let splits = stdout
    .split(/[\n\r]+/)
    .filter(Boolean)
    .map((s) => s.split(":").map((s) => s.trim()))
    .filter((s) => s.length === 2);
  return Object.fromEntries(splits);
}

async function globFiles(pattern: string): Promise<string[]> {
  const globber = await glob.create(pattern, {
    followSymbolicLinks: false,
  });
  // fs.statSync resolve the symbolic link and returns stat for the
  // file it pointed to, so isFile would make sure the resolved
  // file is actually a regular file.
  return (await globber.glob()).filter((file) => fs.statSync(file).isFile());
}

function sort_and_uniq(a: string[]) {
  return a
    .sort((a, b) => a.localeCompare(b))
    .reduce((accumulator: string[], currentValue: string) => {
      const len = accumulator.length;
      // If accumulator is empty or its last element != currentValue
      // Since array is already sorted, elements with the same value
      // are grouped together to be continugous in space.
      //
      // If currentValue != last element, then it must be unique.
      if (len == 0 || accumulator[len - 1].localeCompare(currentValue) != 0) {
        accumulator.push(currentValue);
      }
      return accumulator;
    }, []);
}
