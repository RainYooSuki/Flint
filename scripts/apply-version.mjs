#!/usr/bin/env node

import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import {
  applyVersion,
  planVersionUpdate,
  repoRootFromScript,
} from "./version-files.mjs";

function usage() {
  return "Usage: node scripts/apply-version.mjs <YYYY.M.N-sha7> [--root <path>] [--dry-run]";
}

export function parseCliArguments(argv = process.argv.slice(2)) {
  let version;
  let root = repoRootFromScript();
  let dryRun = false;

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--dry-run") {
      dryRun = true;
      continue;
    }
    if (argument === "--root") {
      const value = argv[index + 1];
      if (!value) {
        throw new Error(`--root requires a path. ${usage()}`);
      }
      root = resolve(value);
      index += 1;
      continue;
    }
    if (argument.startsWith("--root=")) {
      const value = argument.slice("--root=".length);
      if (!value) {
        throw new Error(`--root requires a path. ${usage()}`);
      }
      root = resolve(value);
      continue;
    }
    if (argument.startsWith("-")) {
      throw new Error(`Unknown option ${argument}. ${usage()}`);
    }
    if (version !== undefined) {
      throw new Error(`Unexpected extra argument ${argument}. ${usage()}`);
    }
    version = argument;
  }

  if (!version) {
    throw new Error(usage());
  }

  return { version, root, dryRun };
}

export function main(argv = process.argv.slice(2)) {
  const options = parseCliArguments(argv);
  if (options.dryRun) {
    const plan = planVersionUpdate(options.root, options.version);
    const changed = plan.files.filter((file) => file.content !== file.newContent);
    if (changed.length === 0) {
      console.log(`Flint version is already ${plan.targetVersion}; nothing would change.`);
    } else {
      console.log(`Dry run: would set Flint version to ${plan.targetVersion} in:`);
      for (const file of changed) {
        console.log(`- ${file.relativePath}`);
      }
    }
    return;
  }

  const result = applyVersion(options.version, options.root);
  if (!result.changed) {
    console.log(`Flint version is already ${result.version}; no files changed.`);
    return;
  }
  console.log(`Flint version set to ${result.version} atomically in:`);
  for (const file of result.files) {
    console.log(`- ${file}`);
  }
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
    console.error(`apply-version: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  }
}