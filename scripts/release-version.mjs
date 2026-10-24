#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { appendFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const RELEASE_VERSION_PATTERN = /^([1-9]\d{3})\.([1-9]|1[0-2])\.([1-9]\d*)-([0-9a-f]{7}|g[0-9]{7})$/;
export const LEGACY_VERSION_PATTERN = /^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)$/;
export const VERSION_SCHEME_DESCRIPTION = "YYYY.M.N-sha7, where N is the one-based monthly release ordinal (reset to 1 each UTC month) and an all-numeric leading-zero sha7 uses a g-prefixed prerelease fallback";
export const FULL_GIT_SHA_PATTERN = /^(?:[0-9a-f]{40}|[0-9a-f]{64})$/;

export function repositoryRootFromScript(importMetaUrl = import.meta.url) {
  return resolve(dirname(fileURLToPath(importMetaUrl)), "..");
}

function formatError(error) {
  return error && typeof error.message === "string" ? error.message : String(error);
}

export function extractSha7(commitSha, label = "commit SHA") {
  const text = String(commitSha ?? "").trim();
  if (!FULL_GIT_SHA_PATTERN.test(text)) {
    throw new Error(`${label} must be a full lowercase 40- or 64-character hexadecimal commit SHA; got ${JSON.stringify(text)}`);
  }
  return text.slice(0, 7);
}

function normalizeReleaseOrdinal(releaseOrdinal) {
  const text = String(releaseOrdinal ?? "").trim();
  if (!/^[1-9]\d*$/.test(text)) {
    throw new Error(`monthly release ordinal must be a positive integer without leading zeroes; got ${JSON.stringify(text)}`);
  }
  const value = Number(text);
  if (!Number.isSafeInteger(value)) {
    throw new Error(`monthly release ordinal is too large to represent safely: ${text}`);
  }
  return value;
}

export function utcYearMonthFromTimestamp(commitTimestamp) {
  const text = String(commitTimestamp ?? "").trim();
  if (!/^[0-9]+$/.test(text)) {
    throw new Error(`commit timestamp must be non-negative Unix seconds; got ${JSON.stringify(text)}`);
  }
  const seconds = Number(text);
  if (!Number.isSafeInteger(seconds)) {
    throw new Error(`commit timestamp is too large to represent safely: ${text}`);
  }
  const date = new Date(seconds * 1000);
  if (Number.isNaN(date.getTime())) {
    throw new Error(`commit timestamp does not represent a valid date: ${text}`);
  }
  const year = date.getUTCFullYear();
  const month = date.getUTCMonth() + 1;
  if (year < 1000 || year > 9999) {
    throw new Error(`commit UTC year must be four digits in the supported range; got ${year}`);
  }
  return { year, month };
}

export function prereleaseIdentifierFromSha7(sha7) {
  const value = String(sha7 ?? "").trim();
  if (!/^[0-9a-f]{7}$/.test(value)) {
    throw new Error(`sha7 must be exactly seven lowercase hexadecimal characters; got ${JSON.stringify(value)}`);
  }
  // SemVer forbids numeric prerelease identifiers with leading zeroes.
  if (/^0[0-9]{6}$/.test(value)) {
    return `g${value}`;
  }
  return value;
}

export function computeReleaseVersion({ commitSha, commitTimestamp, releaseOrdinal }) {
  const fullSha = String(commitSha ?? "").trim();
  const sha7 = extractSha7(fullSha);
  const ordinal = normalizeReleaseOrdinal(releaseOrdinal);
  const { year, month } = utcYearMonthFromTimestamp(commitTimestamp);
  const prerelease = prereleaseIdentifierFromSha7(sha7);
  const version = `${year}.${month}.${ordinal}-${prerelease}`;
  return {
    version,
    tag: `flint-v${version}`,
    year,
    month,
    releaseOrdinal: ordinal,
    sha7,
    prerelease,
    usesShaFallback: prerelease.startsWith("g"),
    commitSha: fullSha,
  };
}

