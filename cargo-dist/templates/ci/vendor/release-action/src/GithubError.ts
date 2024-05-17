import {GithubErrorDetail} from "./GithubErrorDetail"

export class GithubError {
    private error: any
    private readonly githubErrors: GithubErrorDetail[]

    constructor(error: any) {
        this.error = error
        this.githubErrors = this.generateGithubErrors()
    }

    private generateGithubErrors(): GithubErrorDetail[] {
        const errors = this.error.errors
        if (errors instanceof Array) {
            return errors.map((err) => new GithubErrorDetail(err))
        } else {
            return []
        }
    }

    get status(): number {
        return this.error.status
    }

    hasErrorWithCode(code: String): boolean {
        return this.githubErrors.some((err) => err.code == code)
    }

    toString(): string {
        const message = this.error.message
        const errors = this.githubErrors
        const status = this.status
        if (errors.length > 0) {
            return `Error ${status}: ${message}\nErrors:\n${this.errorBulletedList(errors)}${this.remediation()}`
        } else {
            return `Error ${status}: ${message}${this.remediation()}`
        }
    }

    private errorBulletedList(errors: GithubErrorDetail[]): string {
        return errors.map((err) => `- ${err}`).join("\n")
    }
    
    private remediation(): String {
        if (this.status == 404) {
            return "\nMake sure your github token has access to the repo and has permission to author releases"
        }
        return ""
    }
}

