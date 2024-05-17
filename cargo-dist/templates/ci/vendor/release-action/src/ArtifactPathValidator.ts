import * as core from "@actions/core";
import {statSync} from "fs";

export class ArtifactPathValidator {
    private readonly errorsFailBuild: boolean;
    private paths: string[];
    private readonly pattern: string
    
    constructor(errorsFailBuild: boolean, paths: string[], pattern: string) {
        this.paths = paths;
        this.pattern = pattern
        this.errorsFailBuild = errorsFailBuild;
    }
    
    validate(): string[] {
        this.verifyPathsNotEmpty()
        return this.paths.filter((path) => this.verifyNotDirectory(path))
    }
    
    private verifyPathsNotEmpty() {
        if (this.paths.length == 0) {
            const message = `Artifact pattern:${this.pattern} did not match any files`
            this.reportError(message)
        }
    }
    
    private verifyNotDirectory(path: string): boolean {
        const isDir = statSync(path).isDirectory()
        if (isDir) {
            const message = `Artifact is a directory:${path}. Directories can not be uploaded to a release.`
            this.reportError(message)
        }
        return !isDir
    }
    
    private reportError(message: string) {
        if (this.errorsFailBuild) {
            throw Error(message)
        } else {
            core.warning(message)
        }
    }
}