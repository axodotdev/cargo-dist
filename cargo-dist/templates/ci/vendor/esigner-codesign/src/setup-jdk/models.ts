export interface ICorrettoAllAvailableVersions {
    [os: string]: {
        [arch: string]: {
            [distributionType: string]: {
                [version: string]: {
                    [fileType: string]: {
                        checksum: string;
                        checksum_sha256: string;
                        resource: string;
                    };
                };
            };
        };
    };
}

export interface ICorrettoAvailableVersions {
    version: string;
    fileType: string;
    checksum: string;
    checksum_sha256: string;
    resource: string;
    downloadLink: string;
    correttoVersion: string;
}
