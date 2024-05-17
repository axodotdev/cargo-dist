export class GithubErrorDetail {
    private error: any;

    constructor(error: any) {
        this.error = error
    }

    get code(): string {
        return this.error.code
    }

    toString(): string {
        const code = this.error.code
        switch (code) {
            case 'missing':
                return this.missingResourceMessage()
            case 'missing_field':
                return this.missingFieldMessage()
            case 'invalid':
                return this.invalidFieldMessage()
            case 'already_exists':
                return this.resourceAlreadyExists()
            default:
                return this.customErrorMessage()
        }
    }

    private customErrorMessage(): string {
        const message = this.error.message;
        const documentation = this.error.documentation_url

        let documentationMessage: string
        if (documentation) {
            documentationMessage = `\nPlease see ${documentation}.`
        } else {
            documentationMessage = ""
        }

        return `${message}${documentationMessage}`
    }

    private invalidFieldMessage(): string {
        const resource = this.error.resource
        const field = this.error.field

        return `The ${field} field on ${resource} is an invalid format.`
    }

    private missingResourceMessage(): string {
        const resource = this.error.resource
        return `${resource} does not exist.`
    }

    private missingFieldMessage(): string {
        const resource = this.error.resource
        const field = this.error.field

        return `The ${field} field on ${resource} is missing.`
    }

    private resourceAlreadyExists(): string {
        const resource = this.error.resource
        return `${resource} already exists.`
    }
}
