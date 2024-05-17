import {ReleaseValidator} from "../src/ReleaseValidator";

describe("validateReleaseUpdate", () => {
    describe("updateOnlyUnreleased is disabled", () => {
        const validator = new ReleaseValidator(false)
        it('should not throw', () => {
            const releaseResponse = {
                draft: false,
                prerelease: false,
                name: "Name"
            }
            expect(() => {
                validator.validateReleaseUpdate(releaseResponse)
            }).not.toThrow()
        })
    })
    describe("updateOnlyUnreleased is enabled", () => {
        const validator = new ReleaseValidator(true)
        it('should throw if neither draft or prerelease are enabled', () => {
            const releaseResponse = {
                draft: false,
                prerelease: false,
                name: "Name"
            }
            expect(() => {
                validator.validateReleaseUpdate(releaseResponse)
            }).toThrow()
        })
        
        it('should not throw if draft is enabled', () => {
            const releaseResponse = {
                draft: true,
                prerelease: false,
                name: "Name"
            }
            expect(() => {
                validator.validateReleaseUpdate(releaseResponse)
            }).not.toThrow()
        })

        it('should not throw if prerelease is enabled', () => {
            const releaseResponse = {
                draft: false,
                prerelease: true,
                name: "Name"
            }
            expect(() => {
                validator.validateReleaseUpdate(releaseResponse)
            }).not.toThrow()
        })

        it('should not throw if draft & prerelease is enabled', () => {
            const releaseResponse = {
                draft: true,
                prerelease: true,
                name: "Name"
            }
            expect(() => {
                validator.validateReleaseUpdate(releaseResponse)
            }).not.toThrow()
        })

        it('should default error message release name to release', () => {
            const releaseResponse = {
                draft: false,
                prerelease: false,
                name: null
            }
            expect(() => {
                validator.validateReleaseUpdate(releaseResponse)
            }).toThrow(`Tried to update "release" which is neither a draft or prerelease. (updateOnlyUnreleased is on)`)
        })
    })
})