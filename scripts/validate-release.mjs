#!/usr/bin/env node

import { createHash } from "node:crypto";
import { inflateRawSync } from "node:zlib";
import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  repoRootFromScript,
  validateVersionConsistency,
  writeFileAtomically,
} from "./version-files.mjs";
import { parseReleaseVersion } from "./release-version.mjs";

export const PLATFORMS = Object.freeze([
  Object.freeze({ id: "windows-x64", rawExtension: ".exe" }),
  Object.freeze({ id: "linux-x64", rawExtension: "" }),
  Object.freeze({ id: "macos-x64", rawExtension: "" }),
  Object.freeze({ id: "macos-arm64", rawExtension: "" }),
]);

const RUNNING_FILE = "RUNNING.txt";
const CHECKSUM_FILE = "sha.txt";

function usage() {
  return [
    "Usage:",
    "  node scripts/validate-release.mjs [--root <path>] [--version <YYYY.M.N-sha7>] [--require-exact]",
    "  node scripts/validate-release.mjs --assets <dir> --platform <platform>",
    "  node scripts/validate-release.mjs --assets <dir> [--write-sha]",
    "",
    `Platforms: ${PLATFORMS.map((platform) => platform.id).join(", ")}`,
    "With --assets and no --platform, all four platform artifact pairs and sha.txt are validated.",
    "--write-sha regenerates sha.txt from the eight raw/ZIP assets before validating it.",
  ].join("\n");
}

function requireValue(argv, index, option) {
  const value = argv[index + 1];
  if (!value || value.startsWith("--")) {
    throw new Error(`${option} requires a value. ${usage()}`);
  }
  return value;
}

export function parseCliArguments(argv = process.argv.slice(2)) {
  let root = repoRootFromScript();
  let assetsDirectory;
  let platform;
  let expectedVersion;
  let writeSha = false;
  let requireExact = false;

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--root") {
      root = resolve(requireValue(argv, index, "--root"));
      index += 1;
    } else if (argument.startsWith("--root=")) {
      root = resolve(argument.slice("--root=".length));
    } else if (argument === "--assets") {
      assetsDirectory = resolve(requireValue(argv, index, "--assets"));
      index += 1;
    } else if (argument.startsWith("--assets=")) {
      assetsDirectory = resolve(argument.slice("--assets=".length));
    } else if (argument === "--platform") {
      platform = requireValue(argv, index, "--platform");
      index += 1;
    } else if (argument.startsWith("--platform=")) {
      platform = argument.slice("--platform=".length);
    } else if (argument === "--version") {
      expectedVersion = parseReleaseVersion(requireValue(argv, index, "--version"), "--version").version;
      index += 1;
    } else if (argument.startsWith("--version=")) {
      expectedVersion = parseReleaseVersion(argument.slice("--version=".length), "--version").version;
    } else if (argument === "--write-sha") {
      writeSha = true;
    } else if (argument === "--require-exact") {
      requireExact = true;
    } else if (argument === "--help" || argument === "-h") {
      console.log(usage());
      process.exit(0);
    } else {
      throw new Error(`Unknown argument ${argument}. ${usage()}`);
    }
  }

  if (platform && !PLATFORMS.some((candidate) => candidate.id === platform)) {
    throw new Error(`Unknown platform ${platform}. ${usage()}`);
  }
  if (!assetsDirectory && (platform || writeSha)) {
    throw new Error(`--assets is required when using --platform or --write-sha. ${usage()}`);
  }
  if (platform && writeSha) {
    throw new Error(`--write-sha is only valid when validating all platform assets. ${usage()}`);
  }

  return { root, assetsDirectory, platform, expectedVersion, writeSha, requireExact };
}

export function assetBaseName(platformId, version) {
  const platform = PLATFORMS.find((candidate) => candidate.id === platformId);
  if (!platform) {
    throw new Error(`Unknown platform ${platformId}`);
  }
  return `Flint-${platform.id}-${parseReleaseVersion(version, "artifact version").version}`;
}

