import {globSync} from "glob";


export interface Globber {
    glob(pattern: string): string[]
}

export class FileGlobber implements Globber {
    glob(pattern: string): string[] {
        return globSync(pattern, { mark: true })
    }
}