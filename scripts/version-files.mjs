import {
  closeSync,
  fsyncSync,
  openSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { randomBytes } from "node:crypto";
import { basename, dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { compareReleaseVersions, LEGACY_VERSION_PATTERN, parseReleaseVersion } from "./release-version.mjs";

export const VERSION_FILE_PATHS = Object.freeze({
  version: "VERSION",
  package: "package.json",
  tauri: "src-tauri/tauri.conf.json",
  cargo: "src-tauri/Cargo.toml",
  cargoLock: "src-tauri/Cargo.lock",
});

export function repoRootFromScript(importMetaUrl = import.meta.url) {
  return resolve(dirname(fileURLToPath(importMetaUrl)), "..");
}


function formatError(error) {
  if (error && typeof error.message === "string") {
    return error.message;
  }
  return String(error);
}

function readUtf8(path, label) {
  try {
    return readFileSync(path, "utf8");
  } catch (error) {
    throw new Error(`Cannot read ${label} at ${path}: ${formatError(error)}`);
  }
}

function parseJson(text, label) {
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`Invalid JSON in ${label}: ${formatError(error)}`);
  }
}

function detectEol(text) {
  return text.includes("\r\n") ? "\r\n" : "\n";
}

function renderJson(originalText, value) {
  const eol = detectEol(originalText);
  const hadTrailingNewline = /\r?\n$/.test(originalText);
  return `${JSON.stringify(value, null, 2).replace(/\n/g, eol)}${hadTrailingNewline ? eol : ""}`;
}

