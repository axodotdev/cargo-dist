import path from "path";

export class PathNormalizer {
    static normalizePath(pathString: string): string {
        return pathString.split(path.sep).join("/")
    } 
}