export function parseReleaseVersion(value, label = "version") {
  const text = String(value ?? "").trim();
  const match = text.match(RELEASE_VERSION_PATTERN);
  if (!match) {
    throw new Error(
      `${label} must match ${VERSION_SCHEME_DESCRIPTION}; got ${JSON.stringify(text)}`,
    );
  }

  const [, yearText, monthText, ordinalText, prerelease] = match;
  if (prerelease.startsWith("g")) {
    const sha7 = prerelease.slice(1);
    if (!/^0[0-9]{6}$/.test(sha7)) {
      throw new Error(
        `${label} uses the g-prefixed fallback only for an all-numeric sha7 beginning with 0; got ${JSON.stringify(text)}`,
      );
    }
  } else if (/^[0-9]+$/.test(prerelease) && prerelease.startsWith("0")) {
    throw new Error(
      `${label} has an invalid numeric prerelease with a leading zero; use g${prerelease} instead: ${JSON.stringify(text)}`,
    );
  }

  const year = Number(yearText);
  const month = Number(monthText);
  const releaseOrdinal = Number(ordinalText);
  if (!Number.isSafeInteger(releaseOrdinal)) {
    throw new Error(`${label} monthly release ordinal is too large to represent safely: ${JSON.stringify(text)}`);
  }

  return {
    version: text,
    tag: `flint-v${text}`,
    year,
    month,
    releaseOrdinal,
    sha7: prerelease.startsWith("g") ? prerelease.slice(1) : prerelease,
    prerelease,
    usesShaFallback: prerelease.startsWith("g"),
  };
}

export function releaseTag(value) {
  return parseReleaseVersion(value).tag;
}

export function parseReleaseTag(tag) {
  const text = String(tag ?? "").trim();
  if (!text.startsWith("flint-v")) {
    throw new Error(`release tag must start with flint-v; got ${JSON.stringify(text)}`);
  }
  return parseReleaseVersion(text.slice("flint-v".length), "release tag version");
}

export function isLegacyFlintTag(tag) {
  const text = String(tag ?? "").trim();
  return text.startsWith("flint-v") && LEGACY_VERSION_PATTERN.test(text.slice("flint-v".length));
}

function comparePrereleaseIdentifiers(left, right) {
  const leftNumeric = /^[0-9]+$/.test(left);
  const rightNumeric = /^[0-9]+$/.test(right);
  if (leftNumeric && rightNumeric) {
    const leftValue = Number(left);
    const rightValue = Number(right);
    if (leftValue !== rightValue) {
      return leftValue < rightValue ? -1 : 1;
    }
    return 0;
  }
  if (leftNumeric !== rightNumeric) {
    return leftNumeric ? -1 : 1;
  }
  if (left === right) {
    return 0;
  }
  return left < right ? -1 : 1;
}

export function compareReleaseVersions(left, right) {
  const a = parseReleaseVersion(left, "left version");
  const b = parseReleaseVersion(right, "right version");
  for (const key of ["year", "month", "releaseOrdinal"]) {
    if (a[key] !== b[key]) {
      return a[key] < b[key] ? -1 : 1;
    }
  }
  return comparePrereleaseIdentifiers(a.prerelease, b.prerelease);
}

export function isReleaseVersion(value) {
  try {
    parseReleaseVersion(value);
    return true;
  } catch {
    return false;
  }
}

