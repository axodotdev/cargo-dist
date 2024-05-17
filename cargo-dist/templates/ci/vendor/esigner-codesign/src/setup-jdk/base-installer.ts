import * as tc from '@actions/tool-cache';
import * as core from '@actions/core';
import * as fs from 'fs';
import semver from 'semver';
import path from 'path';
import * as httpm from '@actions/http-client';
import { getToolCachePath, isVersionSatisfies } from '../util';
import { JavaDownloadRelease, JavaInstallerResults } from './base-models';
import { MACOS_JAVA_CONTENT_POSTFIX } from '../constants';
import os from 'os';

export abstract class JavaBase {
    protected http: httpm.HttpClient;
    protected version: string;
    protected architecture: string;
    protected packageType: string;
    protected stable: boolean;
    protected checkLatest: boolean;

    protected constructor(protected distribution: string) {
        this.http = new httpm.HttpClient('actions-codesign', undefined, {
            allowRetries: true,
            maxRetries: 3
        });

        ({ version: this.version, stable: this.stable } = this.normalizeVersion('11'));
        this.architecture = os.arch();
        this.packageType = 'jdk';
        this.checkLatest = false;
    }

    protected abstract downloadTool(javaRelease: JavaDownloadRelease): Promise<JavaInstallerResults>;

    protected abstract findPackageForDownload(range: string): Promise<JavaDownloadRelease>;

    public async setup(): Promise<JavaInstallerResults> {
        let foundJava = this.findInToolCache();
        if (foundJava && !this.checkLatest) {
            core.info(`Resolved Java ${foundJava.version} from tool-cache`);
        } else {
            core.info('Trying to resolve the latest version from remote');
            const javaRelease = await this.findPackageForDownload(this.version);
            core.info(`Resolved latest version as ${javaRelease.version}`);
            if (foundJava?.version === javaRelease.version) {
                core.info(`Resolved Java ${foundJava.version} from tool-cache`);
            } else {
                core.info('Trying to download...');
                foundJava = await this.downloadTool(javaRelease);
                core.info(`Java ${foundJava.version} was downloaded`);
            }
        }

        // JDK folder may contain postfix "Contents/Home" on macOS
        const macOSPostfixPath = path.join(foundJava.path, MACOS_JAVA_CONTENT_POSTFIX);
        if (process.platform === 'darwin' && fs.existsSync(macOSPostfixPath)) {
            foundJava.path = macOSPostfixPath;
        }

        core.info(`Setting Java ${foundJava.version} as the default`);
        this.setJavaDefault(foundJava.version, foundJava.path);

        return foundJava;
    }

    protected get toolCacheFolderName(): string {
        return `Java_${this.distribution}_${this.packageType}`;
    }

    protected getToolCacheVersionName(version: string): string {
        if (!this.stable) {
            if (version.includes('+')) {
                return version.replace('+', '-ea.');
            } else {
                return `${version}-ea`;
            }
        }
        return version.replace('+', '-');
    }

    protected findInToolCache(): JavaInstallerResults | null {
        const availableVersions = tc
            .findAllVersions(this.toolCacheFolderName, this.architecture)
            .map(item => {
                return {
                    version: item.replace('-ea.', '+').replace(/-ea$/, '').replace('-', '+'),
                    path: getToolCachePath(this.toolCacheFolderName, item, this.architecture) || '',
                    stable: !item.includes('-ea')
                };
            })
            .filter(item => item.stable === this.stable);

        const satisfiedVersions = availableVersions
            .filter(item => isVersionSatisfies(this.version, item.version))
            .filter(item => item.path)
            .sort((a, b) => {
                return -semver.compareBuild(a.version, b.version);
            });
        if (!satisfiedVersions || satisfiedVersions.length === 0) {
            return null;
        }

        return {
            version: satisfiedVersions[0].version,
            path: satisfiedVersions[0].path
        };
    }

    protected normalizeVersion(version: string) {
        let stable = true;

        if (version.endsWith('-ea')) {
            version = version.replace(/-ea$/, '');
            stable = false;
        } else if (version.includes('-ea.')) {
            version = version.replace('-ea.', '+');
            stable = false;
        }

        if (!semver.validRange(version)) {
            throw new Error(`The string '${version}' is not valid SemVer notation for a Java version. Please check README file for code snippets and more detailed information`);
        }

        return {
            version,
            stable
        };
    }

    protected setJavaDefault(version: string, toolPath: string) {
        const majorVersion = version.split('.')[0];
        core.exportVariable('JAVA_HOME', toolPath);
        core.addPath(path.join(toolPath, 'bin'));
        core.setOutput('distribution', this.distribution);
        core.setOutput('path', toolPath);
        core.setOutput('version', version);
        core.exportVariable(`JAVA_HOME_${majorVersion}_${this.architecture.toUpperCase()}`, toolPath);
        core.exportVariable(`JAVA_VERSION`, majorVersion);
    }

    protected distributionArchitecture(): string {
        switch (this.architecture) {
            case 'amd64':
                return 'x64';
            case 'ia32':
                return 'x86';
            case 'arm64':
                return 'aarch64';
            default:
                return this.architecture;
        }
    }
}
