import {GitHub} from '@actions/github/lib/utils'
import {OctokitResponse} from "@octokit/types";
import {RestEndpointMethodTypes} from "@octokit/plugin-rest-endpoint-methods";
import {Inputs} from "./Inputs";

export type CreateReleaseResponse = RestEndpointMethodTypes["repos"]["createRelease"]["response"]
export type ReleaseByTagResponse = RestEndpointMethodTypes["repos"]["getReleaseByTag"]["response"]
export type ListReleasesResponse = RestEndpointMethodTypes["repos"]["listReleases"]["response"]
export type ListReleaseAssetsResponseData = RestEndpointMethodTypes["repos"]["listReleaseAssets"]["response"]["data"]
export type UpdateReleaseResponse = RestEndpointMethodTypes["repos"]["updateRelease"]["response"]
export type UploadArtifactResponse = RestEndpointMethodTypes["repos"]["uploadReleaseAsset"]["response"]
export type CreateOrUpdateReleaseResponse = CreateReleaseResponse | UpdateReleaseResponse

export type ReleaseData = {
    id: number
    html_url: string
    upload_url: string
}

export interface Releases {
    create(
        tag: string,
        body?: string,
        commitHash?: string,
        discussionCategory?: string,
        draft?: boolean,
        generateReleaseNotes?: boolean,
        makeLatest?:  "legacy" | "true" | "false" | undefined,
        name?: string,
        prerelease?: boolean
    ): Promise<CreateReleaseResponse>

    deleteArtifact(assetId: number): Promise<OctokitResponse<any>>

    getByTag(tag: string): Promise<ReleaseByTagResponse>

    listArtifactsForRelease(releaseId: number): Promise<ListReleaseAssetsResponseData>

    listReleases(): Promise<ListReleasesResponse>

    update(
        id: number,
        tag: string,
        body?: string,
        commitHash?: string,
        discussionCategory?: string,
        draft?: boolean,
        makeLatest?:  "legacy" | "true" | "false" | undefined,
        name?: string,
        prerelease?: boolean
    ): Promise<UpdateReleaseResponse>

    uploadArtifact(
        assetUrl: string,
        contentLength: number,
        contentType: string,
        file: string | object,
        name: string,
        releaseId: number,
    ): Promise<UploadArtifactResponse>
}

export class GithubReleases implements Releases {
    git: InstanceType<typeof GitHub>
    inputs: Inputs

    constructor(inputs: Inputs, git: InstanceType<typeof GitHub>) {
        this.inputs = inputs
        this.git = git
    }

    async create(
        tag: string,
        body?: string,
        commitHash?: string,
        discussionCategory?: string,
        draft?: boolean,
        generateReleaseNotes?: boolean,
        makeLatest?:  "legacy" | "true" | "false" | undefined,
        name?: string,
        prerelease?: boolean
    ): Promise<CreateReleaseResponse> {
        // noinspection TypeScriptValidateJSTypes
        return this.git.rest.repos.createRelease({
            body: body,
            name: name,
            discussion_category_name: discussionCategory,
            draft: draft,
            generate_release_notes: generateReleaseNotes,
            make_latest: makeLatest,
            owner: this.inputs.owner,
            prerelease: prerelease,
            repo: this.inputs.repo,
            target_commitish: commitHash,
            tag_name: tag
        })
    }

    async deleteArtifact(
        assetId: number
    ): Promise<OctokitResponse<any>> {
        return this.git.rest.repos.deleteReleaseAsset({
            asset_id: assetId,
            owner: this.inputs.owner,
            repo: this.inputs.repo
        })
    }

    async getByTag(tag: string): Promise<ReleaseByTagResponse> {
        return this.git.rest.repos.getReleaseByTag({
            owner: this.inputs.owner,
            repo: this.inputs.repo,
            tag: tag
        })
    }

    async listArtifactsForRelease(
        releaseId: number
    ): Promise<ListReleaseAssetsResponseData> {
        return this.git.paginate(this.git.rest.repos.listReleaseAssets, {
            owner: this.inputs.owner,
            release_id: releaseId,
            repo: this.inputs.repo
        })
    }

    async listReleases(): Promise<ListReleasesResponse> {
        return this.git.rest.repos.listReleases({
            owner: this.inputs.owner,
            repo: this.inputs.repo
        })
    }

    async update(
        id: number,
        tag: string,
        body?: string,
        commitHash?: string,
        discussionCategory?: string,
        draft?: boolean,
        makeLatest?:  "legacy" | "true" | "false" | undefined,
        name?: string,
        prerelease?: boolean
    ): Promise<UpdateReleaseResponse> {
        // noinspection TypeScriptValidateJSTypes
        return this.git.rest.repos.updateRelease({
            release_id: id,
            body: body,
            name: name,
            discussion_category_name: discussionCategory,
            draft: draft,
            make_latest: makeLatest,
            owner: this.inputs.owner,
            prerelease: prerelease,
            repo: this.inputs.repo,
            target_commitish: commitHash,
            tag_name: tag
        })
    }

    async uploadArtifact(
        assetUrl: string,
        contentLength: number,
        contentType: string,
        file: string | object,
        name: string,
        releaseId: number,
    ): Promise<UploadArtifactResponse> {
        return this.git.rest.repos.uploadReleaseAsset({
            url: assetUrl,
            headers: {
                "content-length": contentLength,
                "content-type": contentType
            },
            data: file as any,
            name: name,
            owner: this.inputs.owner,
            release_id: releaseId,
            repo: this.inputs.repo
        })
    }
}
