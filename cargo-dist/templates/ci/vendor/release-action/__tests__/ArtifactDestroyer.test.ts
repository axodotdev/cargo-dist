import {Artifact} from "../src/Artifact"
import {GithubArtifactUploader} from "../src/ArtifactUploader"
import {Releases} from "../src/Releases";
import {RequestError} from '@octokit/request-error'
import {GithubArtifactDestroyer} from "../src/ArtifactDestroyer";

const releaseId = 100

const deleteMock = jest.fn()
const listArtifactsMock = jest.fn()


describe('ArtifactDestroyer', () => {
    beforeEach(() => {
        deleteMock.mockClear()
        listArtifactsMock.mockClear()
    })

    it('destroys all artifacts', async () => {
        mockListWithAssets()
        mockDeleteSuccess()
        const destroyer = createDestroyer()

        await destroyer.destroyArtifacts(releaseId)

        expect(deleteMock).toBeCalledTimes(2)
    })

    it('destroys nothing when no artifacts found', async () => {
        mockListWithoutAssets()
        const destroyer = createDestroyer()

        await destroyer.destroyArtifacts(releaseId)

        expect(deleteMock).toBeCalledTimes(0)
    })

    it('throws when delete call fails', async () => {
        mockListWithAssets()
        mockDeleteError()
        const destroyer = createDestroyer()
        
        expect.hasAssertions()
        try {
            await destroyer.destroyArtifacts(releaseId)
        } catch (error) {
            expect(error).toEqual("error")
        }
    })

    function createDestroyer(): GithubArtifactDestroyer {
        const MockReleases = jest.fn<Releases, any>(() => {
            return {
                create: jest.fn(),
                deleteArtifact: deleteMock,
                getByTag: jest.fn(),
                listArtifactsForRelease: listArtifactsMock,
                listReleases: jest.fn(),
                update: jest.fn(),
                uploadArtifact: jest.fn()
            }
        })
        return new GithubArtifactDestroyer(new MockReleases())
    }

    function mockDeleteError(): any {
        deleteMock.mockRejectedValue("error")
    }

    function mockDeleteSuccess(): any {
        deleteMock.mockResolvedValue({})
    }

    function mockListWithAssets() {
        listArtifactsMock.mockResolvedValue([
            {
                name: "art1",
                id: 1
            },
            {
                name: "art2",
                id: 2
            }
        ])
    }

    function mockListWithoutAssets() {
        listArtifactsMock.mockResolvedValue([])
    }
});
