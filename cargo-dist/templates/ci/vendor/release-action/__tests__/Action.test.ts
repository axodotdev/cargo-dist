import {Action} from "../src/Action";
import {Artifact} from "../src/Artifact";
import {Inputs} from "../src/Inputs";
import {Releases} from "../src/Releases";
import {ArtifactUploader} from "../src/ArtifactUploader";
import {Outputs} from "../src/Outputs";
import {ArtifactDestroyer} from "../src/ArtifactDestroyer";
import {ActionSkipper} from "../src/ActionSkipper";

const applyReleaseDataMock = jest.fn()
const artifactDestroyMock = jest.fn()
const createMock = jest.fn()
const deleteMock = jest.fn()
const getMock = jest.fn()
const listArtifactsMock = jest.fn()
const listMock = jest.fn()
const shouldSkipMock = jest.fn()
const updateMock = jest.fn()
const uploadMock = jest.fn()

const artifacts = [
    new Artifact('a/art1'),
    new Artifact('b/art2')
]

const createBody = 'createBody'
const createDraft = true
const createName = 'createName'
const commit = 'commit'
const discussionCategory = 'discussionCategory'
const generateReleaseNotes = true
const id = 100
const createPrerelease = true
const releaseId = 101
const replacesArtifacts = true
const tag = 'tag'
const token = 'token'
const updateBody = 'updateBody'
const updateDraft = false
const updateName = 'updateName'
const updatePrerelease = false
const updateOnlyUnreleased = false
const url = 'http://api.example.com'
const makeLatest = 'legacy'