export function rawAssetName(platformId, version) {
  const platform = PLATFORMS.find((candidate) => candidate.id === platformId);
  if (!platform) {
    throw new Error(`Unknown platform ${platformId}`);
  }
  return `${assetBaseName(platformId, version)}${platform.rawExtension}`;
}

export function expectedPlatformAssets(platformId, version) {
  return [rawAssetName(platformId, version), `${assetBaseName(platformId, version)}.zip`];
}

export function expectedAllAssets(version) {
  return PLATFORMS.flatMap((platform) => expectedPlatformAssets(platform.id, version));
}

function assertRegularNonEmptyFile(path, label) {
  let info;
  try {
    info = statSync(path);
  } catch (error) {
    throw new Error(`${label} is missing at ${path}: ${error.message}`);
  }
  if (!info.isFile()) {
    throw new Error(`${label} is not a regular file: ${path}`);
  }
  if (info.size <= 0) {
    throw new Error(`${label} is empty: ${path}`);
  }
  return info;
}

function sorted(values) {
  return [...values].sort((left, right) => (left < right ? -1 : left > right ? 1 : 0));
}

function ensureExactDirectoryEntries(directory, allowedNames, requiredNames) {
  let entries;
  try {
    entries = readdirSync(directory, { withFileTypes: true });
  } catch (error) {
    throw new Error(`Cannot read asset directory ${directory}: ${error.message}`);
  }

  const unexpectedDirectories = entries.filter((entry) => !entry.isFile()).map((entry) => entry.name);
  if (unexpectedDirectories.length > 0) {
    throw new Error(`Asset directory contains unexpected non-file entries: ${unexpectedDirectories.join(", ")}`);
  }

  const actualNames = sorted(entries.map((entry) => entry.name));
  const allowed = new Set(allowedNames);
  const required = sorted(requiredNames);
  const missing = required.filter((name) => !actualNames.includes(name));
  const unexpected = actualNames.filter((name) => !allowed.has(name));
  if (missing.length > 0) {
    throw new Error(`Missing release assets: ${missing.join(", ")}`);
  }
  if (unexpected.length > 0) {
    throw new Error(`Unexpected files in asset directory: ${unexpected.join(", ")}`);
  }
}

function findEndOfCentralDirectory(buffer) {
  if (buffer.length < 22) {
    throw new Error("ZIP file is too short to contain an end-of-central-directory record");
  }
  const minimumOffset = Math.max(0, buffer.length - 22 - 0xffff);
  for (let offset = buffer.length - 22; offset >= minimumOffset; offset -= 1) {
    if (buffer.readUInt32LE(offset) === 0x06054b50) {
      return offset;
    }
  }
  throw new Error("ZIP end-of-central-directory record was not found");
}

