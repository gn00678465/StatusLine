"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");
const http = require("node:http");
const https = require("node:https");
const os = require("node:os");
const path = require("node:path");
const zlib = require("node:zlib");

const PACKAGE_VERSION = require("./package.json").version;
const REPOSITORY_URL = "https://github.com/gn00678465/StatusLine";
const RELEASE_DOWNLOAD_URL = `${REPOSITORY_URL}/releases/latest/download`;
const NPM_TARBALL_URL = `${RELEASE_DOWNLOAD_URL}/cc-statusline-npm.tgz`;
const NPM_INSTALL_COMMAND = `npm install -g --allow-remote=all --allow-scripts=${NPM_TARBALL_URL} ${NPM_TARBALL_URL}`;
const SHELL_INSTALL_COMMAND = `curl -fsSL ${RELEASE_DOWNLOAD_URL}/install.sh | sh`;
const BINARY_NAME = process.platform === "win32" ? "cc-statusline.exe" : "cc-statusline";

function assetFor(platform, architecture) {
    const assets = {
        "darwin-arm64": "cc-statusline-darwin-arm64.tar.gz",
        "darwin-x64": "cc-statusline-darwin-x64.tar.gz",
        "linux-arm64": "cc-statusline-linux-arm64-musl.tar.gz",
        "linux-x64": "cc-statusline-linux-x64-musl.tar.gz",
        "win32-arm64": "cc-statusline-win32-arm64.zip",
        "win32-x64": "cc-statusline-win32-x64.zip",
    };

    const asset = assets[`${platform}-${architecture}`];
    if (!asset) {
        throw new Error(`unsupported platform: ${platform}-${architecture}`);
    }

    return asset;
}

function releaseBaseUrl() {
    const configured = process.env.CC_STATUSLINE_BASE_URL;
    if (configured) {
        return configured.replace(/\/$/, "");
    }

    return `${REPOSITORY_URL}/releases/download/v${PACKAGE_VERSION}`;
}

function download(url, redirectsRemaining = 5) {
    return new Promise((resolve, reject) => {
        const parsed = new URL(url);
        const client = parsed.protocol === "https:" ? https : http;
        if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
            reject(new Error(`unsupported download protocol: ${parsed.protocol}`));
            return;
        }

        const request = client.get(parsed, (response) => {
            const status = response.statusCode || 0;
            if (status >= 300 && status < 400 && response.headers.location) {
                response.resume();
                if (redirectsRemaining === 0) {
                    reject(new Error("download exceeded redirect limit"));
                    return;
                }
                download(new URL(response.headers.location, parsed).toString(), redirectsRemaining - 1)
                    .then(resolve, reject);
                return;
            }
            if (status !== 200) {
                response.resume();
                reject(new Error(`download returned HTTP ${status}`));
                return;
            }

            const chunks = [];
            response.on("data", (chunk) => chunks.push(chunk));
            response.on("end", () => resolve(Buffer.concat(chunks)));
            response.on("error", reject);
        });
        request.setTimeout(30_000, () => request.destroy(new Error("download timed out")));
        request.on("error", reject);
    });
}

function verifiedArchive(archive, checksum, assetName) {
    const match = checksum
        .toString("utf8")
        .split(/\r?\n/)
        .map((line) => line.match(/^([a-fA-F0-9]{64}) {2}(.+)$/))
        .find((entry) => entry && entry[2] === assetName);
    if (!match) {
        throw new Error(`invalid checksum sidecar for ${assetName}`);
    }

    const actual = crypto.createHash("sha256").update(archive).digest("hex");
    if (actual !== match[1].toLowerCase()) {
        throw new Error(`SHA-256 verification failed for ${assetName}`);
    }

    return archive;
}

function tarEntryName(header) {
    const end = header.indexOf(0);
    return header.subarray(0, end === -1 ? header.length : end).toString("utf8");
}

function tarEntrySize(header) {
    const field = header.subarray(124, 136).toString("ascii").replace(/\0/g, "").trim();
    return field ? Number.parseInt(field, 8) : 0;
}

function binaryFromTarGz(archive, binaryName) {
    const tar = zlib.gunzipSync(archive);
    for (let offset = 0; offset + 512 <= tar.length; ) {
        const header = tar.subarray(offset, offset + 512);
        const name = tarEntryName(header);
        if (!name) {
            break;
        }
        const size = tarEntrySize(header);
        if (!Number.isSafeInteger(size) || size < 0 || offset + 512 + size > tar.length) {
            throw new Error("invalid tar archive");
        }
        const type = header[156];
        if ((type === 0 || type === "0".charCodeAt(0)) && name === binaryName) {
            return tar.subarray(offset + 512, offset + 512 + size);
        }
        offset += 512 + Math.ceil(size / 512) * 512;
    }

    throw new Error(`archive does not contain ${binaryName}`);
}

