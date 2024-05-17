const mockSetOutput = jest.fn();

import {CoreOutputs, Outputs} from "../src/Outputs";
import {ReleaseData} from "../src/Releases";

jest.mock('@actions/core', () => {
    return {setOutput: mockSetOutput};
})

describe('Outputs', () => {
    let outputs: Outputs;
    let releaseData: ReleaseData

    beforeEach(() => {
        outputs = new CoreOutputs()
        releaseData = {
            id: 1,
            html_url: 'https://api.example.com/assets',
            upload_url: 'https://api.example.com'
        }
    })

    it('Applies the release data to the action output', () => {
        outputs.applyReleaseData(releaseData)
        expect(mockSetOutput).toBeCalledWith('id', releaseData.id)
        expect(mockSetOutput).toBeCalledWith('html_url', releaseData.html_url)
        expect(mockSetOutput).toBeCalledWith('upload_url', releaseData.upload_url)
    })
})
