import * as github from '@actions/github';
import * as core from '@actions/core';
import {CoreInputs} from './Inputs';
import {GithubReleases} from './Releases';
import {Action} from './Action';
import {GithubArtifactUploader} from './ArtifactUploader';
import {FileArtifactGlobber} from './ArtifactGlobber';
import {GithubError} from './GithubError';
import {CoreOutputs} from "./Outputs";
import {GithubArtifactDestroyer} from "./ArtifactDestroyer";
import {ActionSkipper, ReleaseActionSkipper} from "./ActionSkipper";

async function run() {
    try {
        const action = createAction()
        await action.perform()
    } catch (error) {
        const githubError = new GithubError(error)
        core.setFailed(githubError.toString());
    }
}

function createAction(): Action {
    const token = core.getInput('token')
    const context = github.context
    const git = github.getOctokit(token)
    const globber = new FileArtifactGlobber()

    const inputs = new CoreInputs(globber, context)
    const outputs = new CoreOutputs()
    const releases = new GithubReleases(inputs, git)
    const skipper = new ReleaseActionSkipper(inputs.skipIfReleaseExists, releases, inputs.tag)
    const uploader = new GithubArtifactUploader(releases, inputs.replacesArtifacts, inputs.artifactErrorsFailBuild)
    const artifactDestroyer = new GithubArtifactDestroyer(releases)
    
    return new Action(inputs, outputs, releases, uploader, artifactDestroyer, skipper)
}

run();