export function listZipEntries(zipPath) {
  const buffer = readFileSync(zipPath);
  const eocd = findEndOfCentralDirectory(buffer);
  const diskNumber = buffer.readUInt16LE(eocd + 4);
  const centralDirectoryDisk = buffer.readUInt16LE(eocd + 6);
  const entriesOnDisk = buffer.readUInt16LE(eocd + 8);
  const entriesTotal = buffer.readUInt16LE(eocd + 10);
  const centralDirectorySize = buffer.readUInt32LE(eocd + 12);
  const centralDirectoryOffset = buffer.readUInt32LE(eocd + 16);
  const commentLength = buffer.readUInt16LE(eocd + 20);

  if (diskNumber !== 0 || centralDirectoryDisk !== 0 || entriesOnDisk !== entriesTotal) {
    throw new Error("Multi-disk ZIP archives are not supported");
  }
  if (entriesTotal === 0xffff || centralDirectorySize === 0xffffffff || centralDirectoryOffset === 0xffffffff) {
    throw new Error("ZIP64 assets are not supported by this validator");
  }
  if (eocd + 22 + commentLength > buffer.length) {
    throw new Error("ZIP archive comment extends beyond the end of the file");
  }

  const centralDirectoryEnd = centralDirectoryOffset + centralDirectorySize;
  if (centralDirectoryOffset < 0 || centralDirectoryEnd > buffer.length) {
    throw new Error("ZIP central directory lies outside the archive");
  }

  const entries = [];
  let offset = centralDirectoryOffset;
  for (let index = 0; index < entriesTotal; index += 1) {
    if (offset + 46 > buffer.length || buffer.readUInt32LE(offset) !== 0x02014b50) {
      throw new Error(`Invalid ZIP central-directory entry at offset ${offset}`);
    }
    const flags = buffer.readUInt16LE(offset + 8);
    const method = buffer.readUInt16LE(offset + 10);
    const crc32 = buffer.readUInt32LE(offset + 16);
    const compressedSize = buffer.readUInt32LE(offset + 20);
    const uncompressedSize = buffer.readUInt32LE(offset + 24);
    const nameLength = buffer.readUInt16LE(offset + 28);
    const extraLength = buffer.readUInt16LE(offset + 30);
    const entryCommentLength = buffer.readUInt16LE(offset + 32);
    const localHeaderOffset = buffer.readUInt32LE(offset + 42);
    const nameStart = offset + 46;
    const nameEnd = nameStart + nameLength;
    const nextOffset = nameEnd + extraLength + entryCommentLength;
    if (nameEnd > buffer.length || nextOffset > buffer.length) {
      throw new Error(`Truncated ZIP central-directory entry at offset ${offset}`);
    }
    if (compressedSize === 0xffffffff || uncompressedSize === 0xffffffff || localHeaderOffset === 0xffffffff) {
      throw new Error("ZIP64 entries are not supported by this validator");
    }
    const rawName = buffer.subarray(nameStart, nameEnd);
    const name = (flags & 0x0800 ? rawName.toString("utf8") : rawName.toString("utf8"))
      .replace(/\\/g, "/");

    entries.push({
      name,
      flags,
      method,
      crc32,
      compressedSize,
      uncompressedSize,
      localHeaderOffset,
    });
    offset = nextOffset;
  }

  if (offset > centralDirectoryEnd) {
    throw new Error("ZIP central directory contains more data than declared");
  }
  return entries;
}

export function extractZipEntry(zipPath, entry) {
  const buffer = readFileSync(zipPath);
  const localOffset = entry.localHeaderOffset;
  if (localOffset + 30 > buffer.length || buffer.readUInt32LE(localOffset) !== 0x04034b50) {
    throw new Error(`Invalid ZIP local header for ${entry.name}`);
  }
  const nameLength = buffer.readUInt16LE(localOffset + 26);
  const extraLength = buffer.readUInt16LE(localOffset + 28);
  const dataStart = localOffset + 30 + nameLength + extraLength;
  const dataEnd = dataStart + entry.compressedSize;
  if (dataEnd > buffer.length) {
    throw new Error(`Truncated ZIP data for ${entry.name}`);
  }

  const compressed = buffer.subarray(dataStart, dataEnd);
  let data;
  if (entry.method === 0) {
    data = Buffer.from(compressed);
  } else if (entry.method === 8) {
    data = inflateRawSync(compressed);
  } else {
    throw new Error(`Unsupported ZIP compression method ${entry.method} for ${entry.name}`);
  }

  if (data.length !== entry.uncompressedSize) {
    throw new Error(`ZIP entry size mismatch for ${entry.name}: expected ${entry.uncompressedSize}, got ${data.length}`);
  }
  if (crc32(data) !== entry.crc32) {
    throw new Error(`ZIP entry CRC mismatch for ${entry.name}`);
  }
  return data;
}

const CRC32_TABLE = new Uint32Array(256);
for (let index = 0; index < CRC32_TABLE.length; index += 1) {
  let value = index;
  for (let bit = 0; bit < 8; bit += 1) {
    value = (value >>> 1) ^ (0xedb88320 & -(value & 1));
  }
  CRC32_TABLE[index] = value >>> 0;
}

