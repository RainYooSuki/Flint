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
import { basename, dirname, join, resolve } from "node:path";
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
      current = { start: index, name: null, versionLine: null };
      continue;
    }
    if (current === null) {
      continue;
    }
    if (/^\s*\[/.test(line)) {
      finishCurrent();
      continue;
    }

    const nameMatch = line.match(/^\s*name\s*=\s*"([^"]+)"\s*$/);
    if (nameMatch) {
      current.name = nameMatch[1];
    }
    const versionMatch = line.match(/^(\s*version\s*=\s*)(["'])([^"']+)\2(\s*(?:#.*)?)$/);
    if (versionMatch) {
      current.versionLine = {
        index,
        prefix: versionMatch[1],
        quote: versionMatch[2],
        suffix: versionMatch[4],
        value: versionMatch[3],
      };
    }
  }
  finishCurrent();
  return { lines, blocks };
}

function findCargoLockFlintBlock(lockText) {
  const { lines, blocks } = parseCargoLockBlocks(lockText);
  const flintBlocks = blocks.filter((block) => block.name === "flint");
  if (flintBlocks.length !== 1) {
    throw new Error(`src-tauri/Cargo.lock must contain exactly one package named "flint"; found ${flintBlocks.length}`);
  }
  if (flintBlocks[0].versionLine === null) {
    throw new Error('src-tauri/Cargo.lock flint package must contain a version field');
  }
  return { lines, block: flintBlocks[0] };
}

export function extractCargoLockPackageVersion(lockText) {
  return findCargoLockFlintBlock(lockText).block.versionLine.value;
}

export function replaceCargoLockPackageVersion(lockText, version) {
  const { lines, block } = findCargoLockFlintBlock(lockText);
  const versionLine = block.versionLine;
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
  return { version: entries.version.value, versions, contents };
}

export function validateVersionConsistency(root = repoRootFromScript(), expectedVersion = undefined, options = {}) {
  const state = readVersionState(root);
  const metadataEntries = ["package", "tauri", "cargo"].map((key) => [key, state.versions[key]]);
  const metadataVersion = metadataEntries[0][1];
  const metadataMismatches = metadataEntries.filter(([, value]) => value !== metadataVersion);
  if (metadataMismatches.length > 0) {
    throw new Error(
      `Generated version metadata is inconsistent with package.json=${metadataVersion}: ` +
      metadataMismatches.map(([key, value]) => `${VERSION_FILE_PATHS[key]}=${value}`).join(", "),
    );
  }
  if (state.versions.cargoLock && state.versions.cargoLock !== metadataVersion) {
    throw new Error(
      `src-tauri/Cargo.lock flint package=${state.versions.cargoLock} does not match generated metadata ${metadataVersion}`,
    );
  }

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
  const packageJson = structuredClone(entries.package.json);
  const tauriConfig = structuredClone(entries.tauri.json);
  packageJson.version = target;
  tauriConfig.version = target;

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
      newContent: replaceCargoPackageVersion(entries.cargo.content, target),
    },
  ];
  if (entries.cargoLock) {
    updates.push({
      ...entries.cargoLock,
      newContent: replaceCargoLockPackageVersion(entries.cargoLock.content, target),
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
    throw new Error("Internal error: src-tauri/Cargo.toml was not updated to the target version");
  }
  if (entries.cargoLock && extractCargoLockPackageVersion(updates[4].newContent) !== target) {
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