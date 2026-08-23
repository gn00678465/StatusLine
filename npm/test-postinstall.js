"use strict";

const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const http = require("node:http");
const os = require("node:os");
const path = require("node:path");
const { spawn } = require("node:child_process");
const zlib = require("node:zlib");

const installer = path.join(__dirname, "postinstall.js");
const binaryContents = Buffer.from("statusline test binary\n", "utf8");

function assetForCurrentPlatform() {
    const targets = {
        "darwin-arm64": "cc-statusline-darwin-arm64.tar.gz",
        "darwin-x64": "cc-statusline-darwin-x64.tar.gz",
        "linux-arm64": "cc-statusline-linux-arm64-musl.tar.gz",
        "linux-x64": "cc-statusline-linux-x64-musl.tar.gz",
        "win32-arm64": "cc-statusline-win32-arm64.zip",
        "win32-x64": "cc-statusline-win32-x64.zip",
    };
    const asset = targets[`${process.platform}-${process.arch}`];
    if (!asset) {
        throw new Error(`unsupported test platform: ${process.platform}-${process.arch}`);
    }

    return asset;
}

function writeOctal(buffer, offset, length, value) {
    buffer.write(`${value.toString(8).padStart(length - 1, "0")}\0`, offset, length, "ascii");
}

function tarGzWithBinary(binaryName, contents) {
    const header = Buffer.alloc(512, 0);
    header.write(binaryName, 0, "utf8");
    writeOctal(header, 100, 8, 0o755);
    writeOctal(header, 108, 8, 0);
    writeOctal(header, 116, 8, 0);
    writeOctal(header, 124, 12, contents.length);
    writeOctal(header, 136, 12, 0);
    header.fill(0x20, 148, 156);
    header[156] = "0".charCodeAt(0);
    header.write("ustar\0", 257, "ascii");
    header.write("00", 263, "ascii");
    writeOctal(
        header,
        148,
        8,
        header.reduce((total, byte) => total + byte, 0),
    );

    const padding = Buffer.alloc((512 - (contents.length % 512)) % 512, 0);
    return zlib.gzipSync(Buffer.concat([header, contents, padding, Buffer.alloc(1024, 0)]));
}

function zipWithBinary(binaryName, contents) {
    const name = Buffer.from(binaryName, "utf8");
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(0, 8);
    local.writeUInt32LE(contents.length, 18);
    local.writeUInt32LE(contents.length, 22);
    local.writeUInt16LE(name.length, 26);

    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(0, 10);
    central.writeUInt32LE(contents.length, 20);
    central.writeUInt32LE(contents.length, 24);
    central.writeUInt16LE(name.length, 28);

    const centralOffset = local.length + name.length + contents.length;
    const ending = Buffer.alloc(22);
    ending.writeUInt32LE(0x06054b50, 0);
    ending.writeUInt16LE(1, 8);
    ending.writeUInt16LE(1, 10);
    ending.writeUInt32LE(central.length + name.length, 12);
    ending.writeUInt32LE(centralOffset, 16);

    return Buffer.concat([local, name, contents, central, name, ending]);
}

function archiveWithBinary(asset, binaryName, contents) {
    return asset.endsWith(".zip")
        ? zipWithBinary(binaryName, contents)
        : tarGzWithBinary(binaryName, contents);
}

function startServer(routes) {
    return new Promise((resolve) => {
        const server = http.createServer((request, response) => {
            const body = routes[request.url];
            if (!body) {
                response.writeHead(404);
                response.end("not found");
                return;
            }
            response.writeHead(200, { "content-length": body.length });
            response.end(body);
        });
        server.listen(0, "127.0.0.1", () => {
            const address = server.address();
            resolve({
                baseUrl: `http://127.0.0.1:${address.port}`,
                close: () => new Promise((done) => server.close(done)),
            });
        });
    });
}

function runInstaller(environment) {
    return runNode([installer], environment);
}

function runNode(arguments_, environment) {
    return new Promise((resolve, reject) => {
        const child = spawn(process.execPath, arguments_, {
            env: environment,
            stdio: ["ignore", "pipe", "pipe"],
        });
        let stderr = "";
        child.stderr.on("data", (chunk) => {
            stderr += chunk;
        });
        child.on("error", reject);
        child.on("close", (status) => resolve({ status, stderr }));
    });
}

