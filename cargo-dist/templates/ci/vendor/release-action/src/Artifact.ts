import { basename } from "path";
import {createReadStream, readFileSync, ReadStream, statSync} from "fs";

export class Artifact {
    readonly contentType: string
    readonly name: string
    readonly path: string

    constructor(path: string, contentType: string = "raw") {
        this.path = path
        this.name = basename(path)
        this.contentType = contentType;
    }

    get contentLength(): number {
        return statSync(this.path).size
    }

    readFile(): ReadStream {
        return createReadStream(this.path)
    }
}