function runGit(root, args) {
  const result = spawnSync("git", ["-C", root, ...args], {
    encoding: "utf8",
    windowsHide: true,
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.error) {
    throw new Error(`Failed to run git ${args.join(" ")}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    const detail = (result.stderr || result.stdout || "").trim();
    throw new Error(`git ${args.join(" ")} failed with exit code ${result.status}${detail ? `: ${detail}` : ""}`);
  }
  return result.stdout.trim();
}

function readChangedPaths(root, commitSha, gitRunner) {
  let output;
  try {
    output = gitRunner(root, [
      "diff-tree",
      "--root",
      "--no-commit-id",
      "--name-only",
      "--no-renames",
      "-r",
      commitSha,
    ]);
  } catch (error) {
    throw new Error(`Cannot inspect changed paths for release source ${commitSha}: ${formatError(error)}`);
  }
  return output.split(/\r?\n/).map((path) => path.trim()).filter(Boolean);
}

function isVersionLedgerCommit(root, commitSha, gitRunner) {
  const paths = readChangedPaths(root, commitSha, gitRunner);
  return paths.length === 1 && paths[0] === "VERSION";
}

function normalizeTagRecord(record, index) {
  if (!record || typeof record !== "object") {
    throw new Error(`release tag record ${index} is not an object`);
  }
  const fullSha = String(record.commitSha ?? "").trim();
  const sha7 = extractSha7(fullSha, `release tag record ${index} source commit`);
  const parsed = parseReleaseVersion(record.version, `release tag record ${index} version`);
  const date = utcYearMonthFromTimestamp(record.commitTimestamp);
  if (parsed.sha7 !== sha7 || parsed.year !== date.year || parsed.month !== date.month) {
    throw new Error(
      `Release tag ${parsed.tag} is inconsistent with source commit ${fullSha} (${date.year}-${String(date.month).padStart(2, "0")})`,
    );
  }
  return {
    tag: parsed.tag,
    version: parsed.version,
    year: date.year,
    month: date.month,
    releaseOrdinal: parsed.releaseOrdinal,
    sha7: parsed.sha7,
    prerelease: parsed.prerelease,
    usesShaFallback: parsed.usesShaFallback,
    commitSha: fullSha,
    commitTimestamp: String(record.commitTimestamp).trim(),
  };
}

export function selectMonthlyReleaseVersion({ commitSha, commitTimestamp, releaseTags }) {
  const sourceSha = String(commitSha ?? "").trim();
  extractSha7(sourceSha, "release source commit");
  const sourceDate = utcYearMonthFromTimestamp(commitTimestamp);
  if (!Array.isArray(releaseTags)) {
    throw new Error("releaseTags must be an array of valid release tag records");
  }
  const normalizedTags = releaseTags.map((record, index) => normalizeTagRecord(record, index));
  const sourceMonthLabel = `${sourceDate.year}-${String(sourceDate.month).padStart(2, "0")}`;

  const headTags = normalizedTags.filter((record) => record.commitSha === sourceSha);
  if (headTags.length > 1) {
    throw new Error(
      `Refusing to choose a release version: multiple valid monthly release tags point at HEAD ${sourceSha}: ` +
      headTags.map((record) => record.tag).join(", "),
    );
  }
  if (headTags.length === 1) {
    const existing = headTags[0];
    return {
      version: existing.version,
      tag: existing.tag,
      year: existing.year,
      month: existing.month,
      releaseOrdinal: existing.releaseOrdinal,
      sha7: existing.sha7,
      prerelease: existing.prerelease,
      usesShaFallback: existing.usesShaFallback,
      commitSha: sourceSha,
      commitTimestamp: String(commitTimestamp).trim(),
      reusedExistingTag: true,
    };
  }

  const monthTags = normalizedTags.filter(
    (record) => record.year === sourceDate.year && record.month === sourceDate.month,
  );
  const ordinals = new Set();
  const commits = new Set();
  for (const record of monthTags) {
    if (ordinals.has(record.releaseOrdinal)) {
      throw new Error(
        `Monthly release ordinals for ${sourceMonthLabel} contain duplicate N=${record.releaseOrdinal} (${record.tag}).`,
      );
    }
    if (commits.has(record.commitSha)) {
      throw new Error(
        `Multiple valid monthly release tags point at the same release commit ${record.commitSha}: ` +
        monthTags.filter((candidate) => candidate.commitSha === record.commitSha).map((candidate) => candidate.tag).join(", "),
      );
    }
    ordinals.add(record.releaseOrdinal);
    commits.add(record.commitSha);
  }

  const sortedOrdinals = [...ordinals].sort((left, right) => left - right);
  const expectedOrdinals = Array.from({ length: commits.size }, (_, index) => index + 1);
  if (
    sortedOrdinals.length !== expectedOrdinals.length ||
    sortedOrdinals.some((value, index) => value !== expectedOrdinals[index])
  ) {
    throw new Error(
      `Monthly release ordinals for ${sourceMonthLabel} are not contiguous from 1: ` +
      `N=[${sortedOrdinals.join(", ")}], distinct release commits=${commits.size}.`,
    );
  }

  const result = computeReleaseVersion({
    commitSha: sourceSha,
    commitTimestamp,
    releaseOrdinal: commits.size + 1,
  });
  return { ...result, reusedExistingTag: false };
}

export function readReleaseTagInventory(root, gitRunner = runGit) {
  const tags = gitRunner(root, ["tag", "--list", "flint-v*"])
    .split(/\r?\n/)
    .map((tag) => tag.trim())
    .filter(Boolean);
  const validTags = [];
  const ignoredTags = [];
  const timestampCache = new Map();
  const ledgerCache = new Map();

  for (const tag of tags) {
    if (isLegacyFlintTag(tag)) {
      console.warn(`Ignoring legacy pre-scheme Flint tag ${tag} for monthly release counting.`);
      ignoredTags.push({ tag, reason: "legacy" });
      continue;
    }

    let parsed;
    try {
      parsed = parseReleaseTag(tag);
    } catch (error) {
      console.warn(`Ignoring malformed Flint tag ${tag} for monthly release counting: ${formatError(error)}`);
      ignoredTags.push({ tag, reason: "malformed" });
      continue;
    }

    let commitSha;
    try {
      // Qualify the ref so a branch or other ref with the same short name cannot
      // influence the release-source commit selected for the tag.
      commitSha = gitRunner(root, ["rev-parse", `refs/tags/${tag}^{commit}`]).trim();
    } catch (error) {
      throw new Error(`Cannot resolve release-source commit for valid-format tag ${tag}: ${formatError(error)}`);
    }
    let sha7;
    try {
      sha7 = extractSha7(commitSha, `release tag ${tag} source commit`);
    } catch (error) {
      throw new Error(`Release tag ${tag} resolved to an invalid commit SHA: ${formatError(error)}`);
    }

    let isLedger = ledgerCache.get(commitSha);
    if (isLedger === undefined) {
      isLedger = isVersionLedgerCommit(root, commitSha, gitRunner);
      ledgerCache.set(commitSha, isLedger);
    }
    if (isLedger) {
      console.warn(`Ignoring release tag ${tag}: source commit ${commitSha} is a VERSION-only ledger commit, not a release source.`);
      ignoredTags.push({ tag, reason: "version-ledger" });
      continue;
    }

    let commitTimestamp = timestampCache.get(commitSha);
    if (commitTimestamp === undefined) {
      try {
        commitTimestamp = gitRunner(root, ["show", "-s", "--format=%ct", commitSha]).trim();
      } catch (error) {
        throw new Error(`Cannot read commit timestamp for release tag ${tag} (${commitSha}): ${formatError(error)}`);
      }
      timestampCache.set(commitSha, commitTimestamp);
    }

    let date;
    try {
      date = utcYearMonthFromTimestamp(commitTimestamp);
    } catch (error) {
      throw new Error(`Release tag ${tag} has an invalid source commit timestamp: ${formatError(error)}`);
    }
    if (parsed.sha7 !== sha7 || parsed.year !== date.year || parsed.month !== date.month) {
      console.warn(
        `Ignoring release tag ${tag}: embedded date/SHA does not match source commit ${commitSha} ` +
        `(${date.year}-${String(date.month).padStart(2, "0")}, sha7=${sha7}).`,
      );
      ignoredTags.push({ tag, reason: "source-mismatch" });
      continue;
    }

    validTags.push({
      tag: parsed.tag,
      version: parsed.version,
      year: date.year,
      month: date.month,
      releaseOrdinal: parsed.releaseOrdinal,
      sha7: parsed.sha7,
      prerelease: parsed.prerelease,
      usesShaFallback: parsed.usesShaFallback,
      commitSha,
      commitTimestamp,
    });
  }

  return { tags, validTags, ignoredTags };
}

export function readGitReleaseMetadata(root, expectedSha, gitRunner = runGit) {
  const commitSha = gitRunner(root, ["rev-parse", "HEAD"]).trim();
  if (expectedSha && commitSha !== expectedSha) {
    throw new Error(`Checked-out HEAD ${commitSha} does not match the workflow commit ${expectedSha}`);
  }
  const commitTimestamp = gitRunner(root, ["show", "-s", "--format=%ct", "HEAD"]).trim();
  const inventory = readReleaseTagInventory(root, gitRunner);
  return {
    commitSha,
    commitTimestamp,
    releaseTags: inventory.validTags,
  };
}

export function calculateReleaseMetadata(options, gitRunner = runGit) {
  let source;
  if (options.sha !== undefined || options.timestamp !== undefined) {
    if (options.sha === undefined || options.timestamp === undefined) {
      throw new Error("Explicit --sha and --timestamp must be supplied together.");
    }
    const inventory = readReleaseTagInventory(options.root, gitRunner);
    source = {
      commitSha: options.sha,
      commitTimestamp: options.timestamp,
      releaseTags: inventory.validTags,
      ignoredTags: inventory.ignoredTags,
    };
  } else {
    source = readGitReleaseMetadata(options.root, options.expectedSha, gitRunner);
  }

  if (options.expectedSha && source.commitSha !== options.expectedSha) {
    throw new Error(`Source commit ${source.commitSha} does not match the workflow commit ${options.expectedSha}`);
  }
  const selected = selectMonthlyReleaseVersion({
    commitSha: source.commitSha,
    commitTimestamp: source.commitTimestamp,
    releaseTags: source.releaseTags,
  });
  return selected;
}

function usage() {
  return [
    "Usage:",
    "  node scripts/release-version.mjs --root <repo> --expected-sha <sha> [--github-output <path>] [--json]",
    "  node scripts/release-version.mjs --root <repo> --sha <full-sha> --timestamp <unix-seconds> [--github-output <path>] [--json]",
    "",
    "Reads globally fetched git tags and HEAD by default. Explicit --sha/--timestamp values make source metadata testable offline; tags are still read with git.",
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
  let root = repositoryRootFromScript();
  let expectedSha;
  let sha;
  let timestamp;
  let githubOutput;
  let json = false;

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--root") {
      root = resolve(requireValue(argv, index, "--root"));
      index += 1;
    } else if (argument.startsWith("--root=")) {
      root = resolve(argument.slice("--root=".length));
    } else if (argument === "--expected-sha") {
      expectedSha = requireValue(argv, index, "--expected-sha");
      index += 1;
    } else if (argument.startsWith("--expected-sha=")) {
      expectedSha = argument.slice("--expected-sha=".length);
    } else if (argument === "--sha") {
      sha = requireValue(argv, index, "--sha");
      index += 1;
    } else if (argument.startsWith("--sha=")) {
      sha = argument.slice("--sha=".length);
    } else if (argument === "--timestamp") {
      timestamp = requireValue(argv, index, "--timestamp");
      index += 1;
    } else if (argument.startsWith("--timestamp=")) {
      timestamp = argument.slice("--timestamp=".length);
    } else if (argument === "--github-output") {
      githubOutput = resolve(requireValue(argv, index, "--github-output"));
      index += 1;
    } else if (argument.startsWith("--github-output=")) {
      githubOutput = resolve(argument.slice("--github-output=".length));
    } else if (argument === "--json") {
      json = true;
    } else if (argument === "--help" || argument === "-h") {
      console.log(usage());
      process.exit(0);
    } else {
      throw new Error(`Unknown argument ${argument}. ${usage()}`);
    }
  }

  if ((sha === undefined) !== (timestamp === undefined)) {
    throw new Error(`--sha and --timestamp must be supplied together. ${usage()}`);
  }
  return { root, expectedSha, sha, timestamp, githubOutput, json };
}

function writeGithubOutput(path, metadata) {
  const lines = [
    `version=${metadata.version}`,
    `tag=${metadata.tag}`,
    `sha7=${metadata.sha7}`,
    `release_ordinal=${metadata.releaseOrdinal}`,
    `source_sha=${metadata.commitSha}`,
    `year=${metadata.year}`,
    `month=${metadata.month}`,
    `prerelease=${metadata.prerelease}`,
    `reused_existing_tag=${metadata.reusedExistingTag ? "true" : "false"}`,
  ];
  appendFileSync(path, `${lines.join("\n")}\n`, "utf8");
}

export function main(argv = process.argv.slice(2), gitRunner = runGit) {
  const options = parseCliArguments(argv);
  const metadata = calculateReleaseMetadata(options, gitRunner);
  if (options.githubOutput) {
    writeGithubOutput(options.githubOutput, metadata);
  }
  if (options.json || !options.githubOutput) {
    console.log(JSON.stringify(metadata, null, 2));
  } else {
    console.log(`Computed Flint version ${metadata.version} (${metadata.tag})`);
  }
  return metadata;
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
    console.error(`release-version: ${formatError(error)}`);
    process.exitCode = 1;
  }
}