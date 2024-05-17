import { GithubError } from "../src/GithubError"

describe('ErrorMessage', () => {

    describe('has error with code', () => {
        const error = {
            message: 'something bad happened',
            errors: [
                {
                    code: 'missing',
                    resource: 'release'
                },
                {
                    code: 'already_exists',
                    resource: 'release'
                }
            ],
            status: 422
        }

        it('does not have error', () => {
            const githubError = new GithubError(error)
            expect(githubError.hasErrorWithCode('missing_field')).toBeFalsy()
        })

        it('has error', () => {
            const githubError = new GithubError(error)
            expect(githubError.hasErrorWithCode('missing')).toBeTruthy()
        })
    })

    describe('has error with remediation', () => {
        it('provides remediation with 404 without errors', () => {
            const error = { status: 404, message: "message" }
            const githubError = new GithubError(error)
            expect(githubError.toString())
                .toBe('Error 404: message\nMake sure your github token has access to the repo and has permission to author releases')
        })

        it('provides remediation with 404 with errors', () => {
            const error = {
                message: 'message',
                errors: [
                    {
                        code: 'missing',
                        resource: 'release'
                    }
                ],
                status: 404
            }
            const githubError = new GithubError(error)
            expect(githubError.toString())
                .toBe('Error 404: message\nErrors:\n- release does not exist.\nMake sure your github token has access to the repo and has permission to author releases')
        })
    })
    
    it('generates message with errors', () => {
        const error = {
            message: 'something bad happened',
            errors: [
                {
                    code: 'missing',
                    resource: 'release'
                },
                {
                    code: 'already_exists',
                    resource: 'release'
                }
            ],
            status: 422
        }

        const githubError = new GithubError(error)

        const expectedString = "Error 422: something bad happened\nErrors:\n- release does not exist.\n- release already exists."
        expect(githubError.toString()).toBe(expectedString)
    })

    it('generates message without errors', () => {
        const error = {
            message: 'something bad happened',
            status: 422
        }

        const githubError = new GithubError(error)

        expect(githubError.toString()).toBe('Error 422: something bad happened')
    })

    it('provides error status', () => {
        const error = { status: 404 }
        const githubError = new GithubError(error)
        expect(githubError.status).toBe(404)
    })
})
