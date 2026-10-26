#!/usr/bin/env node
"use strict";

const { createHash } = require("node:crypto");
const http = require("node:http");
const https = require("node:https");
const { spawnSync, execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const REPO = "asanabrial/estigia";

const TARGETS = {
  "linux-x64": { triple: "x86_64-unknown-linux-gnu", archive: "tar.gz", exe: "estigia" },
  "linux-arm64": { triple: "aarch64-unknown-linux-gnu", archive: "tar.gz", exe: "estigia" },
  "darwin-x64": { triple: "x86_64-apple-darwin", archive: "tar.gz", exe: "estigia" },
  "darwin-arm64": { triple: "aarch64-apple-darwin", archive: "tar.gz", exe: "estigia" },
  "win32-x64": { triple: "x86_64-pc-windows-msvc", archive: "zip", exe: "estigia.exe" },
  "win32-arm64": { triple: "x86_64-pc-windows-msvc", archive: "zip", exe: "estigia.exe" },
};

class EstigiaError extends Error {}

function say(message) {
  process.stderr.write(`estigia: ${message}\n`);
}

function fail(message) {
  throw new EstigiaError(message);
}

function resolveTarget() {
  const key = `${process.platform}-${process.arch}`;
  const target = TARGETS[key];
  if (!target) {
    fail(`no prebuilt Estigia for ${key}; build from source with 'cargo install estigia'`);
  }
  return target;
}

function installDirectory() {
  if (process.env.ESTIGIA_INSTALL_DIR) {
    return process.env.ESTIGIA_INSTALL_DIR;
  }
  if (process.platform === "win32") {
    const local = process.env.LOCALAPPDATA || path.join(os.homedir(), "AppData", "Local");
    return path.join(local, "estigia", "bin");
  }
  return path.join(os.homedir(), ".local", "bin");
}

function get(url, redirectsLeft = 5) {
  const client = url.startsWith("http://") ? http : https;
  return new Promise((resolve, reject) => {
    const request = client.get(url, { headers: { "user-agent": "estigia-npm" } }, (response) => {
      const status = response.statusCode || 0;
      if (status >= 300 && status < 400 && response.headers.location) {
        response.resume();
        if (redirectsLeft === 0) {
          reject(new Error(`too many redirects for ${url}`));
          return;
        }
        resolve(get(new URL(response.headers.location, url).toString(), redirectsLeft - 1));
        return;
      }
      if (status < 200 || status >= 300) {
        response.resume();
        reject(new Error(`${status} for ${url}`));
        return;
      }
      const chunks = [];
      response.on("data", (chunk) => chunks.push(chunk));
      response.on("end", () => resolve(Buffer.concat(chunks)));
      response.on("error", reject);
    });
    request.on("error", reject);
    request.setTimeout(60_000, () => request.destroy(new Error(`timed out after 60s for ${url}`)));
  });
}

function installedVersion(binary) {
  if (!fs.existsSync(binary)) {
    return null;
  }
  const answer = spawnSync(binary, ["--version"], { encoding: "utf8" });
  const match = /^estigia (\S+)/.exec(answer.stdout || "");
  return match ? match[1] : null;
}

function extract(archivePath, into) {
  const tar =
    process.platform === "win32"
      ? path.join(process.env.SystemRoot || "C:\\Windows", "System32", "tar.exe")
      : "tar";
  execFileSync(tar, ["-xf", archivePath, "-C", into], { stdio: ["ignore", "ignore", "inherit"] });
}

async function install(tag, target, binary) {
  const packageName = `estigia-${tag}-${target.triple}`;
  const archiveName = `${packageName}.${target.archive}`;
  const base = process.env.ESTIGIA_BASE_URL || `https://github.com/${REPO}/releases/download/${tag}`;

  say(`installing ${tag} for ${target.triple} into ${path.dirname(binary)}`);
  let archive;
  try {
    archive = await get(`${base}/${archiveName}`);
  } catch (error) {
    fail(`no release archive at ${base}/${archiveName}: ${error.message}`);
  }
  let sum;
  try {
    sum = (await get(`${base}/${archiveName}.sha256`)).toString("utf8");
  } catch (error) {
    fail(`no checksum published for ${archiveName}; refusing to install unverified`);
  }
  const expected = sum.trim().split(/\s+/)[0];
  if (!expected) {
    fail(`${archiveName}.sha256 carries no checksum`);
  }
  const actual = createHash("sha256").update(archive).digest("hex");
  if (actual !== expected) {
    fail(`checksum mismatch for ${archiveName}: expected ${expected}, got ${actual}`);
  }

  const staging = fs.mkdtempSync(path.join(os.tmpdir(), "estigia-npm-"));
  try {
    const archivePath = path.join(staging, archiveName);
    fs.writeFileSync(archivePath, archive);
    extract(archivePath, staging);
    const candidate = path.join(staging, packageName, target.exe);
    if (!fs.existsSync(candidate)) {
      fail(`${archiveName} did not contain ${packageName}/${target.exe}`);
    }
    fs.chmodSync(candidate, 0o755);

    const recorded = spawnSync(candidate, ["__record-install"], {
      stdio: ["ignore", process.stderr, process.stderr],
    });
    if (recorded.error) {
      fail(`could not run the downloaded candidate: ${recorded.error.message}`);
    }
    if (recorded.status !== 0) {
      fail("candidate lifecycle admission failed; refusing to replace the installed executable");
    }

    fs.mkdirSync(path.dirname(binary), { recursive: true });
    const placed = `${binary}.${process.pid}.new`;
    fs.copyFileSync(candidate, placed);
    fs.chmodSync(placed, 0o755);
    fs.renameSync(placed, binary);
  } finally {
    fs.rmSync(staging, { recursive: true, force: true });
  }
}

async function main() {
  const target = resolveTarget();
  const { version } = require("../package.json");
  const tag = process.env.ESTIGIA_VERSION || `v${version}`;
  const binary = path.join(installDirectory(), target.exe);

  if (installedVersion(binary) !== tag.replace(/^v/, "")) {
    await install(tag, target, binary);
  }

  const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });
  if (result.error) {
    fail(`could not run ${binary}: ${result.error.message}`);
  }
  process.exitCode = result.signal || result.status === null ? 1 : result.status;
}

main().catch((error) => {
  say(error instanceof EstigiaError ? error.message : error.stack || error.message);
  process.exitCode = 1;
});