export function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc = (crc >>> 8) ^ CRC32_TABLE[(crc ^ byte) & 0xff];
  }
  return (crc ^ 0xffffffff) >>> 0;
}

export function validatePlatformAssets(assetsDirectory, platformId, version) {
  const rawName = rawAssetName(platformId, version);
  const zipName = `${assetBaseName(platformId, version)}.zip`;
  const rawPath = join(assetsDirectory, rawName);
  const zipPath = join(assetsDirectory, zipName);
  const rawInfo = assertRegularNonEmptyFile(rawPath, `${platformId} raw executable`);
  assertRegularNonEmptyFile(zipPath, `${platformId} ZIP archive`);

  const entries = listZipEntries(zipPath);
  const byName = new Map();
  for (const entry of entries) {
    if (entry.name.length === 0 || entry.name.startsWith("/") || entry.name.includes("\0")) {
      throw new Error(`Unsafe ZIP entry name ${JSON.stringify(entry.name)} in ${zipName}`);
    }
    if (entry.name.split("/").includes("..")) {
      throw new Error(`Unsafe ZIP entry path ${JSON.stringify(entry.name)} in ${zipName}`);
    }
    if (byName.has(entry.name)) {
      throw new Error(`Duplicate ZIP entry ${entry.name} in ${zipName}`);
    }
    byName.set(entry.name, entry);
  }

  const runningEntry = byName.get(RUNNING_FILE);
  const rawEntry = byName.get(rawName);
  if (runningEntry === undefined || rawEntry === undefined || entries.length !== 2) {
    const found = entries.map((entry) => entry.name).join(", ") || "<none>";
    throw new Error(`${zipName} must contain exactly ${rawName} and ${RUNNING_FILE}; found: ${found}`);
  }
  if (rawEntry.uncompressedSize <= 0 || rawEntry.uncompressedSize !== rawInfo.size) {
    throw new Error(`${zipName} raw executable entry has an unexpected size`);
  }
  if (!extractZipEntry(zipPath, rawEntry).equals(readFileSync(rawPath))) {
    throw new Error(`${zipName} does not contain the exact bytes of ${rawName}`);
  }

  const runningText = extractZipEntry(zipPath, runningEntry).toString("utf8");
  if (runningText.trim().length === 0) {
    throw new Error(`${zipName} contains an empty ${RUNNING_FILE}`);
  }
  if (!runningText.includes(parseReleaseVersion(version).version) || !runningText.includes(platformId)) {
    throw new Error(`${zipName} ${RUNNING_FILE} does not mention the version and platform`);
  }

  return { platform: platformId, rawName, zipName, rawBytes: rawInfo.size };
}

