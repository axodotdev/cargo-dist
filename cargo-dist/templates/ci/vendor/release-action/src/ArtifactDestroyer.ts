import {Releases} from "./Releases";
import * as core from "@actions/core";

export interface ArtifactDestroyer {
    destroyArtifacts(releaseId: number): Promise<void>
}

export class GithubArtifactDestroyer implements ArtifactDestroyer {
    constructor(private releases: Releases) {
    }

    async destroyArtifacts(releaseId: number): Promise<void> {
        const releaseAssets = await this.releases.listArtifactsForRelease(releaseId)
        for (const artifact of releaseAssets) {
            const asset = artifact
            core.debug(`Deleting existing artifact ${artifact.name}...`)
            await this.releases.deleteArtifact(asset.id)
        }
    }
}