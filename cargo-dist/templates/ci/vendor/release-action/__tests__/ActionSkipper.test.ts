import {ActionSkipper, ReleaseActionSkipper} from "../src/ActionSkipper";
import {Releases} from "../src/Releases";

describe("shouldSkip", () => {
    const getMock = jest.fn()
    const tag = "tag"
    const MockReleases = jest.fn<Releases, any>(() => {
        return {
            create: jest.fn(),
            deleteArtifact: jest.fn(),
            getByTag: getMock,
            listArtifactsForRelease: jest.fn(),
            listReleases: jest.fn(),
            update: jest.fn(),
            uploadArtifact: jest.fn()
        }
    })

    it('should return false when skipIfReleaseExists is false', async () => {
        const actionSkipper = new ReleaseActionSkipper(false, MockReleases(), tag)
        expect(await actionSkipper.shouldSkip()).toBe(false)
    })

    it('should return false when error occurs', async () => {
        getMock.mockRejectedValue(new Error())

        const actionSkipper = new ReleaseActionSkipper(true, MockReleases(), tag)
        expect(await actionSkipper.shouldSkip()).toBe(false)
    })

    it('should return false when release does not exist', async () => {
        getMock.mockResolvedValue({})
        
        const actionSkipper = new ReleaseActionSkipper(true, MockReleases(), tag)
        expect(await actionSkipper.shouldSkip()).toBe(false)
    })

    it('should return true when release does exist', async () => {
        getMock.mockResolvedValue({data: {}})

        const actionSkipper = new ReleaseActionSkipper(true, MockReleases(), tag)
        expect(await actionSkipper.shouldSkip()).toBe(true)
    })
})