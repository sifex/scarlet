/**
 * Represents a file to be downloaded
 */
export interface FileDownload {
    url: string;
    path: string;
    sha256_hash: string;
}