describe("Action", () => {
    beforeEach(() => {
        createMock.mockClear()
        getMock.mockClear()
        listMock.mockClear()
        shouldSkipMock.mockClear()
        updateMock.mockClear()
        uploadMock.mockClear()
    })

    it('creates release but does not upload if no artifact', async () => {
        const action = createAction(false, false)

        await action.perform()

        expect(createMock).toBeCalledWith(tag,
            createBody,
            commit,
            discussionCategory,
            createDraft,
            generateReleaseNotes,
            makeLatest,
            createName,
            createPrerelease)
        expect(uploadMock).not.toBeCalled()
        assertOutputApplied()
    })

    it('creates release if no release exists to update', async () => {
        const action = createAction(true, true)
        const error = {status: 404}
        getMock.mockRejectedValue(error)

        await action.perform()

        expect(createMock).toBeCalledWith(
            tag,
            createBody,
            commit,
            discussionCategory,
            createDraft,
            generateReleaseNotes,
            makeLatest,
            createName,
            createPrerelease)
        expect(uploadMock).toBeCalledWith(artifacts, releaseId, url)
        assertOutputApplied()
    })

    it('creates release if no draft releases', async () => {
        const action = createAction(true, true)
        const error = {status: 404}
        getMock.mockRejectedValue(error)
        listMock.mockResolvedValue({
            data: [
                {id: id, draft: false, tag_name: tag}
            ]
        })

        await action.perform()

        expect(createMock).toBeCalledWith(
            tag,
            createBody,
            commit,
            discussionCategory,
            createDraft,
            generateReleaseNotes,
            makeLatest,
            createName,
            createPrerelease
        )
        expect(uploadMock).toBeCalledWith(artifacts, releaseId, url)
        assertOutputApplied()

    })

    it('creates release then uploads artifact', async () => {
        const action = createAction(false, true)

        await action.perform()

        expect(createMock).toBeCalledWith(
            tag,
            createBody,
            commit,
            discussionCategory,
            createDraft,
            generateReleaseNotes,
            makeLatest,
            createName,
            createPrerelease
        )
        expect(uploadMock).toBeCalledWith(artifacts, releaseId, url)
        assertOutputApplied()
    })

    it('removes all artifacts when artifact destroyer is enabled', async () => {
        const action = createAction(false, true, true)

        await action.perform()

        expect(artifactDestroyMock).toBeCalledWith(releaseId)
        assertOutputApplied()
    })

    it('removes no artifacts when artifact destroyer is disabled', async () => {
        const action = createAction(false, true)

        await action.perform()

        expect(artifactDestroyMock).not.toBeCalled()
        assertOutputApplied()
    })

    it('skips action', async () => {
        const action = createAction(false, false, false)
        shouldSkipMock.mockResolvedValue(true)

        await action.perform()

        expect(createMock).not.toBeCalled()
        expect(updateMock).not.toBeCalled()
    })

    it('throws error when create fails', async () => {
        const action = createAction(false, true)
        createMock.mockRejectedValue("error")

        expect.hasAssertions()
        try {
            await action.perform()
        } catch (error) {
            expect(error).toEqual("error")
        }

        expect(createMock).toBeCalledWith(
            tag,
            createBody,
            commit,
            discussionCategory,
            createDraft,
            generateReleaseNotes,
            makeLatest,
            createName,
            createPrerelease
        )
        expect(uploadMock).not.toBeCalled()
    })

    it('throws error when get fails', async () => {
        const action = createAction(true, true)
        const error = {
            errors: [
                {
                    code: 'already_exists'
                }
            ]
        }

        createMock.mockRejectedValue(error)
        getMock.mockRejectedValue("error")
        expect.hasAssertions()
        try {
            await action.perform()
        } catch (error) {
            expect(error).toEqual("error")
        }

        expect(getMock).toBeCalledWith(tag)
        expect(updateMock).not.toBeCalled()
        expect(uploadMock).not.toBeCalled()

    })

    it('throws error when update fails', async () => {
        const action = createAction(true, true)

        updateMock.mockRejectedValue("error")

        expect.hasAssertions()
        try {
            await action.perform()
        } catch (error) {
            expect(error).toEqual("error")
        }

        expect(updateMock).toBeCalledWith(
            id,
            tag,
            updateBody,
            commit,
            discussionCategory,
            updateDraft,
            makeLatest,
            updateName,
            updatePrerelease
        )
        expect(uploadMock).not.toBeCalled()
    })

    it('throws error when upload fails', async () => {
        const action = createAction(false, true)
        const expectedError = {status: 404}
        uploadMock.mockRejectedValue(expectedError)

        expect.hasAssertions()
        try {
            await action.perform()
        } catch (error) {
            expect(error).toEqual(expectedError)
        }

        expect(createMock).toBeCalledWith(
            tag,
            createBody,
            commit,
            discussionCategory,
            createDraft,
            generateReleaseNotes,
            makeLatest,
            createName,
            createPrerelease
        )
        expect(uploadMock).toBeCalledWith(artifacts, releaseId, url)
    })

    it('updates draft release', async () => {
        const action = createAction(true, true)
        const error = {status: 404}
        getMock.mockRejectedValue(error)
        listMock.mockResolvedValue({
            data: [
                {id: 123, draft: false, tag_name: tag},
                {id: id, draft: true, tag_name: tag}
            ]
        })

        await action.perform()

        expect(updateMock).toBeCalledWith(
            id,
            tag,
            updateBody,
            commit,
            discussionCategory,
            updateDraft,
            makeLatest,
            updateName,
            updatePrerelease
        )
        expect(uploadMock).toBeCalledWith(artifacts, releaseId, url)
        assertOutputApplied()
    })

    it('updates release but does not upload if no artifact', async () => {
        const action = createAction(true, false)

        await action.perform()

        expect(updateMock).toBeCalledWith(
            id,
            tag,
            updateBody,
            commit,
            discussionCategory,
            updateDraft,
            makeLatest,
            updateName,
            updatePrerelease
        )
        expect(uploadMock).not.toBeCalled()
        assertOutputApplied()
    })

    it('updates release then uploads artifact', async () => {
        const action = createAction(true, true)

        await action.perform()

        expect(updateMock).toBeCalledWith(
            id,
            tag,
            updateBody,
            commit,
            discussionCategory,
            updateDraft,
            makeLatest,
            updateName,
            updatePrerelease
        )
        expect(uploadMock).toBeCalledWith(artifacts, releaseId, url)
        assertOutputApplied()
    })

    function assertOutputApplied() {
        expect(applyReleaseDataMock).toBeCalledWith({id: releaseId, upload_url: url})
    }

    function createAction(allowUpdates: boolean,
                          hasArtifact: boolean,
                          removeArtifacts: boolean = false): Action {
        let inputArtifact: Artifact[]
        if (hasArtifact) {
            inputArtifact = artifacts
        } else {
            inputArtifact = []
        }
        const MockReleases = jest.fn<Releases, any>(() => {
            return {
                create: createMock,
                deleteArtifact: deleteMock,
                getByTag: getMock,
                listArtifactsForRelease: listArtifactsMock,
                listReleases: listMock,
                update: updateMock,
                uploadArtifact: jest.fn()
            }
        })

        createMock.mockResolvedValue({
            data: {
                id: releaseId,
                upload_url: url
            }
        })
        getMock.mockResolvedValue({
            data: {
                id: id
            }
        })
        listMock.mockResolvedValue({
            data: []
        })
        shouldSkipMock.mockResolvedValue(false)
        updateMock.mockResolvedValue({
            data: {
                id: releaseId,
                upload_url: url
            }
        })
        uploadMock.mockResolvedValue({})

        const MockInputs = jest.fn<Inputs, any>(() => {
            return {
                allowUpdates: allowUpdates,
                artifactErrorsFailBuild: true,
                artifacts: inputArtifact,
                createdDraft: createDraft,
                createdReleaseBody: createBody,
                createdReleaseName: createName,
                commit: commit,
                discussionCategory: discussionCategory,
                generateReleaseNotes: true,
                makeLatest: makeLatest,
                owner: "owner",
                createdPrerelease: createPrerelease,
                replacesArtifacts: replacesArtifacts,
                removeArtifacts: removeArtifacts,
                repo: "repo",
                skipIfReleaseExists: false,
                tag: tag,
                token: token,
                updatedDraft: updateDraft,
                updatedReleaseBody: updateBody,
                updatedReleaseName: updateName,
                updatedPrerelease: updatePrerelease,
                updateOnlyUnreleased: updateOnlyUnreleased
            }
        })
        const MockOutputs = jest.fn<Outputs, any>(() => {
            return {
                applyReleaseData: applyReleaseDataMock
            }
        })
        const MockUploader = jest.fn<ArtifactUploader, any>(() => {
            return {
                uploadArtifacts: uploadMock
            }
        })
        const MockArtifactDestroyer = jest.fn<ArtifactDestroyer, any>(() => {
            return {
                destroyArtifacts: artifactDestroyMock
            }
        })
        
        const MockActionSkipper = jest.fn<ActionSkipper, any>(() => {
            return {
                shouldSkip: shouldSkipMock
            }
        })

        const inputs = new MockInputs()
        const outputs = new MockOutputs()
        const releases = new MockReleases()
        const uploader = new MockUploader()
        const artifactDestroyer = new MockArtifactDestroyer()
        const actionSkipper = new MockActionSkipper()

        return new Action(inputs, outputs, releases, uploader, artifactDestroyer, actionSkipper)
    }
})