async function installs_a_verified_binary() {
    const asset = assetForCurrentPlatform();
    const binaryName = process.platform === "win32" ? "cc-statusline.exe" : "cc-statusline";
    const archive = archiveWithBinary(asset, binaryName, binaryContents);
    const checksum = crypto.createHash("sha256").update(archive).digest("hex");
    const server = await startServer({
        [`/${asset}`]: archive,
        [`/${asset}.sha256`]: Buffer.from(`${checksum}  ${asset}\n`, "utf8"),
    });
    const home = fs.mkdtempSync(path.join(os.tmpdir(), "cc-statusline-node-test-"));

    try {
        const result = await runInstaller({
            ...process.env,
            CC_STATUSLINE_BASE_URL: server.baseUrl,
            HOME: home,
            USERPROFILE: home,
            npm_config_ignore_scripts: "false",
        });

        assert.equal(result.status, 0, result.stderr);
        const installed = path.join(home, ".claude", "cc-statusline", binaryName);
        assert.deepEqual(fs.readFileSync(installed), binaryContents);
        if (process.platform !== "win32") {
            assert.notEqual(fs.statSync(installed).mode & 0o111, 0);
        }
    } finally {
        await server.close();
        fs.rmSync(home, { recursive: true, force: true });
    }
}

async function rejects_a_tampered_archive_before_installing() {
    const asset = assetForCurrentPlatform();
    const binaryName = process.platform === "win32" ? "cc-statusline.exe" : "cc-statusline";
    const archive = archiveWithBinary(asset, binaryName, binaryContents);
    const tamperedArchive = Buffer.concat([archive, Buffer.from("tampered", "utf8")]);
    const checksum = crypto.createHash("sha256").update(archive).digest("hex");
    const server = await startServer({
        [`/${asset}`]: tamperedArchive,
        [`/${asset}.sha256`]: Buffer.from(`${checksum}  ${asset}\n`, "utf8"),
    });
    const home = fs.mkdtempSync(path.join(os.tmpdir(), "cc-statusline-node-test-"));

    try {
        const result = await runInstaller({
            ...process.env,
            CC_STATUSLINE_BASE_URL: server.baseUrl,
            HOME: home,
            USERPROFILE: home,
            npm_config_ignore_scripts: "false",
        });

        assert.equal(result.status, 1);
        assert.match(result.stderr, /SHA-256 verification failed/);
        assert.match(result.stderr, /Manual install:/);
        assert.equal(fs.existsSync(path.join(home, ".claude", "cc-statusline", binaryName)), false);
    } finally {
        await server.close();
        fs.rmSync(home, { recursive: true, force: true });
    }
}

async function installs_a_verified_windows_zip() {
    const asset = "cc-statusline-win32-x64.zip";
    const archive = zipWithBinary("cc-statusline.exe", binaryContents);
    const checksum = crypto.createHash("sha256").update(archive).digest("hex");
    const server = await startServer({
        [`/${asset}`]: archive,
        [`/${asset}.sha256`]: Buffer.from(`${checksum}  ${asset}\n`, "utf8"),
    });
    const home = fs.mkdtempSync(path.join(os.tmpdir(), "cc-statusline-node-test-"));
    const command = [
        "-e",
        "Object.defineProperty(process, 'platform', { value: 'win32' }); Object.defineProperty(process, 'arch', { value: 'x64' }); require(process.argv[1]).install().catch((error) => { console.error(error.stack || error); process.exitCode = 1; });",
        installer,
    ];

    try {
        const result = await runNode(command, {
            ...process.env,
            CC_STATUSLINE_BASE_URL: server.baseUrl,
            HOME: home,
            USERPROFILE: home,
            npm_config_ignore_scripts: "false",
        });

        assert.equal(result.status, 0, result.stderr);
        const installed = path.join(home, ".claude", "cc-statusline", "cc-statusline.exe");
        assert.deepEqual(fs.readFileSync(installed), binaryContents);
    } finally {
        await server.close();
        fs.rmSync(home, { recursive: true, force: true });
    }
}

async function explains_how_to_recover_from_ignore_scripts() {
    const home = fs.mkdtempSync(path.join(os.tmpdir(), "cc-statusline-node-test-"));

    try {
        const result = await runInstaller({
            ...process.env,
            CC_STATUSLINE_BASE_URL: "http://127.0.0.1:1",
            HOME: home,
            USERPROFILE: home,
            npm_config_ignore_scripts: "true",
        });

        assert.equal(result.status, 1);
        assert.match(result.stderr, /--ignore-scripts/);
        assert.match(result.stderr, /Manual install:/);
    } finally {
        fs.rmSync(home, { recursive: true, force: true });
    }
}

Promise.all([
    installs_a_verified_binary(),
    rejects_a_tampered_archive_before_installing(),
    installs_a_verified_windows_zip(),
    explains_how_to_recover_from_ignore_scripts(),
])
    .then(() => console.log("postinstall installation and checksum tests passed"))
    .catch((error) => {
        console.error(error.stack || error);
        process.exitCode = 1;
    });
