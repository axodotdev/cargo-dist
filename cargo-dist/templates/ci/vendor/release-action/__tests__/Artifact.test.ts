import {Artifact} from "../src/Artifact";

const contentLength = 42
const fakeReadStream = {}

jest.mock('fs', () => {
    return {
        createReadStream: () => fakeReadStream,
        statSync: () => {
            return {size: contentLength}
        }
    };
})

describe("Artifact", () => {
    it('defaults contentType to raw', () => {
        const artifact = new Artifact('')
        expect(artifact.contentType).toBe('raw')
    })

    it('generates name from path', () => {
        const artifact = new Artifact('some/artifact')
        expect(artifact.name).toBe('artifact')
    })

    it('provides contentLength', () => {
        const artifact = new Artifact('some/artifact')
        expect(artifact.contentLength).toBe(contentLength)
    })

    it('provides path', () => {
        const artifact = new Artifact('some/artifact')
        expect(artifact.path).toBe('some/artifact')
    })

    it('reads artifact', () => {
        const artifact = new Artifact('some/artifact')
        expect(artifact.readFile()).toBe(fakeReadStream)
    })
})