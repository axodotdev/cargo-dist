import * as core from '@actions/core';
import {Context} from "@actions/github/lib/context";
import {readFileSync} from 'fs';
import {ArtifactGlobber} from './ArtifactGlobber';
import {Artifact} from './Artifact';

export interface Inputs {
    readonly allowUpdates: boolean
    readonly artifactErrorsFailBuild: boolean
    readonly artifacts: Artifact[]
    readonly commit?: string
    readonly createdDraft: boolean
    readonly createdPrerelease: boolean
    readonly createdReleaseBody?: string
    readonly createdReleaseName?: string
    readonly discussionCategory?: string
    readonly generateReleaseNotes: boolean
    readonly makeLatest?: "legacy" | "true" | "false" | undefined
    readonly owner: string
    readonly removeArtifacts: boolean
    readonly replacesArtifacts: boolean
    readonly repo: string
    readonly skipIfReleaseExists: boolean
    readonly tag: string
    readonly token: string
    readonly updatedDraft?: boolean
    readonly updatedReleaseBody?: string
    readonly updatedReleaseName?: string
    readonly updatedPrerelease?: boolean
    readonly updateOnlyUnreleased: boolean
}

export class CoreInputs implements Inputs {
    private artifactGlobber: ArtifactGlobber
    private context: Context

    constructor(artifactGlobber: ArtifactGlobber, context: Context) {
        this.artifactGlobber = artifactGlobber
        this.context = context
    }

    get allowUpdates(): boolean {
        const allow = core.getInput('allowUpdates')
        return allow == 'true'
    }

    get artifacts(): Artifact[] {
        let artifacts = core.getInput('artifacts')
        if (!artifacts) {
            artifacts = core.getInput('artifact')
        }
        if (artifacts) {
            let contentType = core.getInput('artifactContentType')
            if (!contentType) {
                contentType = 'raw'
            }
            return this.artifactGlobber
                .globArtifactString(artifacts, contentType, this.artifactErrorsFailBuild)
        }
        return []
    }

    get artifactErrorsFailBuild(): boolean {
        const allow = core.getInput('artifactErrorsFailBuild')
        return allow == 'true'
    }

    private get body(): string | undefined {
        const body = core.getInput('body')
        if (body) {
            return body
        }

        const bodyFile = core.getInput('bodyFile')
        if (bodyFile) {
            return this.stringFromFile(bodyFile)
        }

        return ''
    }

    get createdDraft(): boolean {
        const draft = core.getInput('draft')
        return draft == 'true'
    }

    get createdPrerelease(): boolean {
        const preRelease = core.getInput('prerelease')
        return preRelease == 'true'
    }

    get createdReleaseBody(): string | undefined {
        if (CoreInputs.omitBody) return undefined
        return this.body
    }

    private static get omitBody(): boolean {
        return core.getInput('omitBody') == 'true'
    }

    get createdReleaseName(): string | undefined {
        if (CoreInputs.omitName) return undefined
        return this.name
    }

    private static get omitName(): boolean {
        return core.getInput('omitName') == 'true'
    }

    get commit(): string | undefined {
        const commit = core.getInput('commit')
        if (commit) {
            return commit
        }
        return undefined
    }

    get discussionCategory(): string | undefined {
        const category = core.getInput('discussionCategory')
        if (category) {
            return category
        }
        return undefined
    }

    private get name(): string | undefined {
        const name = core.getInput('name')
        if (name) {
            return name
        }

        return this.tag
    }

    get generateReleaseNotes(): boolean {
        const generate = core.getInput('generateReleaseNotes')
        return generate == 'true'
    }

    get makeLatest(): "legacy" | "true" | "false" | undefined {
        let latest = core.getInput('makeLatest')
        if (latest == "true" || latest == "false" || latest == "legacy") {
            return latest;
        }
        
        return undefined
    }

    get owner(): string {
        let owner = core.getInput('owner')
        if (owner) {
            return owner
        }
        return this.context.repo.owner
    }

    get removeArtifacts(): boolean {
        const removes = core.getInput('removeArtifacts')
        return removes == 'true'
    }

    get replacesArtifacts(): boolean {
        const replaces = core.getInput('replacesArtifacts')
        return replaces == 'true'
    }

    get repo(): string {
        let repo = core.getInput('repo')
        if (repo) {
            return repo
        }
        return this.context.repo.repo
    }

    get skipIfReleaseExists(): boolean {
        return core.getBooleanInput("skipIfReleaseExists")
    }

    get tag(): string {
        const tag = core.getInput('tag')
        if (tag) {
            return tag;
        }

        const ref = this.context.ref
        const tagPath = "refs/tags/"
        if (ref && ref.startsWith(tagPath)) {
            return ref.substr(tagPath.length, ref.length)
        }

        throw Error("No tag found in ref or input!")
    }

    get token(): string {
        return core.getInput('token', {required: true})
    }

    get updatedDraft(): boolean | undefined {
        if (CoreInputs.omitDraftDuringUpdate) return undefined
        return this.createdDraft
    }

    private static get omitDraftDuringUpdate(): boolean {
        return core.getInput('omitDraftDuringUpdate') == 'true'
    }

    get updatedPrerelease(): boolean | undefined {
        if (CoreInputs.omitPrereleaseDuringUpdate) return undefined
        return this.createdPrerelease
    }

    private static get omitPrereleaseDuringUpdate(): boolean {
        return core.getInput('omitPrereleaseDuringUpdate') == 'true'
    }

    get updatedReleaseBody(): string | undefined {
        if (CoreInputs.omitBody || CoreInputs.omitBodyDuringUpdate) return undefined
        return this.body
    }

    private static get omitBodyDuringUpdate(): boolean {
        return core.getInput('omitBodyDuringUpdate') == 'true'
    }

    get updatedReleaseName(): string | undefined {
        if (CoreInputs.omitName || CoreInputs.omitNameDuringUpdate) return undefined
        return this.name
    }

    get updateOnlyUnreleased(): boolean {
        return core.getInput('updateOnlyUnreleased') == 'true'
    }

    private static get omitNameDuringUpdate(): boolean {
        return core.getInput('omitNameDuringUpdate') == 'true'
    }

    stringFromFile(path: string): string {
        return readFileSync(path, 'utf-8')
    }
}