export function sha256File(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

export function buildChecksumText(assetsDirectory, names) {
  const expected = new Set(names);
  if (expected.size !== names.length) {
    throw new Error("Checksum input contains duplicate filenames");
  }
  const uniqueNames = sorted(names);
  return uniqueNames
    .map((name) => {
      if (name === CHECKSUM_FILE || name.includes("/") || name.includes("\\")) {
        throw new Error(`Refusing to checksum invalid asset name ${name}`);
      }
      return `${sha256File(join(assetsDirectory, name))}  ${name}\n`;
    })
    .join("");
}

export function validateChecksumFile(assetsDirectory, version) {
  const expectedNames = expectedAllAssets(version);
  const checksumPath = join(assetsDirectory, CHECKSUM_FILE);
  const checksumText = readFileSync(checksumPath, "utf8");
  const lines = checksumText.split(/\r?\n/);
  while (lines.length > 0 && lines[lines.length - 1] === "") {
    lines.pop();
  }
  if (lines.length !== expectedNames.length) {
    throw new Error(`${CHECKSUM_FILE} must contain exactly ${expectedNames.length} entries; found ${lines.length}`);
  }

  const expected = new Set(expectedNames);
  const seen = new Set();
  for (const line of lines) {
    const match = line.match(/^([0-9a-f]{64})  (.+)$/i);
    if (!match) {
      throw new Error(`Invalid ${CHECKSUM_FILE} line: ${JSON.stringify(line)}`);
    }
    const digest = match[1].toLowerCase();
    const name = match[2];
    if (!expected.has(name)) {
      throw new Error(`${CHECKSUM_FILE} references an unexpected asset: ${name}`);
    }
    if (seen.has(name)) {
      throw new Error(`${CHECKSUM_FILE} contains a duplicate entry for ${name}`);
    }
    seen.add(name);
    const actual = sha256File(join(assetsDirectory, name));
    if (actual !== digest) {
      throw new Error(`${CHECKSUM_FILE} checksum mismatch for ${name}`);
    }
  }

  const missing = expectedNames.filter((name) => !seen.has(name));
  if (missing.length > 0) {
    throw new Error(`${CHECKSUM_FILE} is missing entries for: ${missing.join(", ")}`);
  }
  return { entries: expectedNames.length };
}

export function validateRelease(options) {
  const state = validateVersionConsistency(options.root, options.expectedVersion, {
    requireExact: options.requireExact,
  });
  if (state.exactMetadata) {
    console.log(`Version metadata consistent: ${state.version}`);
  } else {
    console.log(
      `VERSION records ${state.version}; generated metadata baseline is ${state.metadataVersion}. ` +
      "Release builds apply the git-computed version to VERSION, package, Tauri, and Cargo metadata before compiling.",
    );
  }

  if (!options.assetsDirectory) {
    console.log("Version-only release validation passed.");
    return state;
  }

  const assetsDirectory = options.assetsDirectory;
  if (!existsSync(assetsDirectory) || !statSync(assetsDirectory).isDirectory()) {
    throw new Error(`Asset directory does not exist: ${assetsDirectory}`);
  }

  if (options.platform) {
    const expected = expectedPlatformAssets(options.platform, state.version);
    ensureExactDirectoryEntries(assetsDirectory, expected, expected);
    const result = validatePlatformAssets(assetsDirectory, options.platform, state.version);
    console.log(`Validated ${result.platform}: ${result.rawName} and ${result.zipName}`);
    console.log("Release asset validation passed.");
    return state;
  }

  const expected = expectedAllAssets(state.version);
  const checksumPath = join(assetsDirectory, CHECKSUM_FILE);
  const checksumAllowed = options.writeSha || existsSync(checksumPath);
  const allowed = checksumAllowed ? [...expected, CHECKSUM_FILE] : expected;
  ensureExactDirectoryEntries(assetsDirectory, allowed, expected);

  for (const platform of PLATFORMS) {
    validatePlatformAssets(assetsDirectory, platform.id, state.version);
  }
  console.log(`Validated all ${expected.length} raw/ZIP release assets.`);

  if (options.writeSha) {
    const checksumText = buildChecksumText(assetsDirectory, expected);
    writeFileAtomically(checksumPath, checksumText, 0o644);
    console.log(`Wrote ${CHECKSUM_FILE} with ${expected.length} SHA-256 entries.`);
  } else if (!existsSync(checksumPath)) {
    throw new Error(`${CHECKSUM_FILE} is required when validating all platform assets. Use --write-sha to generate it.`);
  }

  const checksumResult = validateChecksumFile(assetsDirectory, state.version);
  console.log(`Validated ${CHECKSUM_FILE}: ${checksumResult.entries} exact asset checksums.`);
  console.log("Release asset validation passed.");
  return state;
}

export function main(argv = process.argv.slice(2)) {
  const options = parseCliArguments(argv);
  validateRelease(options);
}

function isMainModule() {
  if (!process.argv[1]) {
    return false;
  }
  return resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

if (isMainModule()) {
  try {
    main();
  } catch (error) {
    console.error(`validate-release: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  }
}