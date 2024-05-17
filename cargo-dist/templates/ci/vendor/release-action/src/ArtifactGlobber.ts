import * as core from '@actions/core';
import {Globber, FileGlobber} from "./Globber";
import {Artifact} from "./Artifact";
import untildify from "untildify";
import {ArtifactPathValidator} from "./ArtifactPathValidator";
import {PathNormalizer} from "./PathNormalizer";

export interface ArtifactGlobber {
    globArtifactString(artifact: string, contentType: string, errorsFailBuild: boolean): Artifact[]
}

export class FileArtifactGlobber implements ArtifactGlobber {
    private globber: Globber

    constructor(globber: Globber = new FileGlobber()) {
        this.globber = globber
    }

    globArtifactString(artifact: string, contentType: string, errorsFailBuild: boolean): Artifact[] {
        const split = /[,\n]/
        return artifact.split(split)
            .map(path => path.trimStart())
            .map(path => PathNormalizer.normalizePath(path))
            .map(path => FileArtifactGlobber.expandPath(path))
            .map(pattern => this.globPattern(pattern, errorsFailBuild))
            .map((globResult) => FileArtifactGlobber.validatePattern(errorsFailBuild, globResult[1], globResult[0]))
            .reduce((accumulated, current) => accumulated.concat(current))
            .map(path => new Artifact(path, contentType))
    }

    private globPattern(pattern: string, errorsFailBuild: boolean): [string, string[]] {
        const paths = this.globber.glob(pattern)
        if (paths.length == 0) {
            if (errorsFailBuild) {
                FileArtifactGlobber.throwGlobError(pattern)
            } else {
                FileArtifactGlobber.reportGlobWarning(pattern)
            }
        }
        return [pattern, paths]
    }

    private static validatePattern(errorsFailBuild: boolean, paths: string[], pattern: string): string[] {
        const validator = new ArtifactPathValidator(errorsFailBuild, paths, pattern)
        return validator.validate()
    }

    private static reportGlobWarning(pattern: string) {
        core.warning(`Artifact pattern :${pattern} did not match any files`)
    }

    private static throwGlobError(pattern: string) {
        throw Error(`Artifact pattern :${pattern} did not match any files`)
    }

    private static expandPath(path: string): string {
        return untildify(path)
    }
}