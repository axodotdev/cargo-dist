import * as core from '@actions/core';
import {ReleaseData} from "./Releases";

export interface Outputs {
    applyReleaseData(releaseData: ReleaseData): void
}

export class CoreOutputs implements Outputs {
    applyReleaseData(releaseData: ReleaseData) {
        core.setOutput('id', releaseData.id)
        core.setOutput('html_url', releaseData.html_url)
        core.setOutput('upload_url', releaseData.upload_url)
    }
}