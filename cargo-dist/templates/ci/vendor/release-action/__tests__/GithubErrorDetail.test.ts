import { GithubErrorDetail } from "../src/GithubErrorDetail"

describe('GithubErrorDetail', () => {

    it('provides error code', () => {
        const error = {
            code: "missing"
        }

        const detail = new GithubErrorDetail(error)

        expect(detail.code).toBe('missing')
    })

    it('generates missing resource error message', () => {
        const resource = "release"
        const error = {
            code: "missing",
            resource: resource
        }

        const detail = new GithubErrorDetail(error)
        const message = detail.toString()

        expect(message).toBe(`${resource} does not exist.`)
    })

    it('generates missing field error message', () => {
        const resource = "release"
        const field = "body"
        const error = {
            code: "missing_field",
            field: field,
            resource: resource
        }

        const detail = new GithubErrorDetail(error)
        const message = detail.toString()

        expect(message).toBe(`The ${field} field on ${resource} is missing.`)
    })

    it('generates invalid field error message', () => {
        const resource = "release"
        const field = "body"
        const error = {
            code: "invalid",
            field: field,
            resource: resource
        }

        const detail = new GithubErrorDetail(error)
        const message = detail.toString()

        expect(message).toBe(`The ${field} field on ${resource} is an invalid format.`)
    })

    it('generates resource already exists error message', () => {
        const resource = "release"
        const error = {
            code: "already_exists",
            resource: resource
        }

        const detail = new GithubErrorDetail(error)
        const message = detail.toString()

        expect(message).toBe(`${resource} already exists.`)
    })

    describe('generates custom error message', () => {
        it('with documentation url', () => {
            const url = "https://api.example.com"
            const error = {
                code: "custom",
                message: "foo",
                documentation_url: url
            }

            const detail = new GithubErrorDetail(error)
            const message = detail.toString()

            expect(message).toBe(`foo\nPlease see ${url}.`)
        })

        it('without documentation url', () => {
            const error = {
                code: "custom",
                message: "foo"
            }

            const detail = new GithubErrorDetail(error)
            const message = detail.toString()

            expect(message).toBe('foo')
        })
    })
})