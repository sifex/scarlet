import { XMLParser } from "fast-xml-parser";
import { FileDownload } from './types';
import * as Path from "node:path";
import * as path from "node:path";

/**
 * Fetches XML data from a URL and converts it to an array of FileDownload objects
 * @param url The URL to fetch the XML data from
 * @returns A Promise resolving to an array of FileDownload objects
 */
export async function fetchAndConvertXML(url: string): Promise<FileDownload[]> {
    try {
        const response = await fetch(url);
        const xmlData = await response.text();

        const parser = new XMLParser();
        const jsonObj = parser.parse(xmlData);

        const fileDownloads: FileDownload[] = [];
        const root_url = url.split('/').slice(0, -1).join('/');

        if (Array.isArray(jsonObj.theupdates.file)) {
            jsonObj.theupdates.file.forEach((file: any) => {
                fileDownloads.push({
                    // TODO Remove the leading slash from the file name

                    url: Path.join(root_url, removeLeadingSlash(file.name)),
                    path: file.name,
                    sha256_hash: file.hash
                });
            });
        } else if (jsonObj.theupdates.file) {
            fileDownloads.push({
                url: Path.join(root_url, removeLeadingSlash(jsonObj.theupdates.file.name)),
                path: jsonObj.theupdates.file.name,
                sha256_hash: jsonObj.theupdates.file.hash
            });
        }

        return fileDownloads;
    } catch (error) {
        console.error('Error fetching or parsing XML:', error);
        throw error;
    }
}

function removeLeadingSlash(pathString: string) {
    // Normalize the path to use forward slashes and remove any trailing slashes
    const normalizedPath = path.normalize(pathString).replace(/\\/g, '/').replace(/\/$/, '');

    // Use path.parse to get the components of the path
    const parsedPath = path.parse(normalizedPath);

    // Reconstruct the path without the leading slash
    return parsedPath.dir.substring(1) + '/' + parsedPath.base;
}



export function getKeywordArguments(): Record<string, string | boolean> {
    const args = process.argv.slice(2); // Remove the first two elements (Electron and script path)
    const keywordArgs: Record<string, string | boolean> = {};

    for (let i = 0; i < args.length; i++) {
        const arg = args[i];

        if (arg.startsWith('--')) {
            const keyValue = arg.slice(2).split('=');
            if (keyValue.length === 2) {
                // Handle --key=value format
                keywordArgs[keyValue[0]] = keyValue[1].replace(/^["']|["']$/g, ''); // Remove surrounding quotes if present
            } else if (i + 1 < args.length && !args[i + 1].startsWith('--')) {
                // Handle --key value format
                keywordArgs[keyValue[0]] = args[i + 1].replace(/^["']|["']$/g, ''); // Remove surrounding quotes if present
                i++; // Skip the next argument since we've used it as the value
            } else {
                // Handle --flag format (boolean flag)
                keywordArgs[keyValue[0]] = true;
            }
        }
    }

    return keywordArgs;
}