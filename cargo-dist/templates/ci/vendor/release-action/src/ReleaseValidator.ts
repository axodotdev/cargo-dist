export class ReleaseValidator {
    constructor(private updateOnlyUnreleased: boolean) {
    }

    validateReleaseUpdate(releaseResponse: ReleaseStageArguments) {
        if (!this.updateOnlyUnreleased) {
            return
        }

        if (!releaseResponse.draft && !releaseResponse.prerelease) {
            throw new Error(`Tried to update "${releaseResponse.name ?? "release"}" which is neither a draft or prerelease. (updateOnlyUnreleased is on)`)
        }
    }
}

export type ReleaseStageArguments = {
    draft: boolean
    name: string | null
    prerelease: boolean
}