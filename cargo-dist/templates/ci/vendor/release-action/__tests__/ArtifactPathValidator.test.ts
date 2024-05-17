const directoryMock = jest.fn()
const warnMock = jest.fn()

import {ArtifactPathValidator} from "../src/ArtifactPathValidator";

const pattern = 'pattern'

jest.mock('@actions/core', () => {
    return {warning: warnMock};
})

jest.mock('fs', () => {
    return {
        statSync: () => {
            return {isDirectory: directoryMock}
        }
    };
})

describe("ArtifactPathValidator", () => {
    beforeEach(() => {
        warnMock.mockClear()
        directoryMock.mockClear()
    })

    it("warns and filters out path which points to a directory", () => {
        const paths = ['path1', 'path2']
        directoryMock.mockReturnValueOnce(true).mockReturnValueOnce(false)

        const validator = new ArtifactPathValidator(false, paths, pattern)

        const result = validator.validate()
        expect(warnMock).toBeCalled()
        expect(result).toEqual(['path2'])
    })

    it("warns when no glob results are produced and empty results shouldn't throw", () => {
        const validator = new ArtifactPathValidator(false, [], pattern)
        const result = validator.validate()
        expect(warnMock).toBeCalled()
    })

    it("throws when no glob results are produced and empty results shouild throw", () => {
        const validator = new ArtifactPathValidator(true, [], pattern)
        expect(() => {
            validator.validate()
        }).toThrow()
    })

    it("throws when path points to directory", () => {
        const paths = ['path1', 'path2']
        directoryMock.mockReturnValueOnce(true).mockReturnValueOnce(false)

        const validator = new ArtifactPathValidator(true, paths, pattern)

        expect(() => {
            validator.validate()
        }).toThrow()
    })
})