function endOfCentralDirectory(archive) {
    const minimumOffset = Math.max(0, archive.length - 65_557);
    for (let offset = archive.length - 22; offset >= minimumOffset; offset -= 1) {
        if (archive.readUInt32LE(offset) === 0x06054b50) {
            return offset;
        }
    }

    throw new Error("invalid zip archive");
}

function binaryFromZip(archive, binaryName) {
    const ending = endOfCentralDirectory(archive);
    const entries = archive.readUInt16LE(ending + 10);
    let centralOffset = archive.readUInt32LE(ending + 16);

    for (let index = 0; index < entries; index += 1) {
        if (centralOffset + 46 > archive.length || archive.readUInt32LE(centralOffset) !== 0x02014b50) {
            throw new Error("invalid zip central directory");
        }
        const flags = archive.readUInt16LE(centralOffset + 8);
        const compression = archive.readUInt16LE(centralOffset + 10);
        const compressedSize = archive.readUInt32LE(centralOffset + 20);
        const uncompressedSize = archive.readUInt32LE(centralOffset + 24);
        const nameLength = archive.readUInt16LE(centralOffset + 28);
        const extraLength = archive.readUInt16LE(centralOffset + 30);
        const commentLength = archive.readUInt16LE(centralOffset + 32);
        const localOffset = archive.readUInt32LE(centralOffset + 42);
        const entryEnd = centralOffset + 46 + nameLength + extraLength + commentLength;
        if (entryEnd > archive.length) {
            throw new Error("invalid zip entry");
        }
        const name = archive
            .subarray(centralOffset + 46, centralOffset + 46 + nameLength)
            .toString("utf8");

        if (name === binaryName) {
            if ((flags & 1) !== 0 || ![0, 8].includes(compression)) {
                throw new Error("unsupported zip entry");
            }
            if (localOffset + 30 > archive.length || archive.readUInt32LE(localOffset) !== 0x04034b50) {
                throw new Error("invalid zip local header");
            }
            const localNameLength = archive.readUInt16LE(localOffset + 26);
            const localExtraLength = archive.readUInt16LE(localOffset + 28);
            const dataOffset = localOffset + 30 + localNameLength + localExtraLength;
            if (dataOffset + compressedSize > archive.length) {
                throw new Error("invalid zip data");
            }
            const compressed = archive.subarray(dataOffset, dataOffset + compressedSize);
            const binary = compression === 0 ? Buffer.from(compressed) : zlib.inflateRawSync(compressed);
            if (binary.length !== uncompressedSize) {
                throw new Error("invalid zip size");
            }

            return binary;
        }
        centralOffset = entryEnd;
    }

    throw new Error(`archive does not contain ${binaryName}`);
}

function installBinary(binary) {
    const home = process.platform === "win32" ? process.env.USERPROFILE : process.env.HOME || os.homedir();
    if (!home) {
        throw new Error("could not determine the home directory");
    }

    const destinationDir = path.join(home, ".claude", "cc-statusline");
    const destination = path.join(destinationDir, BINARY_NAME);
    const temporary = path.join(destinationDir, `.${BINARY_NAME}.${process.pid}.tmp`);
    fs.mkdirSync(destinationDir, { recursive: true });
    fs.writeFileSync(temporary, binary, { mode: 0o755 });
    if (process.platform !== "win32") {
        fs.chmodSync(temporary, 0o755);
    }
    fs.rmSync(destination, { force: true });
    fs.renameSync(temporary, destination);

    return destination;
}

async function install() {
    if (process.env.npm_config_ignore_scripts === "true") {
        throw new Error("npm was invoked with --ignore-scripts; reinstall without it to run the installer");
    }
    const asset = assetFor(process.platform, process.arch);
    const baseUrl = releaseBaseUrl();
    const archive = await download(`${baseUrl}/${asset}`);
    const checksum = await download(`${baseUrl}/${asset}.sha256`);
    const verified = verifiedArchive(archive, checksum, asset);
    const binary = asset.endsWith(".zip")
        ? binaryFromZip(verified, BINARY_NAME)
        : binaryFromTarGz(verified, BINARY_NAME);
    const destination = installBinary(binary);
    console.log(`Installed cc-statusline to ${destination}`);
}

function manualInstallHint() {
    return [
        "The npm postinstall script was skipped or blocked by npm.",
        "Retry with the complete tarball URL and both npm policy flags:",
        `  ${NPM_INSTALL_COMMAND}`,
        "Or use the POSIX installer instead:",
        `  ${SHELL_INSTALL_COMMAND}`,
    ].join("\n");
}

async function main() {
    try {
        await install();
    } catch (error) {
        console.error(`cc-statusline installation failed: ${error.message}`);
        console.error(manualInstallHint());
        process.exitCode = 1;
    }
}

if (require.main === module) {
    main();
}

module.exports = { assetFor, binaryFromTarGz, binaryFromZip, install, verifiedArchive };
