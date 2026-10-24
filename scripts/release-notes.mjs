#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  repoRootFromScript,
  writeFileAtomically,
} from "./version-files.mjs";
import {
  compareReleaseVersions,
  isLegacyFlintTag,
  parseReleaseVersion,
  releaseTag,
} from "./release-version.mjs";

function usage() {
  return "Usage: node scripts/release-notes.mjs --version <YYYY.M.N-sha7> [--tag <flint-vYYYY.M.N-sha7>] [--root <path>] [--output <path>]";
}

function requireValue(argv, index, option) {
  const value = argv[index + 1];
  if (!value || value.startsWith("--")) {
    throw new Error(`${option} requires a value. ${usage()}`);
  }
  return value;
}

export function parseCliArguments(argv = process.argv.slice(2)) {
  let version;
  let tag;
  let root = repoRootFromScript();
  let output = "release-notes.md";

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--version") {
      version = parseReleaseVersion(requireValue(argv, index, "--version"), "--version").version;
      index += 1;
    } else if (argument.startsWith("--version=")) {
      version = parseReleaseVersion(argument.slice("--version=".length), "--version").version;
    } else if (argument === "--tag") {
      tag = requireValue(argv, index, "--tag");
      index += 1;
    } else if (argument.startsWith("--tag=")) {
      tag = argument.slice("--tag=".length);
    } else if (argument === "--root") {
      root = resolve(requireValue(argv, index, "--root"));
      index += 1;
    } else if (argument.startsWith("--root=")) {
      root = resolve(argument.slice("--root=".length));
    } else if (argument === "--output") {
      output = requireValue(argv, index, "--output");
      index += 1;
    } else if (argument.startsWith("--output=")) {
      output = argument.slice("--output=".length);
    } else if (argument === "--help" || argument === "-h") {
      console.log(usage());
      process.exit(0);
    } else {
      throw new Error(`Unknown argument ${argument}. ${usage()}`);
    }
  }

  if (!version) {
    throw new Error(usage());
  }
  const expectedTag = releaseTag(version);
  tag ??= expectedTag;
  if (tag !== expectedTag) {
    throw new Error(`Tag ${tag} does not match --version ${version}; expected ${expectedTag}`);
  }

  return { version, tag, root, output };
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

export function selectPreviousReleaseTag(tags, version) {
  const target = parseReleaseVersion(version);
  const currentTag = releaseTag(target.version);
  const candidates = [];
  for (const tag of tags) {
    if (!tag.startsWith("flint-v") || tag === currentTag) {
      continue;
    }
    const candidateText = tag.slice("flint-v".length);
    if (isLegacyFlintTag(tag)) {
      console.warn(`Ignoring legacy pre-scheme Flint tag ${tag}; date/commit/SHA tags are the release-note baseline.`);
      continue;
    }
    let candidate;
    try {
      candidate = parseReleaseVersion(candidateText, `tag ${tag}`);
    } catch (error) {
      console.warn(`Ignoring unsupported Flint tag ${tag}: ${error.message}`);
      continue;
    }
    if (compareReleaseVersions(candidate.version, target.version) < 0) {
      candidates.push({ tag, version: candidate.version });
    }
  }

  candidates.sort((left, right) => compareReleaseVersions(left.version, right.version));
  return candidates.length > 0 ? candidates[candidates.length - 1].tag : undefined;
}

export function findPreviousReleaseTag(root, version, gitRunner = runGit) {
  const tags = gitRunner(root, ["tag", "--merged", "HEAD", "--list", "flint-v*"])
    .split(/\r?\n/)
    .filter(Boolean);
  return selectPreviousReleaseTag(tags, version);
}

function markdownLink(label, url) {
  return `[${label}](${url})`;
}

export function buildReleaseNotes({ root, version, tag, environment = process.env, gitRunner = runGit }) {
  const previousTag = findPreviousReleaseTag(root, version, gitRunner);
  const repository = environment.GITHUB_REPOSITORY;
  const serverUrl = (environment.GITHUB_SERVER_URL || "https://github.com").replace(/\/$/, "");
  const commitSha = environment.GITHUB_SHA;
  const runId = environment.GITHUB_RUN_ID;
  const branch = environment.GITHUB_REF_NAME;

  const lines = [
    `## Flint ${version}`,
    "",
  ];

  if (previousTag) {
    const commits = gitRunner(root, [
      "log",
      "--no-merges",
      "--pretty=format:- %s (`%h`)",
      `refs/tags/${previousTag}..HEAD`,
    ]).trim();
    lines.push(`### Changes since \`${previousTag}\``, "");
    lines.push(commits || "- No non-merge commits were found in this range.", "");
  } else {
    lines.push(
      "### Initial release",
      "",
      "No earlier `flint-v*` release tag was found in this repository. This release starts the date-based Flint release series; legacy `flint-vX.Y.Z` tags are not used as a baseline.",
      "",
    );
  }

  lines.push(
    "### Release assets",
    "",
    "- Raw executables for Windows x64, Linux x64, macOS x64, and macOS arm64.",
    "- Matching ZIP archives; each ZIP contains the raw executable and a `RUNNING.txt`.",
    "- `sha.txt` with SHA-256 hashes for every raw executable and ZIP (the checksum file does not checksum itself).",
    "",
    "Verify with `sha256sum -c sha.txt` on Linux, `shasum -a 256 -c sha.txt` on macOS, or `Get-FileHash -Algorithm SHA256` on Windows.",
    "",
    "### Build",
    "",
    `- Version: \`${version}\``,
    "- Version scheme: `YYYY.M.N-sha7` using the UTC commit date, one-based monthly release ordinal that resets to 1 each UTC month, and first seven commit SHA characters; an all-numeric leading-zero SHA7 uses the deterministic `g<sha7>` prerelease fallback.",
  );

  if (repository && commitSha) {
    const shortSha = commitSha.slice(0, 12);
    lines.push(`- Commit: ${markdownLink(shortSha, `${serverUrl}/${repository}/commit/${commitSha}`)}`);
  } else if (commitSha) {
    lines.push(`- Commit: \`${commitSha}\``);
  }
  if (branch) {
    lines.push(`- Branch: \`${branch}\``);
  }
  lines.push(`- Tag: \`${tag}\``);
  if (repository && runId) {
    lines.push(
      `- Workflow run: ${markdownLink(runId, `${serverUrl}/${repository}/actions/runs/${runId}`)}`,
    );
  }
  lines.push(
    "",
    "The executables are currently unsigned. Windows SmartScreen and macOS Gatekeeper may warn when they are first run.",
    "",
  );

  return { body: `${lines.join("\n")}\n`, previousTag };
}

export function main(argv = process.argv.slice(2)) {
  const options = parseCliArguments(argv);
  const result = buildReleaseNotes(options);
  if (options.output === "-") {
    process.stdout.write(result.body);
    return;
  }
  writeFileAtomically(resolve(options.output), result.body, 0o644);
  console.log(`Wrote release notes to ${options.output}`);
  console.log(result.previousTag ? `Previous release tag: ${result.previousTag}` : "Previous release tag: none (initial release)");
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
    console.error(`release-notes: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  }
}