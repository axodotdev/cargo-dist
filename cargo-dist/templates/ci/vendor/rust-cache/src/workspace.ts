import * as core from "@actions/core";
import path from "path";

import { getCmdOutput } from "./utils";

const SAVE_TARGETS = new Set(["lib", "proc-macro"]);

export class Workspace {
  constructor(public root: string, public target: string) {}

  async getPackages(filter: ((p: Meta['packages'][0]) => boolean), ...extraArgs: string[]): Promise<Packages> {
    let packages: Packages = [];
    try {
      core.debug(`collecting metadata for "${this.root}"`);
      const meta: Meta = JSON.parse(
        await getCmdOutput("cargo", ["metadata", "--all-features", "--format-version", "1", ...extraArgs], {
          cwd: this.root,
        }),
      );
      core.debug(`workspace "${this.root}" has ${meta.packages.length} packages`);
      for (const pkg of meta.packages.filter(filter)) {
        const targets = pkg.targets.filter((t) => t.kind.some((kind) => SAVE_TARGETS.has(kind))).map((t) => t.name);
        packages.push({ name: pkg.name, version: pkg.version, targets, path: path.dirname(pkg.manifest_path) });
      }
    } catch (err) {
      console.error(err);
    }
    return packages;
  }

  public async getPackagesOutsideWorkspaceRoot(): Promise<Packages> {
    return await this.getPackages(pkg => !pkg.manifest_path.startsWith(this.root));
  }

  public async getWorkspaceMembers(): Promise<Packages> {
    return await this.getPackages(_ => true, "--no-deps");
  }
}

export interface PackageDefinition {
  name: string;
  version: string;
  path: string;
  targets: Array<string>;
}

export type Packages = Array<PackageDefinition>;

interface Meta {
  packages: Array<{
    name: string;
    version: string;
    manifest_path: string;
    targets: Array<{ kind: Array<string>; name: string }>;
  }>;
}
