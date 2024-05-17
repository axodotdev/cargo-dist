import {Artifact} from "../src/Artifact"
import {GithubArtifactUploader} from "../src/ArtifactUploader"
import {Releases} from "../src/Releases";
import {RequestError} from '@octokit/request-error'

const artifacts = [
    new Artifact('a/art1'),
    new Artifact('b/art2')
]
const fakeReadStream = {}
const contentLength = 42
const releaseId = 100
const url = 'http://api.example.com'

const deleteMock = jest.fn()
const listArtifactsMock = jest.fn()
const uploadMock = jest.fn()

jest.mock('fs', () => {
    return {
        promises: {},
        createReadStream: () => fakeReadStream,
        statSync: () => {
            return {size: contentLength}
        }
    };
})

describe('ArtifactUploader', () => {
    beforeEach(() => {
        deleteMock.mockClear()
        listArtifactsMock.mockClear()
        uploadMock.mockClear()
    })

    it('abort when upload failed with non-5xx response', async () => {
        mockListWithoutAssets()
        mockUploadArtifact(401, 2)
        const uploader = createUploader(true)

        await uploader.uploadArtifacts(artifacts, releaseId, url)

        expect(uploadMock).toBeCalledTimes(2)
        expect(uploadMock)
            .toBeCalledWith(url, contentLength, 'raw', fakeReadStream, 'art1', releaseId)
        expect(uploadMock)
            .toBeCalledWith(url, contentLength, 'raw', fakeReadStream, 'art2', releaseId)

        expect(deleteMock).toBeCalledTimes(0)
    })

    it('abort when upload failed with 5xx response after 3 attempts', async () => {
        mockListWithoutAssets()
        mockUploadArtifact(500, 4)
        const uploader = createUploader(true)

        await uploader.uploadArtifacts(artifacts, releaseId, url)

        expect(uploadMock).toBeCalledTimes(5)
        expect(uploadMock)
            .toBeCalledWith(url, contentLength, 'raw', fakeReadStream, 'art1', releaseId)
        expect(uploadMock)
            .toBeCalledWith(url, contentLength, 'raw', fakeReadStream, 'art1', releaseId)
        expect(uploadMock)
            .toBeCalledWith(url, contentLength, 'raw', fakeReadStream, 'art1', releaseId)
        expect(uploadMock)
            .toBeCalledWith(url, contentLength, 'raw', fakeReadStream, 'art2', releaseId)
        expect(uploadMock)
            .toBeCalledWith(url, contentLength, 'raw', fakeReadStream, 'art2', releaseId)

        expect(deleteMock).toBeCalledTimes(0)
    })

    it('replaces all artifacts', async () => {
        mockDeleteSuccess()
        mockListWithAssets()
        mockUploadArtifact()
        const uploader = createUploader(true)

        await uploader.uploadArtifacts(artifacts, releaseId, url)

        expect(uploadMock).toBeCalledTimes(2)
        expect(uploadMock)
            .toBeCalledWith(url, contentLength, 'raw', fakeReadStream, 'art1', releaseId)
        expect(uploadMock)
            .toBeCalledWith(url, contentLength, 'raw', fakeReadStream, 'art2', releaseId)

        expect(deleteMock).toBeCalledTimes(2)
        expect(deleteMock).toBeCalledWith(1)
        expect(deleteMock).toBeCalledWith(2)
    })

    it('replaces no artifacts when previous asset list empty', async () => {
        mockDeleteSuccess()
        mockListWithoutAssets()
        mockUploadArtifact()
        const uploader = createUploader(true)

        await uploader.uploadArtifacts(artifacts, releaseId, url)

        expect(uploadMock).toBeCalledTimes(2)
        expect(uploadMock)
            .toBeCalledWith(url, contentLength, 'raw', fakeReadStream, 'art1', releaseId)
        expect(uploadMock)
            .toBeCalledWith(url, contentLength, 'raw', fakeReadStream, 'art2', releaseId)

        expect(deleteMock).toBeCalledTimes(0)
    })

    it('retry when upload failed with 5xx response', async () => {
        mockListWithoutAssets()
        mockUploadArtifact(500, 2)
        const uploader = createUploader(true)

        await uploader.uploadArtifacts(artifacts, releaseId, url)

        expect(uploadMock).toBeCalledTimes(4)
        expect(uploadMock)
            .toBeCalledWith(url, contentLength, 'raw', fakeReadStream, 'art1', releaseId)
        expect(uploadMock)
            .toBeCalledWith(url, contentLength, 'raw', fakeReadStream, 'art1', releaseId)
        expect(uploadMock)
            .toBeCalledWith(url, contentLength, 'raw', fakeReadStream, 'art1', releaseId)
        expect(uploadMock)
            .toBeCalledWith(url, contentLength, 'raw', fakeReadStream, 'art2', releaseId)

        expect(deleteMock).toBeCalledTimes(0)
    })

    it('throws upload error when replacesExistingArtifacts is true', async () => {
        mockListWithoutAssets()
        mockUploadError()
        const uploader = createUploader(true, true)

        expect.hasAssertions()
        try {
            await uploader.uploadArtifacts(artifacts, releaseId, url)
        } catch (error) {
            expect(error).toEqual(Error("Failed to upload artifact art1. error."))
        }
    })

    it('throws error from replace', async () => {
        mockDeleteError()
        mockListWithAssets()
        mockUploadArtifact()
        const uploader = createUploader(true)

        expect.hasAssertions()
        try {
            await uploader.uploadArtifacts(artifacts, releaseId, url)
        } catch (error) {
            expect(error).toEqual("error")
        }
    })

    it('updates all artifacts, delete none', async () => {
        mockDeleteError()
        mockListWithAssets()
        mockUploadArtifact()
        const uploader = createUploader(false)

        await uploader.uploadArtifacts(artifacts, releaseId, url)

        expect(uploadMock).toBeCalledTimes(2)
        expect(uploadMock)
            .toBeCalledWith(url, contentLength, 'raw', fakeReadStream, 'art1', releaseId)
        expect(uploadMock)
            .toBeCalledWith(url, contentLength, 'raw', fakeReadStream, 'art2', releaseId)

        expect(deleteMock).toBeCalledTimes(0)
    })

    function createUploader(replaces: boolean, throws: boolean = false): GithubArtifactUploader {
        const MockReleases = jest.fn<Releases, any>(() => {
            return {
                create: jest.fn(),
                deleteArtifact: deleteMock,
                getByTag: jest.fn(),
                listArtifactsForRelease: listArtifactsMock,
                listReleases: jest.fn(),
                update: jest.fn(),
                uploadArtifact: uploadMock
            }
        })
        return new GithubArtifactUploader(new MockReleases(), replaces, throws)
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

    function mockUploadArtifact(status: number = 200, failures: number = 0) {
        const error = new RequestError(`HTTP ${status}`, status, {
            headers: {},
            request: {method: 'GET', url: '', headers: {}}
        })
        for (let index = 0; index < failures; index++) {
            uploadMock.mockRejectedValueOnce(error)
        }
        uploadMock.mockResolvedValue({})
    }

    function mockUploadError() {
        uploadMock.mockRejectedValue({
            message: "error",
            status: 502
        })
    }
});