export function extractCargoPackageVersion(cargoText) {
  const lines = cargoText.split(/\r?\n/);
  let section = null;
  let packageSectionCount = 0;
  const versionLines = [];

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const header = line.match(/^\s*\[([^\]]+)\]\s*(?:#.*)?$/);
    if (header) {
      section = header[1].trim();
      if (section === "package") {
        packageSectionCount += 1;
      }
      continue;
    }

    if (section !== "package") {
      continue;
    }

    const match = line.match(/^(\s*version\s*=\s*)(["'])([^"']*)\2(\s*(?:#.*)?)$/);
    if (match) {
      versionLines.push({ index, prefix: match[1], quote: match[2], suffix: match[4], value: match[3] });
    }
  }

  if (packageSectionCount !== 1) {
    throw new Error(`src-tauri/Cargo.toml must contain exactly one [package] section; found ${packageSectionCount}`);
  }
  if (versionLines.length !== 1) {
    throw new Error(`src-tauri/Cargo.toml [package] must contain exactly one version field; found ${versionLines.length}`);
  }

  return versionLines[0].value;
}

export function replaceCargoPackageVersion(cargoText, version) {
  const lines = cargoText.split(/\r?\n/);
  let section = null;
  let packageSectionCount = 0;
  let replacements = 0;

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const header = line.match(/^\s*\[([^\]]+)\]\s*(?:#.*)?$/);
    if (header) {
      section = header[1].trim();
      if (section === "package") {
        packageSectionCount += 1;
      }
      continue;
    }

    if (section !== "package") {
      continue;
    }

    const match = line.match(/^(\s*version\s*=\s*)(["'])([^"']*)\2(\s*(?:#.*)?)$/);
    if (match) {
      lines[index] = `${match[1]}${match[2]}${version}${match[2]}${match[4]}`;
      replacements += 1;
    }
  }

  if (packageSectionCount !== 1) {
    throw new Error(`src-tauri/Cargo.toml must contain exactly one [package] section; found ${packageSectionCount}`);
  }
  if (replacements !== 1) {
    throw new Error(`src-tauri/Cargo.toml [package] must contain exactly one version field; found ${replacements}`);
  }

  return lines.join(cargoText.includes("\r\n") ? "\r\n" : "\n");
}

function stripTomlComment(line) {
  let quote = null;
  for (let index = 0; index < line.length; index += 1) {
    const character = line[index];
    if (quote !== null) {
      if (character === "\\" && quote === '"') {
        index += 1;
      } else if (character === quote) {
        quote = null;
      }
    } else if (character === '"' || character === "'") {
      quote = character;
    } else if (character === "#") {
      return line.slice(0, index);
    }
  }
  return line;
}

function cargoSectionHeader(line) {
  const match = /^\s*\[([^\]]+)\]\s*$/.exec(stripTomlComment(line).trim());
  return match ? match[1].trim() : null;
}

export function extractCargoWorkspaceMembers(cargoText) {
  const lines = cargoText.split(/\r?\n/);
  let section = null;
  let workspaceSections = 0;
  let collecting = false;
  let found = false;
  let complete = false;
  let memberText = "";

  for (const line of lines) {
    const cleaned = stripTomlComment(line);
    const header = cargoSectionHeader(cleaned);
    if (header !== null) {
      if (collecting) {
        throw new Error("src-tauri/Cargo.toml [workspace] members array is unterminated");
      }
      section = header;
      if (section === "workspace") {
        workspaceSections += 1;
      }
      continue;
    }
    if (section !== "workspace") {
      continue;
    }

    if (!collecting) {
      const match = /^\s*members\s*=\s*\[(.*)$/.exec(cleaned);
      if (match === null) {
        continue;
      }
      if (found) {
        throw new Error("src-tauri/Cargo.toml [workspace] must contain exactly one members field");
      }
      found = true;
      collecting = true;
      const remainder = match[1];
      const close = remainder.indexOf("]");
      if (close === -1) {
        memberText += `${remainder}\n`;
      } else {
        memberText += remainder.slice(0, close);
        if (remainder.slice(close + 1).trim() !== "") {
          throw new Error("src-tauri/Cargo.toml [workspace] members array has unexpected trailing content");
        }
        collecting = false;
        complete = true;
      }
      continue;
    }

    const close = cleaned.indexOf("]");
    if (close === -1) {
      memberText += `${cleaned}\n`;
    } else {
      memberText += cleaned.slice(0, close);
      if (cleaned.slice(close + 1).trim() !== "") {
        throw new Error("src-tauri/Cargo.toml [workspace] members array has unexpected trailing content");
      }
      collecting = false;
      complete = true;
    }
  }

  if (workspaceSections === 0) {
    return null;
  }
  if (workspaceSections !== 1) {
    throw new Error(`src-tauri/Cargo.toml must contain exactly one [workspace] section; found ${workspaceSections}`);
  }
  if (!found || !complete || collecting) {
    throw new Error("src-tauri/Cargo.toml [workspace] must declare an explicit members array");
  }

  const members = [];
  const scrubbed = memberText
    .replace(/(['"])(.*?)\1/g, (_match, _quote, value) => {
      members.push(value);
      return " ";
    })
    .replace(/,/g, " ");
  if (scrubbed.trim() !== "") {
    throw new Error("src-tauri/Cargo.toml [workspace] members must be an explicit quoted array");
  }
  if (members.length === 0) {
    throw new Error("src-tauri/Cargo.toml [workspace] members must not be empty");
  }

  const seen = new Set();
  for (const member of members) {
    if (member.length === 0) {
      throw new Error("src-tauri/Cargo.toml [workspace] members must not contain an empty path");
    }
    if (/[\u0000-\u001f?*\[\]{}]/.test(member)) {
      throw new Error(`src-tauri/Cargo.toml [workspace] member '${member}' must be a literal path without wildcards`);
    }
    if (seen.has(member)) {
      throw new Error(`src-tauri/Cargo.toml [workspace] member '${member}' is listed more than once`);
    }
    seen.add(member);
  }
  return members;
}

export function extractCargoWorkspacePackageVersion(cargoText) {
  const lines = cargoText.split(/\r?\n/);
  let section = null;
  let sectionCount = 0;
  const versionLines = [];

  for (let index = 0; index < lines.length; index += 1) {
    const header = cargoSectionHeader(lines[index]);
    if (header !== null) {
      section = header;
      if (section === "workspace.package") {
        sectionCount += 1;
      }
      continue;
    }
    if (section !== "workspace.package") {
      continue;
    }
    const match = /^(\s*version\s*=\s*)(["'])([^"']*)\2(\s*(?:#.*)?)$/.exec(lines[index]);
    if (match) {
      versionLines.push({ value: match[3] });
    }
  }

  if (sectionCount !== 1) {
    throw new Error(`src-tauri/Cargo.toml must contain exactly one [workspace.package] section; found ${sectionCount}`);
  }
  if (versionLines.length !== 1) {
    throw new Error(`src-tauri/Cargo.toml [workspace.package] must contain exactly one version field; found ${versionLines.length}`);
  }
  return versionLines[0].value;
}

export function replaceCargoWorkspacePackageVersion(cargoText, version) {
  const lines = cargoText.split(/\r?\n/);
  let section = null;
  let sectionCount = 0;
  let replacements = 0;

  for (let index = 0; index < lines.length; index += 1) {
    const header = cargoSectionHeader(lines[index]);
    if (header !== null) {
      section = header;
      if (section === "workspace.package") {
        sectionCount += 1;
      }
      continue;
    }
    if (section !== "workspace.package") {
      continue;
    }
    const match = /^(\s*version\s*=\s*)(["'])([^"']*)\2(\s*(?:#.*)?)$/.exec(lines[index]);
    if (match) {
      lines[index] = `${match[1]}${match[2]}${version}${match[2]}${match[4]}`;
      replacements += 1;
    }
  }

  if (sectionCount !== 1) {
    throw new Error(`src-tauri/Cargo.toml must contain exactly one [workspace.package] section; found ${sectionCount}`);
  }
  if (replacements !== 1) {
    throw new Error(`src-tauri/Cargo.toml [workspace.package] must contain exactly one version field; found ${replacements}`);
  }
  return lines.join(cargoText.includes("\r\n") ? "\r\n" : "\n");
}

function parseCargoPackageMetadata(cargoText, label, requireInheritedVersion) {
  const lines = cargoText.split(/\r?\n/);
  let section = null;
  let packageSections = 0;
  const names = [];
  let inheritedVersionFields = 0;
  let directVersionFields = 0;

  for (const line of lines) {
    const header = cargoSectionHeader(line);
    if (header !== null) {
      section = header;
      if (section === "package") {
        packageSections += 1;
      }
      continue;
    }
    if (section !== "package") {
      continue;
    }
    const cleaned = stripTomlComment(line).trim();
    const nameMatch = /^name\s*=\s*(["'])([^"']+)\1$/.exec(cleaned);
    if (nameMatch) {
      names.push(nameMatch[2]);
    }
    const inheritedMatch = /^version\.workspace\s*=\s*(true|false)$/.exec(cleaned);
    if (inheritedMatch) {
      inheritedVersionFields += 1;
      if (inheritedMatch[1] !== "true") {
        throw new Error(`${label} [package] version.workspace must be true`);
      }
    }
    if (/^version\s*=/.test(cleaned)) {
      directVersionFields += 1;
    }
  }

  if (packageSections !== 1) {
    throw new Error(`${label} must contain exactly one [package] section; found ${packageSections}`);
  }
  if (names.length !== 1) {
    throw new Error(`${label} [package] must contain exactly one package name; found ${names.length}`);
  }
  if (requireInheritedVersion) {
    if (directVersionFields !== 0) {
      throw new Error(`${label} must not declare an independent [package] version; use version.workspace = true`);
    }
    if (inheritedVersionFields !== 1) {
      throw new Error(`${label} [package] must declare version.workspace = true`);
    }
  }
  return names[0];
}

function inspectCargoWorkspace(rootDir, cargoText, lockText) {
  const members = extractCargoWorkspaceMembers(cargoText);
  if (members === null) {
    return {
      present: false,
      members: [],
      packageNames: [],
      workspaceVersion: null,
      lockVersions: {},
    };
  }
  if (!members.includes(".")) {
    throw new Error("src-tauri/Cargo.toml [workspace] members must include the root host package '.'");
  }

  const srcTauriDir = resolve(rootDir, "src-tauri");
  const packageNames = [];
  // Read only the explicit member paths; never glob or discover workspace files.
  const seenManifestPaths = new Set();
  for (const member of members) {
    if (
      member.startsWith("/") ||
      isAbsolute(member) ||
      /^[A-Za-z]:[\\/]/.test(member) ||
      member.split(/[\\/]/).includes("..")
    ) {
      throw new Error(`src-tauri/Cargo.toml [workspace] member '${member}' must stay inside src-tauri`);
    }
    const manifestPath = resolve(srcTauriDir, member, "Cargo.toml");
    const relativeManifest = relative(srcTauriDir, manifestPath);
    if (relativeManifest.startsWith("..") || isAbsolute(relativeManifest)) {
      throw new Error(`src-tauri/Cargo.toml [workspace] member '${member}' escapes src-tauri`);
    }
    if (seenManifestPaths.has(manifestPath)) {
      throw new Error(`src-tauri/Cargo.toml [workspace] member '${member}' resolves to a duplicate manifest path`);
    }
    seenManifestPaths.add(manifestPath);

    const isRoot = member === ".";
    const manifestText = isRoot
      ? cargoText
      : readUtf8(manifestPath, `${member}/Cargo.toml`);
    const label = isRoot ? "src-tauri/Cargo.toml" : `src-tauri/${member}/Cargo.toml`;
    const packageName = parseCargoPackageMetadata(manifestText, label, !isRoot);
    if (packageNames.includes(packageName)) {
      throw new Error(`Cargo workspace package name '${packageName}' is declared more than once`);
    }
    packageNames.push(packageName);
  }

  if (lockText === null || lockText === undefined) {
    throw new Error("src-tauri/Cargo.lock is required when Cargo workspace metadata is present");
  }
  return {
    present: true,
    members,
    packageNames,
    workspaceVersion: extractCargoWorkspacePackageVersion(cargoText),
    lockVersions: extractCargoLockPackageVersions(lockText, packageNames),
  };
}

function validateCargoWorkspaceConsistency(workspace, metadataVersion) {
  if (!workspace.present) {
    return;
  }
  if (workspace.workspaceVersion !== metadataVersion) {
    throw new Error(
      `src-tauri/Cargo.toml [workspace.package] version=${workspace.workspaceVersion} does not match generated metadata ${metadataVersion}`,
    );
  }
  for (const packageName of workspace.packageNames) {
    const lockVersion = workspace.lockVersions[packageName];
    if (lockVersion !== metadataVersion) {
      throw new Error(
        `src-tauri/Cargo.lock package '${packageName}'=${lockVersion} does not match generated metadata ${metadataVersion}`,
      );
    }
  }
}

function parseCargoLockBlocks(lockText) {
  const lines = lockText.split(/\r?\n/);
  const blocks = [];
  let current = null;

  const finishCurrent = () => {
    if (current !== null) {
      blocks.push(current);
      current = null;
    }
  };

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (/^\s*\[\[package\]\]\s*$/.test(line)) {
      finishCurrent();
      current = { start: index, name: null, nameFields: 0, versionLines: [] };
      continue;
    }
    if (current === null) {
      continue;
    }
    if (/^\s*\[/.test(line)) {
      finishCurrent();
      continue;
    }

    const nameLine = stripTomlComment(line);
    const nameMatch = nameLine.match(/^\s*name\s*=\s*(["'])([^"']+)\1\s*$/);
    if (nameMatch) {
      current.nameFields += 1;
      if (current.nameFields > 1) {
        throw new Error(
          `src-tauri/Cargo.lock package block starting at line ${current.start + 1} ` +
          `must contain exactly one name field; found ${current.nameFields}`,
        );
      }
      current.name = nameMatch[2];
    }
    const versionMatch = line.match(/^(\s*version\s*=\s*)(["'])([^"']+)\2(\s*(?:#.*)?)$/);
    if (versionMatch) {
      current.versionLines.push({
        index,
        prefix: versionMatch[1],
        quote: versionMatch[2],
        suffix: versionMatch[4],
        value: versionMatch[3],
      });
    }
  }
  finishCurrent();
  return { lines, blocks };
}

function findCargoLockLocalBlocks(lockText, packageNames) {
  if (!Array.isArray(packageNames) || packageNames.length === 0) {
    throw new Error("Cargo lock local package names must be a non-empty array");
  }
  const uniquePackageNames = new Set();
  for (const packageName of packageNames) {
    if (typeof packageName !== "string" || packageName.length === 0) {
      throw new Error("Cargo lock local package names must be non-empty strings");
    }
    if (uniquePackageNames.has(packageName)) {
      throw new Error(`Cargo lock local package name '${packageName}' is duplicated`);
    }
    uniquePackageNames.add(packageName);
  }

  const { lines, blocks } = parseCargoLockBlocks(lockText);
  const byName = new Map();
  for (const packageName of packageNames) {
    const matches = blocks.filter((block) => block.name === packageName);
    if (matches.length !== 1) {
      throw new Error(
        `src-tauri/Cargo.lock must contain exactly one package named "${packageName}"; found ${matches.length}`,
      );
    }
    const block = matches[0];
    if (block.versionLines.length !== 1) {
      throw new Error(
        `src-tauri/Cargo.lock package "${packageName}" must contain exactly one version field; found ${block.versionLines.length}`,
      );
    }
    byName.set(packageName, block);
  }
  return { lines, blocks: byName };
}

export function extractCargoLockPackageVersions(lockText, packageNames) {
  const { blocks } = findCargoLockLocalBlocks(lockText, packageNames);
  const versions = {};
  for (const packageName of packageNames) {
    versions[packageName] = blocks.get(packageName).versionLines[0].value;
  }
  return versions;
}

export function replaceCargoLockPackageVersions(lockText, packageNames, version) {
  const { lines, blocks } = findCargoLockLocalBlocks(lockText, packageNames);
  for (const packageName of packageNames) {
    const versionLine = blocks.get(packageName).versionLines[0];
    lines[versionLine.index] = `${versionLine.prefix}${versionLine.quote}${version}${versionLine.quote}${versionLine.suffix}`;
  }
  return lines.join(lockText.includes("\r\n") ? "\r\n" : "\n");
}

function findCargoLockFlintBlock(lockText) {
  const { lines, blocks } = findCargoLockLocalBlocks(lockText, ["flint"]);
  return { lines, block: blocks.get("flint") };
}

export function extractCargoLockPackageVersion(lockText) {
  return findCargoLockFlintBlock(lockText).block.versionLines[0].value;
}

export function replaceCargoLockPackageVersion(lockText, version) {
  const { lines, block } = findCargoLockFlintBlock(lockText);
  const versionLine = block.versionLines[0];
  lines[versionLine.index] = `${versionLine.prefix}${versionLine.quote}${version}${versionLine.quote}${versionLine.suffix}`;
  return lines.join(lockText.includes("\r\n") ? "\r\n" : "\n");
}

function parseLoadedVersion(value, label, allowLegacyCurrent) {
  const text = String(value ?? "").trim();
  if (allowLegacyCurrent && LEGACY_VERSION_PATTERN.test(text)) {
    return text;
  }
  return parseReleaseVersion(value, label).version;
}

export function compareReleaseCores(left, right) {
  const a = parseReleaseVersion(left, "left version");
  const b = parseReleaseVersion(right, "right version");
  for (const key of ["year", "month", "releaseOrdinal"]) {
    if (a[key] !== b[key]) {
      return a[key] < b[key] ? -1 : 1;
    }
  }
  return 0;
}

function loadVersionInputs(root, options = {}) {
  const rootDir = resolve(root);
  const entries = {};
  for (const [key, relativePath] of Object.entries(VERSION_FILE_PATHS)) {
    const path = join(rootDir, relativePath);
    let mode = 0o644;
    try {
      mode = statSync(path).mode & 0o777;
    } catch (error) {
      if (key === "cargoLock" && error && error.code === "ENOENT") {
        continue;
      }
      throw new Error(`Cannot stat ${relativePath}: ${formatError(error)}`);
    }
    entries[key] = {
      key,
      relativePath,
      path,
      content: readUtf8(path, relativePath),
      mode,
    };
  }

  entries.version.value = parseLoadedVersion(entries.version.content, "VERSION", options.allowLegacyCurrent);
  entries.package.json = parseJson(entries.package.content, "package.json");
  entries.package.value = parseLoadedVersion(entries.package.json.version, "package.json version", options.allowLegacyCurrent);
  entries.tauri.json = parseJson(entries.tauri.content, "src-tauri/tauri.conf.json");
  entries.tauri.value = parseLoadedVersion(entries.tauri.json.version, "src-tauri/tauri.conf.json version", options.allowLegacyCurrent);
  entries.cargo.value = parseLoadedVersion(
    extractCargoPackageVersion(entries.cargo.content),
    "src-tauri/Cargo.toml package version",
    options.allowLegacyCurrent,
  );
  if (entries.cargoLock) {
    entries.cargoLock.value = parseLoadedVersion(
      extractCargoLockPackageVersion(entries.cargoLock.content),
      "src-tauri/Cargo.lock flint package version",
      options.allowLegacyCurrent,
    );
  }
  entries.cargo.workspace = inspectCargoWorkspace(
    rootDir,
    entries.cargo.content,
    entries.cargoLock ? entries.cargoLock.content : null,
  );

  return { root: rootDir, entries };
}

export function readVersionState(root = repoRootFromScript()) {
  const { entries } = loadVersionInputs(root);
  const versions = {
    version: entries.version.value,
    package: entries.package.value,
    tauri: entries.tauri.value,
    cargo: entries.cargo.value,
  };
  const contents = {
    version: entries.version.content,
    package: entries.package.content,
    tauri: entries.tauri.content,
    cargo: entries.cargo.content,
  };
  if (entries.cargoLock) {
    versions.cargoLock = entries.cargoLock.value;
    contents.cargoLock = entries.cargoLock.content;
  }
  return { version: entries.version.value, versions, contents, cargoWorkspace: entries.cargo.workspace };
}

// Shared by normal validation and apply preflight so inconsistent generated metadata cannot be repaired silently.
function validateGeneratedMetadataConsistency(versions, cargoWorkspace) {
  const metadataEntries = ["package", "tauri", "cargo"].map((key) => [key, versions[key]]);
  const metadataVersion = metadataEntries[0][1];
  const metadataMismatches = metadataEntries.filter(([, value]) => value !== metadataVersion);
  if (metadataMismatches.length > 0) {
    throw new Error(
      `Generated version metadata is inconsistent with package.json=${metadataVersion}: ` +
      metadataMismatches.map(([key, value]) => `${VERSION_FILE_PATHS[key]}=${value}`).join(", "),
    );
  }
  validateCargoWorkspaceConsistency(cargoWorkspace, metadataVersion);
  if (versions.cargoLock && versions.cargoLock !== metadataVersion) {
    throw new Error(
      `src-tauri/Cargo.lock flint package=${versions.cargoLock} does not match generated metadata ${metadataVersion}`,
    );
  }
  return metadataVersion;
}

export function validateVersionConsistency(root = repoRootFromScript(), expectedVersion = undefined, options = {}) {
  const state = readVersionState(root);
  const metadataVersion = validateGeneratedMetadataConsistency(state.versions, state.cargoWorkspace);

  if (expectedVersion !== undefined) {
    const expected = parseReleaseVersion(expectedVersion, "expected version").version;
    const offenders = Object.entries(state.versions)
      .filter(([, value]) => value !== expected)
      .map(([key, value]) => `${VERSION_FILE_PATHS[key]}=${value}`);
    if (offenders.length > 0) {
      throw new Error(`Version files must all equal the build version ${expected}; ${offenders.join(", ")}`);
    }
    return { ...state, metadataVersion, exactMetadata: true, pendingVersionCommit: false };
  }

  if (options.requireExact) {
    const offenders = Object.entries(state.versions)
      .filter(([, value]) => value !== state.version)
      .map(([key, value]) => `${VERSION_FILE_PATHS[key]}=${value}`);
    if (offenders.length > 0) {
      throw new Error(`Version files are not exact; VERSION=${state.version}, ${offenders.join(", ")}`);
    }
    return { ...state, metadataVersion, exactMetadata: true, pendingVersionCommit: false };
  }

  if (options.allowDivergentMetadata) {
    const metadataCoreDirection = compareReleaseCores(metadataVersion, state.version);
    if (metadataCoreDirection > 0) {
      throw new Error(
        `Generated version metadata ${metadataVersion} is ahead of VERSION ${state.version} by year/month/release ordinal; ` +
        "SHA prereleases are not chronological ordering signals.",
      );
    }
    const exactMetadata = metadataVersion === state.version;
    return {
      ...state,
      metadataVersion,
      exactMetadata,
      pendingVersionCommit: !exactMetadata,
    };
  }

  const metadataDirection = compareReleaseVersions(metadataVersion, state.version);
  if (metadataDirection > 0) {
    throw new Error(
      `Generated version metadata ${metadataVersion} is ahead of VERSION ${state.version}; the workflow derives releases from git`,
    );
  }

  const exactMetadata = metadataDirection === 0;
  return {
    ...state,
    metadataVersion,
    exactMetadata,
    pendingVersionCommit: !exactMetadata,
  };
}

export function planVersionUpdate(root, targetVersion) {
  const target = parseReleaseVersion(targetVersion, "target version").version;
  const { root: rootDir, entries } = loadVersionInputs(root, { allowLegacyCurrent: true });
  const workspace = entries.cargo.workspace;
  const generatedVersions = {
    package: entries.package.value,
    tauri: entries.tauri.value,
    cargo: entries.cargo.value,
  };
  if (entries.cargoLock) {
    generatedVersions.cargoLock = entries.cargoLock.value;
  }
  validateGeneratedMetadataConsistency(generatedVersions, workspace);

  const packageJson = structuredClone(entries.package.json);
  const tauriConfig = structuredClone(entries.tauri.json);
  packageJson.version = target;
  tauriConfig.version = target;

  // Member manifests inherit this value; only the root manifest and lockfile are release files.
  let cargoContent = replaceCargoPackageVersion(entries.cargo.content, target);
  if (workspace.present) {
    cargoContent = replaceCargoWorkspacePackageVersion(cargoContent, target);
  }

  const updates = [
    {
      ...entries.version,
      newContent: `${target}${detectEol(entries.version.content)}`,
    },
    {
      ...entries.package,
      newContent: renderJson(entries.package.content, packageJson),
    },
    {
      ...entries.tauri,
      newContent: renderJson(entries.tauri.content, tauriConfig),
    },
    {
      ...entries.cargo,
      newContent: cargoContent,
    },
  ];
  if (entries.cargoLock) {
    updates.push({
      ...entries.cargoLock,
      newContent: workspace.present
        ? replaceCargoLockPackageVersions(entries.cargoLock.content, workspace.packageNames, target)
        : replaceCargoLockPackageVersion(entries.cargoLock.content, target),
    });
  }

  // Verify all staged text before touching the worktree.
  if (parseReleaseVersion(updates[0].newContent, "updated VERSION").version !== target) {
    throw new Error("Internal error: VERSION was not updated to the target version");
  }
  if (parseJson(updates[1].newContent, "updated package.json").version !== target) {
    throw new Error("Internal error: package.json was not updated to the target version");
  }
  if (parseJson(updates[2].newContent, "updated src-tauri/tauri.conf.json").version !== target) {
    throw new Error("Internal error: src-tauri/tauri.conf.json was not updated to the target version");
  }
  if (extractCargoPackageVersion(updates[3].newContent) !== target) {
    throw new Error("Internal error: src-tauri/Cargo.toml package version was not updated to the target version");
  }
  if (workspace.present) {
    if (extractCargoWorkspacePackageVersion(updates[3].newContent) !== target) {
      throw new Error("Internal error: src-tauri/Cargo.toml [workspace.package] version was not updated to the target version");
    }
    if (!entries.cargoLock) {
      throw new Error("Internal error: Cargo workspace metadata requires a Cargo.lock update");
    }
    const updatedLockVersions = extractCargoLockPackageVersions(updates[4].newContent, workspace.packageNames);
    for (const packageName of workspace.packageNames) {
      if (updatedLockVersions[packageName] !== target) {
        throw new Error(
          `Internal error: src-tauri/Cargo.lock package '${packageName}' was not updated to the target version`,
        );
      }
    }
  } else if (entries.cargoLock && extractCargoLockPackageVersion(updates[4].newContent) !== target) {
    throw new Error("Internal error: src-tauri/Cargo.lock flint package was not updated to the target version");
  }

  return { root: rootDir, targetVersion: target, files: updates };
}

function writeTempFile(filePath, content, mode) {
  const tempPath = join(
    dirname(filePath),
    `.${basename(filePath)}.${process.pid}.${randomBytes(8).toString("hex")}.tmp`,
  );
  let descriptor;
  try {
    descriptor = openSync(tempPath, "wx", mode || 0o644);
    writeFileSync(descriptor, content, "utf8");
    fsyncSync(descriptor);
    closeSync(descriptor);
    descriptor = undefined;
    return tempPath;
  } catch (error) {
    if (descriptor !== undefined) {
      try {
        closeSync(descriptor);
      } catch {
        // Best-effort cleanup only; the original error is more useful.
      }
    }
    try {
      rmSync(tempPath, { force: true });
    } catch {
      // Best-effort cleanup only.
    }
    throw error;
  }
}

export function writeFileAtomically(filePath, content, mode = 0o644) {
  const tempPath = writeTempFile(filePath, content, mode);
  try {
    renameSync(tempPath, filePath);
  } catch (error) {
    try {
      rmSync(tempPath, { force: true });
    } catch {
      // Best-effort cleanup only.
    }
    throw error;
  }
}

export function applyVersion(targetVersion, root = repoRootFromScript()) {
  const plan = planVersionUpdate(root, targetVersion);
  const changed = plan.files.filter((file) => file.content !== file.newContent);
  if (changed.length === 0) {
    return { changed: false, version: plan.targetVersion, files: [] };
  }

  const staged = [];
  const renamed = [];
  try {
    for (const file of changed) {
      staged.push({ file, tempPath: writeTempFile(file.path, file.newContent, file.mode) });
    }
    for (const entry of staged) {
      renameSync(entry.tempPath, entry.file.path);
      entry.renamed = true;
      renamed.push(entry.file);
    }

    validateVersionConsistency(plan.root, plan.targetVersion);
    return {
      changed: true,
      version: plan.targetVersion,
      files: changed.map((file) => file.relativePath),
    };
  } catch (error) {
    const cleanupErrors = [];
    for (const entry of staged) {
      if (entry.renamed) {
        continue;
      }
      try {
        rmSync(entry.tempPath, { force: true });
      } catch (cleanupError) {
        cleanupErrors.push(`remove ${entry.tempPath}: ${formatError(cleanupError)}`);
      }
    }

    for (const file of [...renamed].reverse()) {
      try {
        writeFileAtomically(file.path, file.content, file.mode);
      } catch (rollbackError) {
        cleanupErrors.push(`restore ${file.relativePath}: ${formatError(rollbackError)}`);
      }
    }

    const rollbackNote = cleanupErrors.length === 0
      ? "All changed files were restored."
      : `Rollback encountered errors: ${cleanupErrors.join("; ")}`;
    throw new Error(`Atomic version update failed: ${formatError(error)}. ${rollbackNote}`);
  }
}
