#!/usr/bin/env node
/**
 * Flint architecture boundary checker.
 *
 * Static, phase-aware, and fail-closed. Frontend parsing and module resolution
 * use the installed TypeScript compiler API and the repository tsconfig.
 * Rust phases 1-3 are source checks: comments/strings are masked first, and
 * only `#[cfg(test)] mod ... { ... }` scopes are removed. Production inline
 * modules are intentionally scanned. Phase 4 adds Cargo metadata for workspace,
 * package, and dependency truth plus all-workspace source/typed-port gates;
 * Phase 5 remains a strict future phase.
 */
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import ts from "typescript";
const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "..");
const FRONTEND_ROOT = path.join(REPO_ROOT, "src");
const RUST_SOURCE_ROOT = path.join(REPO_ROOT, "src-tauri", "src");
const RUST_MANIFEST = path.join(REPO_ROOT, "src-tauri", "Cargo.toml");
const PROVIDER_API_SOURCE = "src-tauri/crates/flint-provider-api/src/lib.rs";
const FOUNDATION_WORKSPACE_MEMBERS = [".", "crates/flint-core", "crates/flint-feature-api", "crates/flint-provider-api", "modules/flint-feature-accounts", "modules/flint-feature-launch", "modules/flint-feature-loaders", "modules/flint-feature-mods", "modules/flint-feature-vanilla", "modules/flint-feature-settings", "modules/flint-feature-saves", "providers/flint-mod-provider-curseforge", "providers/flint-mod-provider-modrinth", "providers/flint-loader-provider-fabric", "providers/flint-loader-provider-quilt", "providers/flint-loader-provider-forge", "providers/flint-loader-provider-neoforge", "providers/flint-loader-provider-liteloader", "providers/flint-loader-provider-optifine"];
const COMPOSITION_ROOT = "src/app/composition.ts";
const CONTRIBUTIONS_CONTRACT = "src/core/contributions.ts";
const toPosix = (value) => value.split(path.sep).join("/");
const relFromRepo = (absolutePath) => toPosix(path.relative(REPO_ROOT, absolutePath));
const lineOf = (text, index) => text.slice(0, Math.max(0, index)).split("\n").length;
function isInside(parent, candidate) {
  const relative = path.relative(parent, candidate);
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
}
function realpathOrNull(value) {
  try {
    return fs.realpathSync(value);
  } catch {
    return null;
  }
}
function samePath(left, right) {
  const normalizedLeft = path.resolve(left);
  const normalizedRight = path.resolve(right);
  return process.platform === "win32"
    ? normalizedLeft.toLowerCase() === normalizedRight.toLowerCase()
    : normalizedLeft === normalizedRight;
}
class CliError extends Error {}
export function parsePhase(argv) {
  let phase = 1;
  let seen = false;
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--phase" || argument.startsWith("--phase=")) {
      if (seen) throw new CliError("--phase may be supplied only once.");
      const value = argument === "--phase" ? argv[++index] : argument.slice("--phase=".length);
      if (value === undefined) throw new CliError("--phase requires a value from 1 through 5.");
      phase = value;
      seen = true;
      continue;
    }
    throw new CliError(`Unknown argument: ${argument}`);
  }
  if (!/^[1-5]$/.test(String(phase))) {
    throw new CliError(`Unsupported phase '${phase}'. Expected an integer from 1 through 5.`);
  }
  return Number(phase);
}
function createReporter() {
  const violations = [];
  const seen = new Set();
  const report = (rule, file, line, message) => {
    const normalized = {
      rule,
      file: toPosix(file),
      line: Number.isInteger(line) && line > 0 ? line : 1,
      message,
    };
    const key = `${normalized.rule}|${normalized.file}|${normalized.line}|${normalized.message}`;
    if (seen.has(key)) return;
    seen.add(key);
    violations.push(normalized);
  };
  return { violations, report };
}
function walkFilesFailClosed(root, extensions, report) {
  const files = [];
  if (!fs.existsSync(root)) return files;
  const rootStat = fs.lstatSync(root);
  if (rootStat.isSymbolicLink()) {
    report("ARCH-SYMLINK-001", relFromRepo(root), 1, "symlinked scan roots are forbidden; use a real directory.");
    return files;
  }
  const stack = [root];
  while (stack.length > 0) {
    const directory = stack.pop();
    const entries = fs.readdirSync(directory, { withFileTypes: true })
      .sort((left, right) => left.name.localeCompare(right.name));
    for (const entry of entries) {
      const absolute = path.join(directory, entry.name);
      if (entry.isSymbolicLink()) {
        report("ARCH-SYMLINK-001", relFromRepo(absolute), 1, "symbolic links are forbidden in scanned source trees; replace with a real reviewed path.");
        continue;
      }
      if (entry.isDirectory()) {
        stack.push(absolute);
      } else if (entry.isFile() && extensions.has(path.extname(entry.name))) {
        files.push(absolute);
      }
    }
  }
  return files.sort((left, right) => left.localeCompare(right));
}
function readText(absolutePath) {
  return fs.readFileSync(absolutePath, "utf8");
}
// ---------------------------------------------------------------------------
// TypeScript AST and reviewed frontend policy
// ---------------------------------------------------------------------------
export function loadTypeScriptConfig(repoRoot = REPO_ROOT) {
  const configPath = path.join(repoRoot, "tsconfig.json");
  const configFile = ts.readConfigFile(configPath, ts.sys.readFile);
  if (configFile.error) {
    throw new Error(ts.flattenDiagnosticMessageText(configFile.error.messageText, "\n"));
  }
  const parsed = ts.parseJsonConfigFileContent(configFile.config, ts.sys, repoRoot, undefined, configPath);
  const errors = parsed.errors.filter((diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error);
  if (errors.length > 0) {
    throw new Error(errors.map((diagnostic) => ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n")).join("; "));
  }
  return parsed.options;
}
export function scriptKindForFile(fileName) {
  if (fileName.endsWith(".tsx")) return ts.ScriptKind.TSX;
  if (fileName.endsWith(".ts")) return ts.ScriptKind.TS;
  return ts.ScriptKind.Unknown;
}
export function parseTypeScriptSource(fileName, text) {
  return ts.createSourceFile(fileName, text, ts.ScriptTarget.Latest, true, scriptKindForFile(fileName));
}
export function isRelativeSpecifier(specifier) {
  return specifier.startsWith("./") || specifier.startsWith("../");
}
export function isAbsoluteOrRemoteSpecifier(specifier) {
  return (
    specifier.startsWith("/") ||
    specifier.startsWith("\\") ||
    specifier.startsWith("//") ||
    /^[A-Za-z]:[\\/]/.test(specifier) ||
    /^[A-Za-z][A-Za-z0-9+.-]*:/.test(specifier)
  );
}
export function externalPackageRoot(specifier) {
  if (!specifier.startsWith("@")) return specifier.split("/")[0];
  return specifier.split("/").slice(0, 2).join("/");
}
export function packageRootFromPackageId(packageId) {
  try {
    let rawName = null;
    if (typeof packageId === "string") rawName = packageId;
    else if (packageId && typeof packageId === "object" && !Array.isArray(packageId) && typeof packageId.name === "string") rawName = packageId.name;
    if (!rawName) return null;
    const value = rawName.trim();
    if (!value) return null;
    if (value.startsWith("@")) {
      const [scope, packageAndRest] = value.split("/");
      if (scope.length < 2 || !packageAndRest) return null;
      const packageSegment = packageAndRest.split("/")[0].split("@")[0];
      return packageSegment ? `${scope}/${packageSegment}` : null;
    }
    const packageSegment = value.split("/")[0].split("@")[0];
    return packageSegment || null;
  } catch {
    return null;
  }
}
function declarationPackageRootForRuntimeRoot(runtimeRoot) {
  if (!runtimeRoot || runtimeRoot.startsWith("@types/")) return null;
  if (runtimeRoot.startsWith("@")) {
    const [scope, packageName] = runtimeRoot.split("/");
    if (scope.length < 2 || !packageName) return null;
    return `@types/${scope.slice(1)}__${packageName}`;
  }
  return runtimeRoot.includes("/") ? null : `@types/${runtimeRoot}`;
}
/**
 * TypeScript resolves runtime imports through DefinitelyTyped fallbacks, so
 * the resolved package identity can be `@types/react` for a `react` import.
 * Translate it only when the specifier's declared root maps to that exact
 * declaration package. Direct `@types/*` imports keep their declaration
 * identity, and aliases that do not match remain unchanged, so both fail the
 * phase-3 allowlist.
 */
export function policyPackageRoot(specifier, resolvedPackageRoot) {
  const runtimeRoot = externalPackageRoot(specifier);
  if (runtimeRoot === "@types" || runtimeRoot.startsWith("@types/")) return runtimeRoot;
  if (typeof resolvedPackageRoot !== "string" || !resolvedPackageRoot.startsWith("@types/")) {
    return resolvedPackageRoot;
  }
  return declarationPackageRootForRuntimeRoot(runtimeRoot) === resolvedPackageRoot
    ? runtimeRoot
    : resolvedPackageRoot;
}
function packageRootFromResolvedFile(resolvedFile) {
  const parts = toPosix(resolvedFile).split("/");
  const marker = parts.lastIndexOf("node_modules");
  if (marker === -1 || marker + 1 >= parts.length) return null;
  return parts[marker + 1].startsWith("@")
    ? `${parts[marker + 1]}/${parts[marker + 2] ?? ""}`
    : parts[marker + 1];
}
export function matchesTsconfigAlias(specifier, compilerOptions) {
  const paths = compilerOptions?.paths;
  if (!paths) return false;
  for (const pattern of Object.keys(paths)) {
    if (pattern.endsWith("*")) {
      const prefix = pattern.slice(0, -1);
      if (specifier.startsWith(prefix)) return true;
    } else if (specifier === pattern) {
      return true;
    }
  }
  return false;
}
/**
 * Resolve a frontend module using TypeScript's configured resolver. Local
 * aliases are classified as local targets and may not escape the real src tree.
 */
export function resolveFrontendModule(specifier, containingFile, compilerOptions, frontendRoot = FRONTEND_ROOT) {
  const relative = isRelativeSpecifier(specifier);
  const resolved = ts.resolveModuleName(specifier, containingFile, compilerOptions, ts.sys).resolvedModule;
  let resolvedFile = resolved?.resolvedFileName ?? null;
  const isExternalLibraryImport = resolved?.isExternalLibraryImport === true;
  if (!resolvedFile && relative) {
    const assetPath = path.resolve(path.dirname(containingFile), specifier);
    if (fs.existsSync(assetPath) && fs.statSync(assetPath).isFile()) resolvedFile = assetPath;
  }
  if (!resolvedFile) {
    if (relative) return { kind: "unresolved", specifier };
    if (matchesTsconfigAlias(specifier, compilerOptions) || specifier.startsWith("@/") || specifier.startsWith("~/")) {
      return { kind: "unresolved-alias", specifier };
    }
    return { kind: "external", packageRoot: externalPackageRoot(specifier), specifier };
  }
  if (isExternalLibraryImport) {
    return {
      kind: "external",
      packageRoot: packageRootFromPackageId(resolved.packageId) ?? packageRootFromResolvedFile(resolvedFile) ?? externalPackageRoot(specifier),
      resolvedFile,
      specifier,
    };
  }
  const realRoot = realpathOrNull(frontendRoot);
  const realTarget = realpathOrNull(resolvedFile);
  if (!realRoot || !realTarget) return { kind: "unresolved", specifier };
  if (!isInside(realRoot, realTarget)) {
    return { kind: "escape", resolvedFile: realTarget, specifier };
  }
  return { kind: "local", resolvedFile: realTarget, specifier };
}
function nodeLine(sourceFile, node) {
  return sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)).line + 1;
}
export function collectFrontendModuleReferences(sourceFile) {
  const references = [];
  const add = (node, kind, specifier, valid = true) => {
    references.push({ kind, specifier, valid, line: nodeLine(sourceFile, node) });
  };
  function visit(node) {
    if (ts.isImportDeclaration(node)) {
      if (ts.isStringLiteralLike(node.moduleSpecifier)) add(node, "static", node.moduleSpecifier.text);
    } else if (ts.isExportDeclaration(node) && node.moduleSpecifier && ts.isStringLiteralLike(node.moduleSpecifier)) {
      add(node, "reexport", node.moduleSpecifier.text);
    } else if (ts.isImportEqualsDeclaration(node) && ts.isExternalModuleReference(node.moduleReference)) {
      const expression = node.moduleReference.expression;
      if (ts.isStringLiteralLike(expression)) add(node, "require", expression.text);
    } else if (ts.isImportTypeNode(node)) {
      const argument = node.argument;
      if (ts.isLiteralTypeNode(argument) && ts.isStringLiteralLike(argument.literal)) {
        add(node, "import-type", argument.literal.text);
      }
    } else if (ts.isCallExpression(node)) {
      const isDynamicImport = node.expression.kind === ts.SyntaxKind.ImportKeyword;
      const isRequire = ts.isIdentifier(node.expression) && node.expression.text === "require";
      if (isDynamicImport || isRequire) {
        const kind = isDynamicImport ? "dynamic" : "require";
        const only = node.arguments.length === 1 ? node.arguments[0] : null;
        if (only && ts.isStringLiteralLike(only)) {
          add(node, kind, only.text);
        } else {
          add(node, kind, null, false);
        }
      }
    }
    ts.forEachChild(node, visit);
  }
  visit(sourceFile);
  return references;
}
export function collectImportMetaMembers(sourceFile) {
  const members = [];
  function visit(node) {
    if (
      ts.isMetaProperty(node) &&
      node.keywordToken === ts.SyntaxKind.ImportKeyword &&
      node.name.text === "meta"
    ) {
      let member = null;
      if (ts.isPropertyAccessExpression(node.parent) && node.parent.expression === node) {
        member = node.parent.name.text;
      } else if (
        ts.isElementAccessExpression(node.parent) &&
        node.parent.expression === node &&
        ts.isStringLiteralLike(node.parent.argumentExpression)
      ) {
        member = node.parent.argumentExpression.text;
      }
      members.push({ member, line: nodeLine(sourceFile, node) });
    }
    ts.forEachChild(node, visit);
  }
  visit(sourceFile);
  return members;
}
const HTML_INJECTION_SINKS = new Set(["innerHTML", "outerHTML", "insertAdjacentHTML", "srcdoc", "dangerouslySetInnerHTML"]);
const HTML_WRITE_PROPERTIES = new Set(["innerHTML", "outerHTML", "srcdoc"]);
const ASSIGNMENT_OPERATORS = new Set([
  ts.SyntaxKind.EqualsToken,
  ts.SyntaxKind.PlusEqualsToken,
  ts.SyntaxKind.MinusEqualsToken,
  ts.SyntaxKind.AsteriskEqualsToken,
  ts.SyntaxKind.SlashEqualsToken,
  ts.SyntaxKind.PercentEqualsToken,
  ts.SyntaxKind.AsteriskAsteriskEqualsToken,
  ts.SyntaxKind.LessThanLessThanEqualsToken,
  ts.SyntaxKind.GreaterThanGreaterThanEqualsToken,
  ts.SyntaxKind.GreaterThanGreaterThanGreaterThanEqualsToken,
  ts.SyntaxKind.AmpersandEqualsToken,
  ts.SyntaxKind.BarEqualsToken,
  ts.SyntaxKind.CaretEqualsToken,
  ts.SyntaxKind.BarBarEqualsToken,
  ts.SyntaxKind.AmpersandAmpersandEqualsToken,
  ts.SyntaxKind.QuestionQuestionEqualsToken,
]);
function propertyNameText(node) {
  if (!node) return null;
  if (ts.isIdentifier(node) || ts.isStringLiteralLike(node)) return node.text;
  return null;
}
function isWriteTarget(node) {
  const parent = node.parent;
  if (ts.isBinaryExpression(parent) && parent.left === node && ASSIGNMENT_OPERATORS.has(parent.operatorToken.kind)) return true;
  if (ts.isPrefixUnaryExpression(parent) && parent.operand === node && [ts.SyntaxKind.PlusPlusToken, ts.SyntaxKind.MinusMinusToken].includes(parent.operator)) return true;
  if (ts.isPostfixUnaryExpression(parent) && parent.operand === node && [ts.SyntaxKind.PlusPlusToken, ts.SyntaxKind.MinusMinusToken].includes(parent.operator)) return true;
  return false;
}
function isCallTarget(node) {
  return ts.isCallExpression(node.parent) && node.parent.expression === node;
}
export function collectHtmlInjectionSites(sourceFile) {
  const sites = [];
  const add = (node, sink) => sites.push({ sink, line: nodeLine(sourceFile, node) });
  function inspectAccess(node, name) {
    if (HTML_WRITE_PROPERTIES.has(name) && isWriteTarget(node)) add(node, name);
    if (name === "insertAdjacentHTML" && isCallTarget(node)) add(node, name);
    if (name === "write" && isCallTarget(node) && ts.isIdentifier(node.expression) && node.expression.text === "document") add(node, "document.write");
  }
  function visit(node) {
    if (ts.isPropertyAccessExpression(node)) {
      inspectAccess(node, node.name.text);
    } else if (ts.isElementAccessExpression(node) && ts.isStringLiteralLike(node.argumentExpression)) {
      inspectAccess(node, node.argumentExpression.text);
    } else if (ts.isPropertyAssignment(node)) {
      const name = propertyNameText(node.name);
      if (name && HTML_INJECTION_SINKS.has(name)) add(node, name);
    } else if (ts.isJsxAttribute(node)) {
      const name = propertyNameText(node.name);
      if (name && HTML_INJECTION_SINKS.has(name)) add(node, name);
    }
    ts.forEachChild(node, visit);
  }
  visit(sourceFile);
  return sites;
}
const REVIEWED_IMPORT_META_MEMBERS = new Set([]);
const PHASE3_PACKAGE_ALLOWLIST = new Set(["react", "react-dom", "@tauri-apps/api", "@tauri-apps/plugin-dialog", "@tauri-apps/plugin-opener"]);
const DYNAMIC_CHUNK_ALLOWLIST = new Set([]);
export const TAURI_TRANSITIONAL_WAIVERS = [
  { file: "src/api.ts", specifiers: ["@tauri-apps/api/core"], owner: "shared-api-transport", removeByPhase: 5, removal: "Move invoke behind the reviewed typed API transport boundary." },
  { file: "src/app/App.tsx", specifiers: ["@tauri-apps/api/event", "@tauri-apps/plugin-dialog", "@tauri-apps/plugin-opener"], owner: "app-shell-adapters", removeByPhase: 5, removal: "Move shell event/dialog/opener calls behind typed clients and adapters." },
  { file: "src/modules/accounts/AccountDialog.tsx", specifiers: ["@tauri-apps/plugin-opener"], owner: "accounts-feature", removeByPhase: 5, removal: "Route link opening through the reviewed shared opener contract." },
  { file: "src/modules/saves/SaveManagerView.tsx", specifiers: ["@tauri-apps/plugin-dialog"], owner: "saves-feature", removeByPhase: 5, removal: "Route save dialogs through a typed shared dialog contract." },
  { file: "src/modules/versions/downloads/LoaderInstallPanel.tsx", specifiers: ["@tauri-apps/api/event"], owner: "versions-downloads-feature", removeByPhase: 5, removal: "Consume progress through the typed shared event client." },
  { file: "src/modules/versions/downloads/ModDiscoveryPanel.tsx", specifiers: ["@tauri-apps/api/event"], owner: "versions-downloads-feature", removeByPhase: 5, removal: "Consume progress through the typed shared event client." },
  { file: "src/components/TitleBar.tsx", specifiers: ["@tauri-apps/api/window"], owner: "shared-window-chrome-adapter", removeByPhase: 5, removal: "Relocate window chrome to the reviewed host/shell adapter." },
];
const policyViolation = (rule, message) => ({ rule, message });
export function classifyTauriImport(phase, fileRelative, specifier) {
  if (specifier !== "@tauri-apps" && !specifier.startsWith("@tauri-apps/")) return null;
  const waiver = TAURI_TRANSITIONAL_WAIVERS.find((candidate) => candidate.file === fileRelative);
  if (!waiver) {
    return policyViolation("FE-TAURI-DIRECT-001", `direct Tauri import '${specifier}' is outside the exact transitional allowlist.`);
  }
  if (phase >= waiver.removeByPhase) {
    return policyViolation("FE-TAURI-DIRECT-001", `Tauri waiver for '${waiver.file}' expired at phase ${waiver.removeByPhase}: ${waiver.removal}`);
  }
  if (!waiver.specifiers.includes(specifier)) {
    return policyViolation("FE-TAURI-DIRECT-001", `specifier '${specifier}' is not one of the exact reviewed specifiers for ${waiver.file}.`);
  }
  return null;
}
export function frontendFeature(relativePath) {
  const match = /^src\/modules\/([^/]+)(?:\/|$)/.exec(relativePath);
  return match?.[1] ?? null;
}
export function isFeatureEntry(relativePath) {
  return /^src\/modules\/[^/]+\/index\.(?:ts|tsx|js|jsx)$/.test(relativePath) ||
    /^src\/modules\/[^/]+\.(?:ts|tsx|js|jsx)$/.test(relativePath);
}
export function isSharedFrontendSource(relativePath) {
  return (
    relativePath === "src/api.ts" ||
    relativePath === "src/types.ts" ||
    relativePath === "src/utils.ts" ||
    relativePath.startsWith("src/components/") ||
    relativePath.startsWith("src/core/") ||
    relativePath.startsWith("src/shared/")
  );
}
export function isApprovedSharedTarget(relativePath) {
  return (
    relativePath === "src/api.ts" ||
    relativePath === "src/types.ts" ||
    relativePath === "src/utils.ts" ||
    relativePath.startsWith("src/components/") ||
    relativePath.startsWith("src/core/") ||
    relativePath.startsWith("src/shared/")
  );
}
export function evaluateFrontendBoundary(phase, sourceRelative, targetRelative, referenceKind = "static") {
  const sourceFeature = frontendFeature(sourceRelative);
  const targetFeature = frontendFeature(targetRelative);
  const targetIsModule = targetRelative.startsWith("src/modules/");
  const sourceIsComposition = sourceRelative === COMPOSITION_ROOT;
  if (isSharedFrontendSource(sourceRelative) && (targetRelative.startsWith("src/app/") || targetIsModule)) {
    return policyViolation("FE-CORE-SHARED-IMPORT-001", `core/shared code must not import app or feature module '${targetRelative}'.`);
  }
  if (sourceFeature && targetRelative.startsWith("src/app/")) {
    return policyViolation("FE-FEATURE-APP-IMPORT-001", `feature '${sourceFeature}' must not import app target '${targetRelative}'.`);
  }
  if (sourceFeature && targetFeature && sourceFeature !== targetFeature) {
    return policyViolation("FE-FEATURE-CROSS-IMPORT-001", `feature '${sourceFeature}' must not import feature '${targetFeature}' (including public entries).`);
  }
  if (sourceFeature && !targetFeature && !isApprovedSharedTarget(targetRelative)) {
    return policyViolation("FE-FEATURE-SHARED-IMPORT-001", `feature '${sourceFeature}' may import only its own subtree plus reviewed core/shared APIs, not '${targetRelative}'.`);
  }
  if (targetIsModule && !sourceFeature && !sourceIsComposition) {
    if (sourceRelative.startsWith("src/app/")) {
      return policyViolation("FE-APP-MODULE-IMPORT-001", `app shell/layout files must not import feature module '${targetRelative}'; only ${COMPOSITION_ROOT} composes modules.`);
    }
    return policyViolation("FE-COMPOSITION-ONLY-001", `only ${COMPOSITION_ROOT} may import concrete feature modules; '${sourceRelative}' imported '${targetRelative}'.`);
  }
  if (targetIsModule && sourceIsComposition && !isFeatureEntry(targetRelative)) {
    return policyViolation("FE-COMPOSITION-ENTRY-001", `composition root may import only a feature public entry point, not private path '${targetRelative}'.`);
  }
  if (phase >= 3 && referenceKind === "dynamic") {
    const insideOwnFeature = Boolean(sourceFeature && targetFeature === sourceFeature);
    if (!insideOwnFeature && !DYNAMIC_CHUNK_ALLOWLIST.has(targetRelative)) {
      return policyViolation("FE-DYNAMIC-IMPORT-003", `dynamic import target '${targetRelative}' must be inside the importing feature subtree or an exact reviewed local chunk.`);
    }
  }
  return null;
}
function calleeName(expression) {
  if (ts.isIdentifier(expression)) return expression.text;
  if (ts.isPropertyAccessExpression(expression)) return expression.name.text;
  if (ts.isElementAccessExpression(expression) && ts.isStringLiteralLike(expression.argumentExpression)) {
    return expression.argumentExpression.text;
  }
  return null;
}
export function collectUnsafeCodeSites(sourceFile) {
  const sites = [];
  const add = (node, kind) => sites.push({ kind, line: nodeLine(sourceFile, node) });
  function isPropertyName(node) {
    return (
      (ts.isPropertyAccessExpression(node.parent) && node.parent.name === node) ||
      (ts.isPropertyAssignment(node.parent) && node.parent.name === node) ||
      (ts.isVariableDeclaration(node.parent) && node.parent.name === node)
    );
  }
  function visit(node) {
    if (ts.isIdentifier(node) && node.text === "eval" && !isPropertyName(node)) {
      if (!(ts.isCallExpression(node.parent) && node.parent.expression === node)) {
        add(node, "eval-reference");
      }
    }
    if (ts.isCallExpression(node) || ts.isNewExpression(node)) {
      const name = calleeName(node.expression);
      if (name === "eval") add(node, "eval");
      if (name === "Function") add(node, "Function-constructor");
    }
    ts.forEachChild(node, visit);
  }
  visit(sourceFile);
  return sites;
}
function reportParseDiagnostics(report, fileRelative, sourceFile) {
  for (const diagnostic of sourceFile.parseDiagnostics ?? []) {
    const start = diagnostic.start ?? 0;
    const line = sourceFile.getLineAndCharacterOfPosition(start).line + 1;
    report("FE-PARSE-001", fileRelative, line, `TypeScript parse error: ${ts.flattenDiagnosticMessageText(diagnostic.messageText, " ")}`);
  }
}
export function validateFrontendModuleReference({
  phase,
  sourceRelative,
  sourceAbsolute,
  compilerOptions,
  frontendRoot,
  reference,
  report,
}) {
  if (!reference.valid || !reference.specifier) {
    const rule = reference.kind === "dynamic" ? "FE-DYNAMIC-IMPORT-001" : "FE-REQUIRE-001";
    report(
      rule,
      sourceRelative,
      reference.line,
      `${reference.kind} requires exactly one string-literal module specifier; computed loading is forbidden.`,
    );
    return;
  }
  const specifier = reference.specifier;
  if (!specifier) return;
  if (specifier.includes("?") || specifier.includes("#")) {
    report("FE-IMPORT-SPECIFIER-001", sourceRelative, reference.line, `module specifier '${specifier}' uses query/hash resolution that is not reviewed.`);
    return;
  }
  const tauriViolation = classifyTauriImport(phase, sourceRelative, specifier);
  if (tauriViolation) report(tauriViolation.rule, sourceRelative, reference.line, tauriViolation.message);
  if (reference.kind === "dynamic" && !isRelativeSpecifier(specifier)) {
    report("FE-DYNAMIC-IMPORT-002", sourceRelative, reference.line, `dynamic import '${specifier}' must be a literal relative local chunk; package/alias/remote contribution loaders are forbidden.`);
    return;
  }
  if (isAbsoluteOrRemoteSpecifier(specifier)) {
    report("FE-IMPORT-SPECIFIER-001", sourceRelative, reference.line, `absolute, remote, protocol, package-import alias, or unsupported module specifier '${specifier}' is forbidden.`);
    return;
  }
  const resolution = resolveFrontendModule(specifier, sourceAbsolute, compilerOptions, frontendRoot);
  if (resolution.kind === "unresolved" || resolution.kind === "unresolved-alias") {
    report(
      "FE-IMPORT-RESOLVE-001",
      sourceRelative,
      reference.line,
      resolution.kind === "unresolved-alias"
        ? `tsconfig alias/module '${specifier}' does not resolve to a reviewed file.`
        : `relative module '${specifier}' does not resolve.`,
    );
    return;
  }
  if (resolution.kind === "escape") {
    report("FE-IMPORT-ESCAPE-001", sourceRelative, reference.line, `module '${specifier}' resolves outside the real frontend src root.`);
    return;
  }
  if (resolution.kind === "external") {
    const packageRoot = policyPackageRoot(specifier, resolution.packageRoot);
    if (
      packageRoot === "@tauri-apps" ||
      packageRoot.startsWith("@tauri-apps/")
    ) {
      if (!specifier.startsWith("@tauri-apps/") && specifier !== "@tauri-apps") {
        report("FE-TAURI-DIRECT-001", sourceRelative, reference.line, `Tauri package '${packageRoot}' is reached through alias '${specifier}'; only exact reviewed direct imports are allowed.`);
      }
    }
    if (phase >= 3 && !PHASE3_PACKAGE_ALLOWLIST.has(packageRoot)) {
      report("FE-EXTERNAL-PACKAGE-001", sourceRelative, reference.line, `external package '${packageRoot}' is not in the reviewed phase-3 package allowlist.`);
    }
    return;
  }
  const targetRelative = relFromRepo(resolution.resolvedFile);
  const boundary = evaluateFrontendBoundary(phase, sourceRelative, targetRelative, reference.kind);
  if (boundary) report(boundary.rule, sourceRelative, reference.line, boundary.message);
}
function checkFrontend(phase, report) {
  let compilerOptions;
  try {
    compilerOptions = loadTypeScriptConfig(REPO_ROOT);
  } catch (error) {
    report("FE-TSCONFIG-001", "tsconfig.json", 1, `cannot load TypeScript module resolution options: ${error.message}`);
    return;
  }
  const sourceFiles = walkFilesFailClosed(FRONTEND_ROOT, new Set([".ts", ".tsx"]), report);
  const parsedFiles = new Map();
  for (const sourceAbsolute of sourceFiles) {
    const sourceRelative = relFromRepo(sourceAbsolute);
    let sourceFile;
    try {
      sourceFile = parseTypeScriptSource(sourceAbsolute, readText(sourceAbsolute));
    } catch (error) {
      report("FE-PARSE-001", sourceRelative, 1, `cannot parse TypeScript source: ${error.message}`);
      continue;
    }
    parsedFiles.set(sourceAbsolute, sourceFile);
    reportParseDiagnostics(report, sourceRelative, sourceFile);
    for (const reference of collectFrontendModuleReferences(sourceFile)) {
      validateFrontendModuleReference({
        phase,
        sourceRelative,
        sourceAbsolute,
        compilerOptions,
        frontendRoot: FRONTEND_ROOT,
        reference,
        report,
      });
    }
    for (const meta of collectImportMetaMembers(sourceFile)) {
      if (!meta.member || !REVIEWED_IMPORT_META_MEMBERS.has(meta.member)) {
        report("FE-IMPORT-META-001", sourceRelative, meta.line, `unreviewed import.meta member '${meta.member ?? "<whole>"}'; import.meta.glob, globEager, and arbitrary contribution loading are forbidden.`);
      }
    }
    for (const site of collectHtmlInjectionSites(sourceFile)) {
      report("FE-HTML-INJECTION-001", sourceRelative, site.line, `HTML injection sink '${site.sink}' is forbidden.`);
    }
    for (const site of collectUnsafeCodeSites(sourceFile)) {
      const rule = site.kind === "Function-constructor"
        ? "FE-FUNCTION-CONSTRUCTOR-001"
        : "FE-EVAL-001";
      report(rule, sourceRelative, site.line, `dynamic code evaluation via '${site.kind}' is forbidden.`);
    }
  }
  if (phase >= 3) checkPhase3Contributions(parsedFiles, compilerOptions, report);
}
function hasExportModifier(node) {
  return Boolean(ts.canHaveModifiers(node) && ts.getModifiers(node)?.some((modifier) => modifier.kind === ts.SyntaxKind.ExportKeyword));
}
function objectHasProperties(node, names) {
  if (!ts.isObjectLiteralExpression(node)) return false;
  const present = new Set();
  for (const property of node.properties) {
    if (ts.isPropertyAssignment(property)) {
      const name = propertyNameText(property.name);
      if (name) present.add(name);
    }
  }
  return names.every((name) => present.has(name));
}
function isFeatureDescriptorInitializer(declaration) {
  const typeText = declaration.type?.getText() ?? "";
  if (typeText.includes("FeatureDescriptor")) return true;
  return objectHasProperties(declaration.initializer, ["id", "featureVersion", "requires", "contributions"]);
}
export function getExportedFeatureDescriptors(sourceFile) {
  const declared = new Set();
  const names = new Set();
  let defaultIsDescriptor = false;
  for (const statement of sourceFile.statements) {
    if (!ts.isVariableStatement(statement)) continue;
    for (const declaration of statement.declarationList.declarations) {
      if (!ts.isIdentifier(declaration.name) || !declaration.initializer || !isFeatureDescriptorInitializer(declaration)) continue;
      declared.add(declaration.name.text);
      if (hasExportModifier(statement)) names.add(declaration.name.text);
    }
  }
  for (const statement of sourceFile.statements) {
    if (ts.isExportDeclaration(statement) && !statement.moduleSpecifier && statement.exportClause && ts.isNamedExports(statement.exportClause)) {
      for (const element of statement.exportClause.elements) {
        const localName = (element.propertyName ?? element.name).text;
        if (!declared.has(localName)) continue;
        if (element.name.text === "default") defaultIsDescriptor = true;
        else names.add(element.name.text);
      }
    } else if (ts.isExportAssignment(statement)) {
      if (ts.isIdentifier(statement.expression) && declared.has(statement.expression.text)) defaultIsDescriptor = true;
      else if (isFeatureDescriptorInitializer({ initializer: statement.expression, type: undefined })) defaultIsDescriptor = true;
    }
  }
  return { names: new Set(defaultIsDescriptor ? [...names, "default"] : [...names]), hasDescriptor: names.size > 0 || defaultIsDescriptor, hasDefault: defaultIsDescriptor };
}
function findFeatureManifests(report) {
  const modulesRoot = path.join(FRONTEND_ROOT, "modules");
  const manifests = new Map();
  if (!fs.existsSync(modulesRoot)) {
    report("FE-PHASE-TARGET-001", "src/modules", 1, `phase 3 requires the feature modules directory.`);
    return manifests;
  }
  for (const entry of fs.readdirSync(modulesRoot, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    if (entry.isSymbolicLink() || !entry.isDirectory()) continue;
    const candidates = ["index.ts", "index.tsx"]
      .map((name) => path.join(modulesRoot, entry.name, name))
      .filter((candidate) => fs.existsSync(candidate));
    if (candidates.length !== 1) {
      report(
        "FE-FEATURE-MANIFEST-001",
        `src/modules/${entry.name}/index.tsx`,
        1,
        candidates.length === 0 ? "feature manifest is missing." : "feature has both index.ts and index.tsx; exactly one is required.",
      );
      continue;
    }
    manifests.set(entry.name, realpathOrNull(candidates[0]) ?? candidates[0]);
  }
  if (manifests.size === 0) {
    report("FE-PHASE-TARGET-001", "src/modules", 1, "phase 3 requires at least one reviewed feature manifest.");
  }
  return manifests;
}
function importBindingsFor(sourceFile, parserOptions) {
  const bindings = new Map();
  for (const statement of sourceFile.statements) {
    if (!ts.isImportDeclaration(statement) || !ts.isStringLiteralLike(statement.moduleSpecifier)) continue;
    const clause = statement.importClause;
    if (!clause || clause.isTypeOnly) continue;
    const resolution = resolveFrontendModule(
      statement.moduleSpecifier.text,
      sourceFile.fileName,
      parserOptions.compilerOptions,
      parserOptions.frontendRoot,
    );
    if (resolution.kind !== "local") continue;
    const target = realpathOrNull(resolution.resolvedFile) ?? resolution.resolvedFile;
    if (clause.name) {
      bindings.set(clause.name.text, { target, importedName: "default", line: nodeLine(sourceFile, clause.name) });
    }
    if (clause.namedBindings && ts.isNamedImports(clause.namedBindings)) {
      for (const element of clause.namedBindings.elements) {
        if (element.isTypeOnly) continue;
        const importedName = (element.propertyName ?? element.name).text;
        bindings.set(element.name.text, { target, importedName, line: nodeLine(sourceFile, element.name) });
      }
    }
  }
  return bindings;
}
function findRegistryCalls(sourceFile, builderNames) {
  const calls = [];
  function visit(node) {
    if (
      ts.isCallExpression(node) &&
      ts.isIdentifier(node.expression) &&
      builderNames.has(node.expression.text)
    ) {
      calls.push(node);
    }
    ts.forEachChild(node, visit);
  }
  visit(sourceFile);
  return calls;
}
export function evaluatePhase3Orchestration({ manifests, descriptorByManifest, compositionFile, bindings, builderNames, compositionRelative = COMPOSITION_ROOT }) {
  const diagnostics = [];
  const add = (rule, file, line, message) => diagnostics.push({ rule, file, line, message });
  const label = (value) => value && (path.isAbsolute(value) ? relFromRepo(value) : toPosix(value));
  for (const [featureName, manifestPath] of manifests) {
    if (!descriptorByManifest.get(manifestPath)?.hasDescriptor) {
      add("FE-FEATURE-DESCRIPTOR-001", label(manifestPath), 1, `feature '${featureName}' must export a FeatureDescriptor value.`);
    }
  }
  if (!compositionFile) {
    add("FE-PHASE-TARGET-001", compositionRelative, 1, "phase 3 requires the frontend composition root.");
    return diagnostics;
  }
  const importedManifests = new Set();
  for (const binding of bindings.values()) if (descriptorByManifest.has(binding.target)) importedManifests.add(binding.target);
  for (const [featureName, manifestPath] of manifests) {
    if (!importedManifests.has(manifestPath)) add("FE-COMPOSITION-MANIFEST-001", compositionRelative, 1, `composition root does not value-import feature '${featureName}' manifest '${label(manifestPath)}'.`);
  }
  if (builderNames.size === 0) {
    add("FE-COMPOSITION-REGISTRY-001", compositionRelative, 1, "composition root must import buildContributionRegistry from the reviewed contract.");
    return diagnostics;
  }
  const calls = findRegistryCalls(compositionFile, builderNames);
  if (calls.length !== 1) {
    add("FE-COMPOSITION-REGISTRY-001", compositionRelative, 1, `expected exactly one explicit buildContributionRegistry(...) call, found ${calls.length}.`);
    return diagnostics;
  }
  const call = calls[0];
  if (call.arguments.length !== 1) {
    add("FE-COMPOSITION-REGISTRY-001", compositionRelative, nodeLine(compositionFile, call), `buildContributionRegistry must receive exactly one argument, found ${call.arguments.length}.`);
    return diagnostics;
  }
  const argument = call.arguments[0];
  if (!ts.isArrayLiteralExpression(argument)) {
    add("FE-COMPOSITION-REGISTRY-001", compositionRelative, nodeLine(compositionFile, call), "buildContributionRegistry must receive exactly one literal array argument.");
    return diagnostics;
  }
  const usedManifests = new Set();
  const usedIdentifiers = new Set();
  for (const element of argument.elements) {
    if (!ts.isIdentifier(element)) {
      add("FE-COMPOSITION-FEATURE-001", compositionRelative, nodeLine(compositionFile, element), "registry feature list may contain only explicit imported descriptor identifiers; spreads/computed entries are forbidden.");
      continue;
    }
    if (usedIdentifiers.has(element.text)) {
      add("FE-COMPOSITION-FEATURE-001", compositionRelative, nodeLine(compositionFile, element), `descriptor identifier '${element.text}' appears more than once in the registry array.`);
      continue;
    }
    usedIdentifiers.add(element.text);
    const binding = bindings.get(element.text);
    if (!binding || !descriptorByManifest.has(binding.target)) {
      add("FE-COMPOSITION-FEATURE-001", compositionRelative, nodeLine(compositionFile, element), `identifier '${element.text}' is not a value import from a reviewed feature manifest.`);
      continue;
    }
    if (!descriptorByManifest.get(binding.target).names.has(binding.importedName)) {
      add("FE-COMPOSITION-FEATURE-001", compositionRelative, nodeLine(compositionFile, element), `identifier '${element.text}' imports '${binding.importedName}', which the manifest does not export as a FeatureDescriptor.`);
      continue;
    }
    if (usedManifests.has(binding.target)) {
      add("FE-COMPOSITION-FEATURE-001", compositionRelative, nodeLine(compositionFile, element), `feature manifest '${label(binding.target)}' appears more than once in the registry array.`);
      continue;
    }
    usedManifests.add(binding.target);
  }
  for (const [featureName, manifestPath] of manifests) {
    if (!usedManifests.has(manifestPath)) add("FE-COMPOSITION-FEATURE-001", compositionRelative, 1, `explicit registry array does not include descriptor for feature '${featureName}'.`);
  }
  return diagnostics;
}
function checkPhase3Contributions(parsedFiles, compilerOptions, report) {
  const contractPath = realpathOrNull(path.join(REPO_ROOT, CONTRIBUTIONS_CONTRACT));
  if (!contractPath || !fs.existsSync(contractPath)) {
    report("FE-PHASE-TARGET-001", CONTRIBUTIONS_CONTRACT, 1, "phase 3 requires the typed contribution registry contract.");
    return;
  }
  const manifests = findFeatureManifests(report);
  const descriptorByManifest = new Map();
  for (const [featureName, manifestPath] of manifests) {
    const sourceFile = parsedFiles.get(manifestPath) ?? parsedFiles.get(realpathOrNull(manifestPath));
    descriptorByManifest.set(manifestPath, sourceFile ? getExportedFeatureDescriptors(sourceFile) : { names: new Set(), hasDescriptor: false });
    if (!sourceFile) report("FE-FEATURE-DESCRIPTOR-001", relFromRepo(manifestPath), 1, `feature '${featureName}' manifest was not parsed.`);
  }
  const compositionPath = realpathOrNull(path.join(REPO_ROOT, COMPOSITION_ROOT));
  const compositionFile = compositionPath ? parsedFiles.get(compositionPath) ?? parsedFiles.get(path.join(REPO_ROOT, COMPOSITION_ROOT)) : null;
  const bindings = compositionFile ? importBindingsFor(compositionFile, { compilerOptions, frontendRoot: FRONTEND_ROOT }) : new Map();
  const builderNames = new Set();
  for (const [localName, binding] of bindings) {
    if (samePath(binding.target, contractPath) && binding.importedName === "buildContributionRegistry") builderNames.add(localName);
  }
  for (const diagnostic of evaluatePhase3Orchestration({ manifests, descriptorByManifest, compositionFile, bindings, builderNames, compositionRelative: COMPOSITION_ROOT })) {
    report(diagnostic.rule, diagnostic.file, diagnostic.line, diagnostic.message);
  }
}
// ---------------------------------------------------------------------------
// Rust masking, test-scope stripping, and module graph
// ---------------------------------------------------------------------------
function blankRustRanges(text, ranges) {
  const chars = text.split("");
  for (const range of ranges) {
    for (let index = range.start; index < range.end && index < chars.length; index += 1) {
      if (chars[index] !== "\n" && chars[index] !== "\r") chars[index] = " ";
    }
  }
  return chars.join("");
}
export function maskRustSource(source) {
  const chars = source.split("");
  const blank = (start, end) => {
    for (let index = start; index < end && index < chars.length; index += 1) {
      if (chars[index] !== "\n" && chars[index] !== "\r") chars[index] = " ";
    }
  };
  let index = 0;
  while (index < source.length) {
    if (source[index] === "/" && source[index + 1] === "/") {
      let end = index + 2;
      while (end < source.length && source[end] !== "\n") end += 1;
      blank(index, end);
      index = end;
      continue;
    }
    if (source[index] === "/" && source[index + 1] === "*") {
      let depth = 1;
      let end = index + 2;
      while (end < source.length && depth > 0) {
        if (source[end] === "/" && source[end + 1] === "*") {
          depth += 1;
          end += 2;
        } else if (source[end] === "*" && source[end + 1] === "/") {
          depth -= 1;
          end += 2;
        } else {
          end += 1;
        }
      }
      if (depth !== 0) throw new Error(`unterminated Rust block comment near line ${lineOf(source, index)}`);
      blank(index, end);
      index = end;
      continue;
    }
    const raw = /^(?:br|cr|r)(#*)"/.exec(source.slice(index));
    if (raw) {
      const closing = `"${raw[1]}`;
      const start = index;
      const contentStart = start + raw[0].length;
      const closeIndex = source.indexOf(closing, contentStart);
      if (closeIndex === -1) throw new Error(`unterminated Rust raw string near line ${lineOf(source, start)}`);
      const end = closeIndex + closing.length;
      blank(start, end);
      index = end;
      continue;
    }
    const prefixString = (source[index] === "b" || source[index] === "c") && source[index + 1] === '"';
    if (source[index] === '"' || prefixString) {
      const start = index;
      let cursor = (source[index] === '"' ? index : index + 1) + 1;
      let closed = false;
      while (cursor < source.length) {
        if (source[cursor] === "\\") {
          cursor += 2;
        } else if (source[cursor] === '"') {
          cursor += 1;
          closed = true;
          break;
        } else {
          cursor += 1;
        }
      }
      if (!closed) throw new Error(`unterminated Rust string near line ${lineOf(source, start)}`);
      blank(start, cursor);
      index = cursor;
      continue;
    }
    if (source[index] === "'") {
      const start = index;
      let cursor = index + 1;
      if (source[cursor] === "\\") {
        cursor += 2;
        while (cursor < source.length && source[cursor] !== "'" && source[cursor] !== "\n") cursor += 1;
        if (source[cursor] === "'") {
          blank(start, cursor + 1);
          index = cursor + 1;
          continue;
        }
      } else {
        cursor += source.codePointAt(cursor) > 0xffff ? 2 : 1;
        if (source[cursor] === "'") {
          blank(start, cursor + 1);
          index = cursor + 1;
          continue;
        }
      }
    }
    index += 1;
  }
  return chars.join("");
}
function findMatchingBrace(text, openIndex) {
  let depth = 0;
  for (let index = openIndex; index < text.length; index += 1) {
    if (text[index] === "{") depth += 1;
    else if (text[index] === "}") {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  return -1;
}
export function stripCfgTestModules(maskedSource) {
  const ranges = [];
  const pattern = /#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*(?:(?:#\s*\[[^\]]*\])\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+[A-Za-z_][A-Za-z0-9_]*\s*\{/g;
  let match;
  while ((match = pattern.exec(maskedSource)) !== null) {
    const openBrace = maskedSource.indexOf("{", match.index);
    const closeBrace = findMatchingBrace(maskedSource, openBrace);
    if (closeBrace === -1) throw new Error(`unterminated #[cfg(test)] module near line ${lineOf(maskedSource, match.index)}`);
    ranges.push({ start: match.index, end: closeBrace + 1 });
    pattern.lastIndex = closeBrace + 1;
  }
  return blankRustRanges(maskedSource, ranges);
}
export function prepareRustProductionSource(source) {
  return stripCfgTestModules(maskRustSource(source));
}
function rustModuleIdForFile(absolutePath) {
  const relative = toPosix(path.relative(RUST_SOURCE_ROOT, absolutePath));
  if (relative === "lib.rs") return "crate";
  if (relative === "main.rs") return "main";
  const parts = relative.split("/");
  const file = parts.pop();
  if (file !== "mod.rs") parts.push(file.replace(/\.rs$/, ""));
  return parts.join("::");
}
function rustModuleDirectory(absolutePath) {
  const relative = toPosix(path.relative(RUST_SOURCE_ROOT, absolutePath));
  if (relative === "lib.rs" || relative === "main.rs") return RUST_SOURCE_ROOT;
  const directory = path.dirname(absolutePath);
  const base = path.basename(absolutePath);
  return base === "mod.rs" ? directory : path.join(directory, base.replace(/\.rs$/, ""));
}
function moduleParts(moduleId) {
  return moduleId === "crate" || moduleId === "main" ? [] : moduleId.split("::");
}
function moduleId(parts) {
  return parts.length === 0 ? "crate" : parts.join("::");
}
const RUST_MODULE_ALLOWLIST = new Set(["crate", "main", "feature_host", "install", "loaders", "models", "mods", "provider_composition"]);
const RUST_MODULE_EDGE_ALLOWLIST = new Map(Object.entries({
  crate: ["feature_host", "install", "loaders", "models", "mods", "provider_composition"],
  main: [],
  feature_host: ["provider_composition"],
  install: ["models"],
  loaders: ["models"],
  models: [],
  mods: ["models"],
  provider_composition: [],
}));
export function reviewedHostRustModuleIds() {
  return [...RUST_MODULE_ALLOWLIST].sort();
}
function cargoExternalRoots(manifestText, packageName) {
  const roots = new Set(["std", "core", "alloc", "crate", "self", "super", "flint_lib"]);
  if (packageName) roots.add(packageName.replace(/-/g, "_"));
  let dependencySection = false;
  for (const rawLine of manifestText.split(/\r?\n/)) {
    const line = rawLine.replace(/\s+#.*$/, "").trim();
    if (line.startsWith("[") && line.endsWith("]")) {
      const section = line.slice(1, -1);
      dependencySection =
        section === "dependencies" ||
        section === "dev-dependencies" ||
        section === "build-dependencies" ||
        section.endsWith(".dependencies") ||
        section.endsWith(".dev-dependencies") ||
        section.endsWith(".build-dependencies");
      if (section.startsWith("dependencies.")) {
        const name = section.slice("dependencies.".length).split(".")[0];
        if (/^[A-Za-z0-9_-]+$/.test(name)) roots.add(name.replace(/-/g, "_"));
      }
      continue;
    }
    if (!dependencySection) continue;
    const key = /^([A-Za-z0-9_-]+)\s*=/.exec(line);
    if (key) roots.add(key[1].replace(/-/g, "_"));
  }
  return roots;
}
export function cargoDeclaredDependencyNames(manifestText) {
  const names = [];
  let dependencySection = false;
  for (const rawLine of manifestText.split(/\r?\n/)) {
    const line = rawLine.replace(/\s+#.*$/, "").trim();
    if (line.startsWith("[") && line.endsWith("]")) {
      const section = line.slice(1, -1);
      dependencySection =
        section === "dependencies" ||
        section === "dev-dependencies" ||
        section === "build-dependencies" ||
        section.endsWith(".dependencies") ||
        section.endsWith(".dev-dependencies") ||
        section.endsWith(".build-dependencies");
      if (section.startsWith("dependencies.")) {
        const name = section.slice("dependencies.".length).split(".")[0];
        if (/^[A-Za-z0-9_-]+$/.test(name)) names.push(name);
      }
      continue;
    }
    if (!dependencySection) continue;
    const key = /^([A-Za-z0-9_-]+)\s*=/.exec(line);
    if (key) names.push(key[1]);
  }
  return [...new Set(names)].sort();
}

function extractRustUseDeclarations(productionText) {
  const declarations = [];
  const pattern = /\buse\b([\s\S]*?);/g;
  let match;
  while ((match = pattern.exec(productionText)) !== null) {
    declarations.push({
      payload: match[1],
      index: match.index,
      line: lineOf(productionText, match.index),
    });
  }
  return declarations;
}
export function firstUseRoot(payload) {
  const match = /^\s*(?:::)?\s*(?:r#)?([A-Za-z_][A-Za-z0-9_]*)/.exec(payload);
  return match?.[1] ?? null;
}
export function moduleTargetsFromUse(payload, currentModuleId, moduleSet) {
  const targets = new Set();
  const tokens = [...payload.matchAll(/(?:r#)?([A-Za-z_][A-Za-z0-9_]*)/g)];
  for (let tokenIndex = 0; tokenIndex < tokens.length; tokenIndex += 1) {
    const token = tokens[tokenIndex];
    const tokenText = token[1];
    let candidate = null;
    if (moduleSet.has(tokenText)) candidate = tokenText;
    else if (moduleSet.has(`${currentModuleId}::${tokenText}`)) candidate = `${currentModuleId}::${tokenText}`;
    if (!candidate) continue;
    let suffix = payload.slice(token.index + token[0].length);
    while (true) {
      const extension = /^\s*::\s*(?:r#)?([A-Za-z_][A-Za-z0-9_]*)/.exec(suffix);
      if (!extension) break;
      const next = `${candidate}::${extension[1]}`;
      if (!moduleSet.has(next)) break;
      candidate = next;
      suffix = suffix.slice(extension[0].length);
    }
    // `crate` and `main` are graph roots, not importable target modules.
    if (candidate === "crate" || candidate === "main") continue;
    targets.add(candidate);
  }
  const root = firstUseRoot(payload);
  if (root === "super") {
    let parts = moduleParts(currentModuleId);
    let cursor = 0;
    while (new RegExp(`^\\s*(?:r#)?super\\b`).test(payload.slice(cursor))) {
      parts = parts.slice(0, -1);
      const step = /^\s*(?:r#)?super\b/.exec(payload.slice(cursor));
      cursor += step?.[0].length ?? 0;
    }
    const target = moduleId(parts);
    if (moduleSet.has(target)) targets.add(target);
  } else if (root === "self" && moduleSet.has(currentModuleId)) {
    targets.add(currentModuleId);
  }
  return targets;
}
function resolveExplicitRustPath(pathText, currentModuleId, moduleSet) {
  const segments = pathText.trim().split(/\s*::\s*/).map((segment) => segment.replace(/^r#/, ""));
  let base;
  let rest;
  if (segments[0] === "crate") {
    base = [];
    rest = segments.slice(1);
  } else if (segments[0] === "super") {
    base = moduleParts(currentModuleId);
    let cursor = 0;
    while (segments[cursor] === "super") {
      base = base.slice(0, -1);
      cursor += 1;
    }
    rest = segments.slice(cursor);
  } else if (segments[0] === "self") {
    base = moduleParts(currentModuleId);
    rest = segments.slice(1);
  } else {
    return null;
  }
  const full = base.concat(rest);
  for (let length = full.length; length > 0; length -= 1) {
    const candidate = moduleId(full.slice(0, length));
    if (moduleSet.has(candidate)) return candidate;
  }
  return null;
}
function buildRustGraph(report) {
  const files = walkFilesFailClosed(RUST_SOURCE_ROOT, new Set([".rs"]), report);
  const records = [];
  const moduleSet = new Set(["crate", "main"]);
  for (const absolutePath of files) {
    const relative = relFromRepo(absolutePath);
    let productionText;
    try {
      productionText = prepareRustProductionSource(readText(absolutePath));
    } catch (error) {
      report("RUST-LEX-001", relative, 1, `cannot statically mask Rust source: ${error.message}`);
      continue;
    }
    const currentModuleId = rustModuleIdForFile(absolutePath);
    moduleSet.add(currentModuleId);
    records.push({ absolutePath, relative, moduleId: currentModuleId, productionText });
  }
  for (const record of records) {
    if (!RUST_MODULE_ALLOWLIST.has(record.moduleId)) {
      report("RUST-MODULE-ALLOW-001", record.relative, 1, `module '${record.moduleId}' is not in the exact reviewed module allowlist.`);
    }
  }
  const declarationPattern = /^[ \t]*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;/gm;
  for (const record of records) {
    let match;
    while ((match = declarationPattern.exec(record.productionText)) !== null) {
      const name = match[1];
      const expected = record.moduleId === "crate" || record.moduleId === "main"
        ? name
        : `${record.moduleId}::${name}`;
      const directory = rustModuleDirectory(record.absolutePath);
      const fileCandidate = path.join(directory, `${name}.rs`);
      const modCandidate = path.join(directory, name, "mod.rs");
      const fileExists = fs.existsSync(fileCandidate) && fs.statSync(fileCandidate).isFile();
      const modExists = fs.existsSync(modCandidate) && fs.statSync(modCandidate).isFile();
      if (fileExists === modExists) {
        report(
          "RUST-MODULE-DECL-001",
          record.relative,
          lineOf(record.productionText, match.index),
          fileExists
            ? `module '${name}' is ambiguous between ${relFromRepo(fileCandidate)} and ${relFromRepo(modCandidate)}.`
            : `module '${name}' does not resolve to a declared Rust file.`,
        );
        continue;
      }
      const target = fileExists ? fileCandidate : modCandidate;
      if (!RUST_MODULE_ALLOWLIST.has(expected)) {
        report("RUST-MODULE-DECL-ALLOW-001", record.relative, lineOf(record.productionText, match.index), `module '${expected}' is not in the exact reviewed module allowlist.`);
      }
      if (rustModuleIdForFile(target) !== expected) {
        report("RUST-MODULE-DECL-001", record.relative, lineOf(record.productionText, match.index), `module '${name}' resolves to unexpected module id '${rustModuleIdForFile(target)}'.`);
      }
    }
  }
  const manifestText = fs.existsSync(RUST_MANIFEST) ? readText(RUST_MANIFEST) : "";
  const packageName = /\[package\][\s\S]*?^\s*name\s*=\s*"([^"]+)"/m.exec(manifestText)?.[1] ?? "flint";
  const externalRoots = cargoExternalRoots(manifestText, packageName);
  const edges = new Map();
  const addEdge = (sourceId, targetId, line) => {
    if (!sourceId || !targetId || sourceId === targetId) return;
    if (!edges.has(sourceId)) edges.set(sourceId, new Map());
    const targets = edges.get(sourceId);
    if (!targets.has(targetId)) targets.set(targetId, line);
  };
  for (const record of records) {
    for (const declaration of extractRustUseDeclarations(record.productionText)) {
      const root = firstUseRoot(declaration.payload);
      if (!root) continue;
      const local =
        root === "crate" || root === "super" || root === "self" ||
        moduleSet.has(root) || moduleSet.has(`${record.moduleId}::${root}`);
      if (local) {
        for (const target of moduleTargetsFromUse(declaration.payload, record.moduleId, moduleSet)) {
          addEdge(record.moduleId, target, declaration.line);
        }
      } else if (!externalRoots.has(root)) {
        report("RUST-MODULE-IMPORT-001", record.relative, declaration.line, `Rust use root '${root}' is neither a declared local module nor a reviewed Cargo dependency.`);
      }
    }
    const explicitPathPattern = /\b(?:crate|super|self)(?:\s*::\s*(?:r#)?[A-Za-z_][A-Za-z0-9_]*)+/g;
    let pathMatch;
    while ((pathMatch = explicitPathPattern.exec(record.productionText)) !== null) {
      const target = resolveExplicitRustPath(pathMatch[0], record.moduleId, moduleSet);
      if (target) addEdge(record.moduleId, target, lineOf(record.productionText, pathMatch.index));
    }
    if (record.moduleId === "crate" || record.moduleId === "main") {
      for (const rootModule of [...moduleSet].filter((id) => !id.includes("::") && id !== "crate" && id !== "main")) {
        const pattern = new RegExp(`\\b${rootModule}\\s*::`, "g");
        let match;
        while ((match = pattern.exec(record.productionText)) !== null) {
          addEdge(record.moduleId, rootModule, lineOf(record.productionText, match.index));
        }
      }
    }
  }
  for (const [sourceId, targets] of edges) {
    const allowed = new Set(RUST_MODULE_EDGE_ALLOWLIST.get(sourceId) ?? []);
    for (const [targetId, line] of targets) {
      if (!allowed.has(targetId)) {
        const sourceRecord = records.find((record) => record.moduleId === sourceId);
        report("RUST-MODULE-EDGE-001", sourceRecord?.relative ?? "src-tauri/src", line, `unreviewed Rust module edge '${sourceId}' -> '${targetId}'.`);
      }
    }
  }
  return { records, moduleSet, externalRoots };
}
// ---------------------------------------------------------------------------
// Host lifecycle command gate audit
// ---------------------------------------------------------------------------
const FEATURE_COMMAND_GATES = Object.freeze([
  { command: "load_settings", featureId: "flint.settings", constant: "FEATURE_SETTINGS" },
  { command: "save_settings", featureId: "flint.settings", constant: "FEATURE_SETTINGS" },
  { command: "list_accounts", featureId: "flint.accounts", constant: "FEATURE_ACCOUNTS" },
  { command: "begin_microsoft_login", featureId: "flint.accounts", constant: "FEATURE_ACCOUNTS" },
  { command: "poll_microsoft_login", featureId: "flint.accounts", constant: "FEATURE_ACCOUNTS" },
  { command: "select_account", featureId: "flint.accounts", constant: "FEATURE_ACCOUNTS" },
  { command: "remove_account", featureId: "flint.accounts", constant: "FEATURE_ACCOUNTS" },
  { command: "refresh_active_account", featureId: "flint.accounts", constant: "FEATURE_ACCOUNTS" },
  { command: "list_instances", featureId: "flint.vanilla", constant: "FEATURE_VANILLA" },
  { command: "scan_java", featureId: "flint.vanilla", constant: "FEATURE_VANILLA" },
  { command: "fetch_remote_versions", featureId: "flint.vanilla", constant: "FEATURE_VANILLA" },
  { command: "install_version", featureId: "flint.vanilla", constant: "FEATURE_VANILLA" },
  { command: "launch_instance", featureId: "flint.launch", constant: "FEATURE_LAUNCH" },
  { command: "list_loader_versions", featureId: "flint.loaders", constant: "FEATURE_LOADERS" },
  { command: "install_loader", featureId: "flint.loaders", constant: "FEATURE_LOADERS" },
  { command: "search_mods", featureId: "flint.mods", constant: "FEATURE_MODS" },
  { command: "list_mod_versions", featureId: "flint.mods", constant: "FEATURE_MODS" },
  { command: "resolve_mod_dependencies", featureId: "flint.mods", constant: "FEATURE_MODS" },
  { command: "install_mod", featureId: "flint.mods", constant: "FEATURE_MODS" },
  { command: "list_installed_mods", featureId: "flint.mods", constant: "FEATURE_MODS" },
  { command: "set_mod_enabled", featureId: "flint.mods", constant: "FEATURE_MODS" },
  { command: "delete_mod", featureId: "flint.mods", constant: "FEATURE_MODS" },
  { command: "list_saves", featureId: "flint.saves", constant: "FEATURE_SAVES" },
  { command: "backup_save", featureId: "flint.saves", constant: "FEATURE_SAVES" },
  { command: "export_save", featureId: "flint.saves", constant: "FEATURE_SAVES" },
  { command: "import_save", featureId: "flint.saves", constant: "FEATURE_SAVES" },
  { command: "rename_save", featureId: "flint.saves", constant: "FEATURE_SAVES" },
  { command: "delete_save", featureId: "flint.saves", constant: "FEATURE_SAVES" },
]);
const UNGATED_HOST_COMMANDS = Object.freeze(["launcher_info", "get_host_availability"]);

export function reviewedFeatureCommandGates() {
  return FEATURE_COMMAND_GATES.map(({ command, featureId, constant }) => ({ command, featureId, constant }));
}

export function auditHostCommandGates(sourceText, featureIdSourceText = sourceText) {
  const issues = [];
  const productionText = prepareRustProductionSource(sourceText);
  const expected = new Map();
  for (const gate of FEATURE_COMMAND_GATES) expected.set(gate.command, { kind: "gated", ...gate });
  for (const command of UNGATED_HOST_COMMANDS) expected.set(command, { kind: "ungated", command });

  const featureIdConstants = new Map();
  const featureIdPattern = /(?:pub(?:\([^)]*\))?\s+)?const\s+(FEATURE_[A-Z_]+)\s*:\s*&str\s*=\s*"([^"]+)";/g;
  let featureIdMatch;
  while ((featureIdMatch = featureIdPattern.exec(featureIdSourceText)) !== null) {
    const constant = featureIdMatch[1];
    const line = lineOf(featureIdSourceText, featureIdMatch.index);
    if (featureIdConstants.has(constant)) {
      issues.push({ line, message: `feature ID constant '${constant}' is declared more than once.` });
      continue;
    }
    featureIdConstants.set(constant, { value: featureIdMatch[2], line });
  }
  const checkedFeatureConstants = new Set();
  for (const gate of FEATURE_COMMAND_GATES) {
    if (checkedFeatureConstants.has(gate.constant)) continue;
    checkedFeatureConstants.add(gate.constant);
    const declared = featureIdConstants.get(gate.constant);
    if (!declared) {
      issues.push({ line: 1, message: `missing host feature ID constant '${gate.constant}'.` });
    } else if (declared.value !== gate.featureId) {
      issues.push({
        line: declared.line,
        message: `host feature ID constant '${gate.constant}' must equal '${gate.featureId}', found '${declared.value}'.`,
      });
    }
  }

  const actual = new Map();
  const commandPattern = /#\[tauri::command\]\s*(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(/g;
  let commandMatch;
  while ((commandMatch = commandPattern.exec(productionText)) !== null) {
    const command = commandMatch[1];
    const line = lineOf(productionText, commandMatch.index);
    if (actual.has(command)) {
      issues.push({ line, message: `duplicate #[tauri::command] function '${command}'.` });
      continue;
    }
    const bodyOpen = productionText.indexOf("{", commandMatch.index + commandMatch[0].length);
    if (bodyOpen === -1) {
      issues.push({ line, message: `command '${command}' has no function body.` });
      continue;
    }
    const bodyClose = findMatchingBrace(productionText, bodyOpen);
    if (bodyClose === -1) {
      issues.push({ line, message: `command '${command}' has an unterminated function body.` });
      continue;
    }
    actual.set(command, {
      line,
      signature: productionText.slice(commandMatch.index, bodyOpen),
      body: productionText.slice(bodyOpen, bodyClose + 1),
    });
  }

  for (const [command, details] of actual) {
    const expectation = expected.get(command);
    if (!expectation) {
      issues.push({ line: details.line, message: `unreviewed host command '${command}'.` });
      continue;
    }
    const gateCalls = (details.body.match(/\bensure_feature_ready\b/g) ?? []).length;
    const featureConstants = [...details.body.matchAll(/feature_host::(FEATURE_[A-Z_]+)/g)].map((match) => match[1]);
    if (expectation.kind === "ungated") {
      if (gateCalls !== 0 || featureConstants.length !== 0) {
        issues.push({ line: details.line, message: `ungated command '${command}' must not call a feature gate.` });
      }
      continue;
    }
    if (gateCalls !== 1) {
      issues.push({ line: details.line, message: `command '${command}' must contain exactly one feature gate call, found ${gateCalls}.` });
    }
    if (featureConstants.length !== 1 || featureConstants[0] !== expectation.constant) {
      issues.push({
        line: details.line,
        message: `command '${command}' must gate '${expectation.featureId}' via feature_host::${expectation.constant}.`,
      });
    }
    if (!/runtime\s*:\s*tauri::State\s*<\s*'_\s*,\s*feature_host::HostRuntime\s*>/m.test(details.signature)) {
      issues.push({ line: details.line, message: `command '${command}' must receive managed HostRuntime state.` });
    }
  }

  for (const [command, expectation] of expected) {
    if (!actual.has(command)) {
      const kind = expectation.kind === "ungated" ? "ungated command" : `command gated by '${expectation.featureId}'`;
      issues.push({ line: 1, message: `missing ${kind} '${command}'.` });
    }
  }

  const handlerMatch = /tauri::generate_handler!\s*\[\s*([\s\S]*?)\s*\]/.exec(productionText);
  if (!handlerMatch) {
    issues.push({ line: 1, message: "tauri::generate_handler! registration list is missing." });
    return issues;
  }
  const handlerLine = lineOf(productionText, handlerMatch.index);
  const handlerEntries = handlerMatch[1].split(",").map((entry) => entry.trim()).filter(Boolean);
  const seenHandlers = new Set();
  for (const handler of handlerEntries) {
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(handler)) {
      issues.push({ line: handlerLine, message: `generate_handler entry '${handler}' is not a compile-time command identifier.` });
    }
    if (seenHandlers.has(handler)) {
      issues.push({ line: handlerLine, message: `generate_handler registers '${handler}' more than once.` });
    }
    seenHandlers.add(handler);
    if (!actual.has(handler)) {
      issues.push({ line: handlerLine, message: `generate_handler entry '${handler}' does not match a #[tauri::command] function.` });
    }
  }
  for (const command of actual.keys()) {
    if (!seenHandlers.has(command)) {
      issues.push({ line: actual.get(command).line, message: `#[tauri::command] function '${command}' is missing from generate_handler!.` });
    }
  }
  if (handlerEntries.length !== actual.size) {
    issues.push({
      line: handlerLine,
      message: `generate_handler! must contain exactly one entry per command (${actual.size} commands, ${handlerEntries.length} entries).`,
    });
  }
  return issues;
}

function checkHostCommandGates(report) {
  const hostSource = path.join(RUST_SOURCE_ROOT, "lib.rs");
  if (!fs.existsSync(hostSource)) {
    report("RUST-COMMAND-GATE-001", "src-tauri/src/lib.rs", 1, "host command registration source is missing.");
    return;
  }
  const featureHostSource = path.join(RUST_SOURCE_ROOT, "feature_host.rs");
  const featureIdSource = fs.existsSync(featureHostSource) ? readText(featureHostSource) : "";
  for (const issue of auditHostCommandGates(readText(hostSource), featureIdSource)) {
    report("RUST-COMMAND-GATE-001", "src-tauri/src/lib.rs", issue.line, issue.message);
  }
}

// ---------------------------------------------------------------------------
// Phase 1/2 provider boundaries and single ModSource mapping adapter
// ---------------------------------------------------------------------------
const HOST_PRIVATE_MODULES = new Set([
  "auth", "feature_host", "install", "java", "launch", "provider_composition", "version",
]);
const FEATURE_PRIVATE_MODULE_NAMES = new Set([
  ...RUST_MODULE_ALLOWLIST,
  "paths",
  "net",
  "error",
  "settings",
  "saves",
  "auth",
  "java",
  "version",
  "launch",
]);
// Exact provider files/import roots. transitional entries are named and expire at phase 5.
const providerTarget = (file, role, localTargets, transitional = {}, external = ["std", "semver", "flint_core"]) => ({
  file, role, localTargets: new Set(localTargets), transitionalLocalTargets: new Map(Object.entries(transitional)), externalRoots: new Set(external),
});
const MOD_PROVIDER_TARGETS = [
  providerTarget(PROVIDER_API_SOURCE, "mod-provider-api", ["loaders", "mods"]),
  providerTarget("src-tauri/crates/flint-provider-api/src/mods.rs", "mod-provider-api", [], {}, ["std", "flint_core"]),
  providerTarget(
    "src-tauri/providers/flint-mod-provider-curseforge/src/lib.rs",
    "mod-provider-leaf",
    [],
    {},
    ["std", "serde_json", "flint_core", "flint_provider_api"],
  ),
  providerTarget(
    "src-tauri/providers/flint-mod-provider-modrinth/src/lib.rs",
    "mod-provider-leaf",
    [],
    {},
    ["std", "serde_json", "flint_core", "flint_provider_api"],
  ),
];
const MOD_PROVIDER_CRATE_SPECS = [
  {
    crateName: "flint-mod-provider-curseforge",
    manifest: "src-tauri/providers/flint-mod-provider-curseforge/Cargo.toml",
    source: "src-tauri/providers/flint-mod-provider-curseforge/src/lib.rs",
    dependencies: ["flint-core", "flint-provider-api", "serde_json"],
  },
  {
    crateName: "flint-mod-provider-modrinth",
    manifest: "src-tauri/providers/flint-mod-provider-modrinth/Cargo.toml",
    source: "src-tauri/providers/flint-mod-provider-modrinth/src/lib.rs",
    dependencies: ["flint-core", "flint-provider-api", "serde_json"],
  },
];
const MOD_PROVIDER_CRATE_MANIFESTS = MOD_PROVIDER_CRATE_SPECS.map((spec) => spec.manifest);
const MOD_PROVIDER_CRATE_REQUIRED_DEPENDENCIES = new Set([
  "flint-core",
  "flint-provider-api",
  "serde_json",
]);
const MOD_PROVIDER_CRATE_ALLOWED_DEPENDENCIES = new Set(MOD_PROVIDER_CRATE_REQUIRED_DEPENDENCIES);
function checkModProviderCrateManifests(report) {
  for (const relative of MOD_PROVIDER_CRATE_MANIFESTS) {
    const absolute = path.join(REPO_ROOT, relative);
    if (!fs.existsSync(absolute)) {
      report("RUST-PHASE-TARGET-001", relative, 1, "phase 3 requires the reviewed mod provider crate manifest.");
      continue;
    }
    const dependencies = cargoDeclaredDependencyNames(readText(absolute));
    for (const dependency of dependencies) {
      if (!MOD_PROVIDER_CRATE_ALLOWED_DEPENDENCIES.has(dependency)) {
        report("RUST-PROVIDER-DEP-ALLOW-001", relative, 1, `mod provider dependency '${dependency}' is not in the exact reviewed allowlist.`);
      }
    }
    for (const required of MOD_PROVIDER_CRATE_REQUIRED_DEPENDENCIES) {
      if (!dependencies.includes(required)) {
        report("RUST-PROVIDER-DEP-MISSING-001", relative, 1, `mod provider crate must declare dependency '${required}'.`);
      }
    }
  }
}

const LOADER_PROVIDER_CRATE_SPECS = [
  {
    crateName: "flint-loader-provider-fabric",
    manifest: "src-tauri/providers/flint-loader-provider-fabric/Cargo.toml",
    source: "src-tauri/providers/flint-loader-provider-fabric/src/lib.rs",
    dependencies: ["flint-core", "flint-provider-api", "serde_json"],
  },
  {
    crateName: "flint-loader-provider-quilt",
    manifest: "src-tauri/providers/flint-loader-provider-quilt/Cargo.toml",
    source: "src-tauri/providers/flint-loader-provider-quilt/src/lib.rs",
    dependencies: ["flint-core", "flint-provider-api", "serde_json"],
  },
  {
    crateName: "flint-loader-provider-forge",
    manifest: "src-tauri/providers/flint-loader-provider-forge/Cargo.toml",
    source: "src-tauri/providers/flint-loader-provider-forge/src/lib.rs",
    dependencies: ["flint-core", "flint-provider-api", "serde_json"],
  },
  {
    crateName: "flint-loader-provider-neoforge",
    manifest: "src-tauri/providers/flint-loader-provider-neoforge/Cargo.toml",
    source: "src-tauri/providers/flint-loader-provider-neoforge/src/lib.rs",
    dependencies: ["flint-core", "flint-provider-api"],
  },
  {
    crateName: "flint-loader-provider-liteloader",
    manifest: "src-tauri/providers/flint-loader-provider-liteloader/Cargo.toml",
    source: "src-tauri/providers/flint-loader-provider-liteloader/src/lib.rs",
    dependencies: ["flint-core", "flint-provider-api", "regex", "serde_json"],
  },
  {
    crateName: "flint-loader-provider-optifine",
    manifest: "src-tauri/providers/flint-loader-provider-optifine/Cargo.toml",
    source: "src-tauri/providers/flint-loader-provider-optifine/src/lib.rs",
    dependencies: ["flint-core", "flint-provider-api", "regex"],
  },
];
const FEATURE_CRATE_SPECS = [
  {
    crateName: "flint-feature-settings",
    manifest: "src-tauri/modules/flint-feature-settings/Cargo.toml",
    source: "src-tauri/modules/flint-feature-settings/src/lib.rs",
    dependencies: ["flint-core", "flint-feature-api", "serde", "serde_json"],
    localModules: [],
  },
  {
    crateName: "flint-feature-saves",
    manifest: "src-tauri/modules/flint-feature-saves/Cargo.toml",
    source: "src-tauri/modules/flint-feature-saves/src/lib.rs",
    dependencies: ["base64", "flate2", "flint-core", "flint-feature-api", "serde", "zip"],
    localModules: [],
  },
  {
    crateName: "flint-feature-accounts",
    manifest: "src-tauri/modules/flint-feature-accounts/Cargo.toml",
    source: "src-tauri/modules/flint-feature-accounts/src/lib.rs",
    dependencies: ["flint-core", "flint-feature-api", "flint-feature-settings", "reqwest", "serde", "serde_json"],
    localModules: [],
  },
  {
    crateName: "flint-feature-vanilla",
    manifest: "src-tauri/modules/flint-feature-vanilla/Cargo.toml",
    source: "src-tauri/modules/flint-feature-vanilla/src/lib.rs",
    dependencies: ["dirs", "flint-core", "flint-feature-api", "regex", "reqwest", "serde", "serde_json", "sha1"],
    localModules: ["install", "java", "version"],
  },
  {
    crateName: "flint-feature-launch",
    manifest: "src-tauri/modules/flint-feature-launch/Cargo.toml",
    source: "src-tauri/modules/flint-feature-launch/src/lib.rs",
    dependencies: ["flint-core", "flint-feature-accounts", "flint-feature-api", "flint-feature-settings", "flint-feature-vanilla", "serde", "serde_json", "zip"],
    localModules: [],
  },
  {
    crateName: "flint-feature-mods",
    manifest: "src-tauri/modules/flint-feature-mods/Cargo.toml",
    source: "src-tauri/modules/flint-feature-mods/src/lib.rs",
    dependencies: ["flint-core", "flint-feature-api", "flint-feature-settings", "flint-provider-api", "regex", "serde", "serde_json", "zip"],
    localModules: [],
  },
  {
    crateName: "flint-feature-loaders",
    manifest: "src-tauri/modules/flint-feature-loaders/Cargo.toml",
    source: "src-tauri/modules/flint-feature-loaders/src/lib.rs",
    dependencies: ["flint-core", "flint-feature-api", "flint-feature-vanilla", "flint-provider-api", "serde", "serde_json"],
    localModules: [],
  },
];
export function reviewedFeatureCrateSpecs() {
  return FEATURE_CRATE_SPECS.map((spec) => ({
    crateName: spec.crateName,
    manifest: spec.manifest,
    source: spec.source,
    dependencies: [...spec.dependencies].sort(),
    localModules: [...spec.localModules].sort(),
  }));
}
const FEATURE_LEAF_TARGETS = FEATURE_CRATE_SPECS.map((spec) =>
  providerTarget(
    spec.source,
    "feature-leaf",
    spec.localModules,
    {},
    ["std", ...spec.dependencies.map((dependency) => dependency.replace(/-/g, "_"))],
  ),
);
const FEATURE_TARGET_BY_SOURCE = new Map(FEATURE_LEAF_TARGETS.map((target) => [target.file, target]));
export function reviewedFeatureTargets() {
  return FEATURE_LEAF_TARGETS.map((target) => ({
    file: target.file,
    role: target.role,
    localTargets: [...target.localTargets].sort(),
    transitionalLocalTargets: [...target.transitionalLocalTargets.keys()].sort(),
    externalRoots: [...target.externalRoots].sort(),
  }));
}
export function evaluateFeatureDirectoryNames(names) {
  const expected = FEATURE_CRATE_SPECS.map((spec) => spec.crateName).sort();
  const actual = names.filter((name) => name.startsWith("flint-feature-")).sort();
  const issues = [];
  for (const name of expected) if (!actual.includes(name)) issues.push(`missing feature crate directory '${name}'.`);
  for (const name of actual) if (!expected.includes(name)) issues.push(`unexpected feature crate directory '${name}'.`);
  return issues;
}
export function evaluateFeatureCrateManifest(relative, manifestText) {
  const spec = FEATURE_CRATE_SPECS.find((candidate) => candidate.manifest === relative);
  if (!spec) return [{ line: 1, message: `unknown feature crate manifest target '${relative}'.` }];
  const issues = [];
  if (!new RegExp(`^\\s*name\\s*=\\s*"${spec.crateName}"\\s*$`, "m").test(manifestText)) {
    issues.push({ line: 1, message: `feature crate must be named exactly '${spec.crateName}'.` });
  }
  if (!/^\s*version\.workspace\s*=\s*true\s*$/m.test(manifestText)) {
    issues.push({ line: 1, message: "feature crate must inherit the workspace version." });
  }
  if (!/^\s*publish\s*=\s*false\s*$/m.test(manifestText)) {
    issues.push({ line: 1, message: "feature crate must declare publish = false." });
  }
  const declared = cargoDeclaredDependencyNames(manifestText);
  const expected = [...spec.dependencies].sort();
  if (JSON.stringify(declared) !== JSON.stringify(expected)) {
    issues.push({
      line: 1,
      message: `feature crate dependencies must be exactly ${JSON.stringify(expected)}, found ${JSON.stringify(declared)}.`,
    });
  }
  return issues;
}
function checkFeatureCrateManifests(report) {
  for (const spec of FEATURE_CRATE_SPECS) {
    const absolutePath = path.join(REPO_ROOT, spec.manifest);
    if (!fs.existsSync(absolutePath)) {
      report("RUST-PHASE-TARGET-001", spec.manifest, 1, "the exact feature crate manifest is missing.");
      continue;
    }
    for (const issue of evaluateFeatureCrateManifest(spec.manifest, readText(absolutePath))) {
      report("RUST-FEATURE-DEP-ALLOW-001", spec.manifest, issue.line, issue.message);
    }
  }
}
export function evaluateHostFeatureDependencyDeclarations(manifestText) {
  const issues = [];
  for (const spec of FEATURE_CRATE_SPECS) {
    const relativePath = spec.manifest
      .replace(/^src-tauri\//, "")
      .replace(/\/Cargo\.toml$/, "");
    const declaration = `${spec.crateName} = { path = "${relativePath}" }`;
    if (!manifestText.includes(declaration)) {
      issues.push({ line: 1, message: `host manifest must declare feature path dependency '${declaration}'.` });
    }
  }
  return issues;
}
export function evaluateVanillaInstallBoundary(featureInstallText, hostAdapterText) {
  const issues = [];
  if (!/pub\s+trait\s+InstallProgressSink\s*:\s*Send\s*\+\s*Sync\s*\{/.test(featureInstallText)) {
    issues.push({ line: 1, message: "vanilla install must define object-safe InstallProgressSink: Send + Sync." });
  }
  if (!/fn\s+report\s*\(\s*&self\s*,\s*progress\s*:\s*InstallProgress\s*\)\s*;/.test(featureInstallText)) {
    issues.push({ line: 1, message: "InstallProgressSink::report must accept InstallProgress by value." });
  }
  if (!/pub\s+fn\s+install_blocking\s*\(\s*game_dir\s*:\s*&str\s*,\s*version_id\s*:\s*&str\s*,\s*sink\s*:\s*&dyn\s+InstallProgressSink\s*,?\s*\)/.test(featureInstallText)) {
    issues.push({ line: 1, message: "vanilla install_blocking must take game_dir, version_id, and a sink." });
  }
  if (/\btauri\s*::/.test(featureInstallText) || /\b(?:AppHandle|Emitter)\b/.test(featureInstallText)) {
    issues.push({ line: 1, message: "vanilla install must not depend on Tauri handles or emitters." });
  }
  if (/flint:\/\/install-progress/.test(featureInstallText)) {
    issues.push({ line: 1, message: "the install progress event contract must remain in the host adapter." });
  }
  if (!/flint:\/\/install-progress/.test(hostAdapterText)) {
    issues.push({ line: 1, message: "host install adapter must preserve the flint://install-progress event." });
  }
  if (!/impl\s+InstallProgressSink\s+for\s+TauriInstallProgressSink/.test(hostAdapterText)) {
    issues.push({ line: 1, message: "host install adapter must implement InstallProgressSink." });
  }
  if (!/self\.app\.emit\s*\(\s*INSTALL_PROGRESS_EVENT\s*,\s*progress\s*\)/.test(hostAdapterText)) {
    issues.push({ line: 1, message: "host install adapter must emit InstallProgress through the preserved event." });
  }
  if (!/pub\s+fn\s+install_blocking\s*\(\s*app\s*:\s*&AppHandle\s*,\s*game_dir\s*:\s*&str\s*,\s*version_id\s*:\s*&str\s*,?\s*\)/.test(hostAdapterText)) {
    issues.push({ line: 1, message: "host install adapter must preserve the app/game_dir/version_id signature." });
  }
  if (!/feature_install\s*::\s*install_blocking\s*\(\s*game_dir\s*,\s*version_id\s*,\s*&sink\s*\)/.test(hostAdapterText)) {
    issues.push({ line: 1, message: "host install adapter must delegate to the feature install_blocking with its sink." });
  }
  return issues;
}
export function evaluateHostModLoaderAdapters(modsText, loadersText) {
  const issues = [];
  const modsRequired = [
    ["flint_feature_mods::install_mod_blocking", "host mods adapter must delegate installs to the feature crate"],
    ["impl ModInstallProgressSink for TauriModInstallProgressSink", "host mods adapter must implement the feature sink"],
    ["flint://mod-install-progress", "host mods adapter must preserve the mod progress event"],
    ["self.app.emit(MOD_INSTALL_PROGRESS_EVENT, progress)", "host mods adapter must emit through the event adapter"],
  ];
  for (const [needle, message] of modsRequired) if (!modsText.includes(needle)) issues.push({ line: 1, message });
  for (const forbidden of ["ProviderRegistry::new", "fn install_single_mod", "fn build_dependency_plan", "fn select_for_source", "fn parse_mod_jar_metadata"]) {
    if (modsText.includes(forbidden)) issues.push({ line: 1, message: `host mods adapter still contains legacy implementation '${forbidden}'.` });
  }
  const loadersRequired = [
    ["flint_feature_loaders::list_loader_versions_blocking", "host loaders adapter must delegate list operations"],
    ["flint_feature_loaders::install_loader_blocking", "host loaders adapter must delegate installs"],
    ["impl LoaderProgressSink for TauriLoaderProgressSink", "host loaders adapter must implement loader progress sink"],
    ["impl LoaderInstallProgressSink for TauriVanillaProgressSink", "host loaders adapter must implement vanilla progress sink"],
    ["flint://loader-install-progress", "host loaders adapter must preserve loader progress event"],
    ["flint://install-progress", "host loaders adapter must preserve vanilla progress event"],
    ["self.app.emit(LOADER_INSTALL_PROGRESS_EVENT, progress)", "host loaders adapter must emit loader progress"],
    ["self.app.emit(INSTALL_PROGRESS_EVENT, progress)", "host loaders adapter must emit vanilla progress"],
  ];
  for (const [needle, message] of loadersRequired) if (!loadersText.includes(needle)) issues.push({ line: 1, message });
  for (const forbidden of ["ProviderRegistry::new", "fn loader_binding", "fn select_loader_provider", "fn ensure_vanilla_base"]) {
    if (loadersText.includes(forbidden)) issues.push({ line: 1, message: `host loaders adapter still contains legacy implementation '${forbidden}'.` });
  }
  return issues;
}
export function evaluateFeatureLockEntries(lockText) {
  const issues = [];
  const host = /\[\[package\]\]\s*\r?\nname = "flint"\s*\r?\nversion = "([^"]+)"/.exec(lockText);
  if (!host) return [{ line: 1, message: "Cargo.lock must contain the host package 'flint' with its workspace version." }];
  const version = host[1];
  for (const spec of FEATURE_CRATE_SPECS) {
    const pattern = new RegExp(`\\[\\[package\\]\\]\\r?\\nname = "${spec.crateName}"\\r?\\nversion = "([^"]+)"`);
    const match = pattern.exec(lockText);
    if (!match) {
      issues.push({ line: 1, message: `Cargo.lock must contain local package '${spec.crateName}'.` });
    } else if (match[1] !== version) {
      issues.push({ line: 1, message: `Cargo.lock package '${spec.crateName}' version '${match[1]}' does not match host workspace version '${version}'.` });
    }
  }
  return issues;
}
export function evaluateLoaderProviderDirectoryNames(names) {
  const expected = LOADER_PROVIDER_CRATE_SPECS.map((spec) => spec.crateName).sort();
  const actual = names.filter((name) => name.startsWith("flint-loader-provider-")).sort();
  const issues = [];
  for (const name of expected) {
    if (!actual.includes(name)) issues.push(`missing loader provider crate directory '${name}'.`);
  }
  for (const name of actual) {
    if (!expected.includes(name)) issues.push(`unexpected loader provider crate directory '${name}'.`);
  }
  return issues;
}
export function reviewedLoaderProviderCrateSpecs() {
  return LOADER_PROVIDER_CRATE_SPECS.map((spec) => ({
    crateName: spec.crateName,
    manifest: spec.manifest,
    source: spec.source,
    dependencies: [...spec.dependencies].sort(),
  }));
}
const LOADER_PROVIDER_TARGET_BY_SOURCE = new Map();
const LOADER_API_FILES = [PROVIDER_API_SOURCE, "src-tauri/crates/flint-provider-api/src/loaders.rs"];
const LOADER_ADAPTER_FILES = ["src-tauri/src/loaders.rs"];
const LOADER_BINDING_SOURCE = "src-tauri/modules/flint-feature-loaders/src/lib.rs";
const LOADER_PROVIDER_TARGETS = [
  ...LOADER_API_FILES.map((file) =>
    providerTarget(
      file,
      "loader-provider-api",
      file === PROVIDER_API_SOURCE ? ["loaders", "mods"] : [],
      {},
      file === PROVIDER_API_SOURCE ? ["std", "semver", "flint_core"] : ["std", "flint_core"],
    ),
  ),
  ...LOADER_ADAPTER_FILES.map((file) =>
    providerTarget(
      file,
      "loader-provider-adapter",
      ["models"],
      {},
      ["std", "tauri", "flint_feature_loaders", "flint_provider_api"],
    ),
  ),
  ...LOADER_PROVIDER_CRATE_SPECS.map((spec) =>
    providerTarget(
      spec.source,
      "loader-provider-leaf",
      [],
      {},
      ["std", ...spec.dependencies.map((dependency) => dependency.replace(/-/g, "_"))],
    ),
  ),
];
for (const target of LOADER_PROVIDER_TARGETS) LOADER_PROVIDER_TARGET_BY_SOURCE.set(target.file, target);
export function evaluateLoaderProviderCrateManifest(relative, manifestText) {
  const spec = LOADER_PROVIDER_CRATE_SPECS.find((candidate) => candidate.manifest === relative);
  if (!spec) return [{ line: 1, message: `unknown loader provider crate manifest target '${relative}'.` }];
  const issues = [];
  if (!new RegExp(`^\\s*name\\s*=\\s*"${spec.crateName}"\\s*$`, "m").test(manifestText)) {
    issues.push({ line: 1, message: `loader provider crate must be named exactly '${spec.crateName}'.` });
  }
  if (!/^\s*version\.workspace\s*=\s*true\s*$/m.test(manifestText)) {
    issues.push({ line: 1, message: "loader provider crate must inherit the workspace version." });
  }
  if (!/^\s*publish\s*=\s*false\s*$/m.test(manifestText)) {
    issues.push({ line: 1, message: "loader provider crate must declare publish = false." });
  }
  const declared = cargoDeclaredDependencyNames(manifestText);
  const expected = [...spec.dependencies].sort();
  if (JSON.stringify(declared) !== JSON.stringify(expected)) {
    issues.push({
      line: 1,
      message: `loader provider dependencies must be exactly ${JSON.stringify(expected)}, found ${JSON.stringify(declared)}.`,
    });
  }
  return issues;
}

function checkLoaderProviderCrateManifests(report) {
  for (const spec of LOADER_PROVIDER_CRATE_SPECS) {
    const absolutePath = path.join(REPO_ROOT, spec.manifest);
    if (!fs.existsSync(absolutePath)) {
      report("RUST-PHASE-TARGET-001", spec.manifest, 1, "the exact loader provider crate manifest is missing.");
      continue;
    }
    for (const issue of evaluateLoaderProviderCrateManifest(spec.manifest, readText(absolutePath))) {
      report("RUST-PROVIDER-DEP-ALLOW-001", spec.manifest, issue.line, issue.message);
    }
  }
}

export function evaluateLoaderProviderLockEntries(lockText) {
  const issues = [];
  const host = /\[\[package\]\]\s*\r?\nname = "flint"\s*\r?\nversion = "([^"]+)"/.exec(lockText);
  if (!host) return [{ line: 1, message: "Cargo.lock must contain the host package 'flint' with its workspace version." }];
  const version = host[1];
  for (const spec of LOADER_PROVIDER_CRATE_SPECS) {
    const pattern = new RegExp(`\\[\\[package\\]\\]\\r?\\nname = "${spec.crateName}"\\r?\\nversion = "([^"]+)"`);
    const match = pattern.exec(lockText);
    if (!match) {
      issues.push({ line: 1, message: `Cargo.lock must contain local package '${spec.crateName}'.` });
    } else if (match[1] !== version) {
      issues.push({ line: 1, message: `Cargo.lock package '${spec.crateName}' version '${match[1]}' does not match host workspace version '${version}'.` });
    }
  }
  return issues;
}
export function evaluateHostLoaderDependencyDeclarations(manifestText) {
  const issues = [];
  for (const spec of LOADER_PROVIDER_CRATE_SPECS) {
    const relativePath = spec.manifest
      .replace(/^src-tauri\//, "")
      .replace(/\/Cargo\.toml$/, "");
    const declaration = `${spec.crateName} = { path = "${relativePath}" }`;
    if (!manifestText.includes(declaration)) {
      issues.push({
        line: 1,
        message: `host manifest must declare loader provider path dependency '${declaration}'.`,
      });
    }
  }
  return issues;
}
export function reviewedProviderTargets() {
  return [...MOD_PROVIDER_TARGETS, ...LOADER_PROVIDER_TARGETS].map((target) => ({
    file: target.file,
    role: target.role,
    localTargets: [...target.localTargets].sort(),
    transitionalLocalTargets: [...target.transitionalLocalTargets.keys()].sort(),
    externalRoots: [...target.externalRoots].sort(),
  }));
}
const FORBIDDEN_PROVIDER_CODE = [
  [/\btauri(?:_[A-Za-z0-9_]+)?\s*::/g, "direct Tauri path"],
  [/\bstd\s*::[^;\n]*\b(?:fs|process|net)\b/g, "direct std filesystem/process/network path"],
  [/\bstd\s*::[^;\n]*\bos\b[^;\n]*\b(?:fs|process)\b/g, "platform process/filesystem extension"],
  [/\btokio\s*::[^;\n]*\bprocess\b/g, "tokio process path"],
  [/\bsettings\s*::/g, "direct settings module path"],
  [/\bCommand\s*::\s*new\s*\(/g, "process spawning"],
  [/\bFile\s*::\s*(?:open|create|create_new)\s*\(/g, "direct filesystem file access"],
  [/\b(?:TcpStream|TcpListener|UdpSocket)\b/g, "direct socket access"],
];
const PROVIDER_CONTEXT_PORT_ACCESSOR = /\b[A-Za-z_][A-Za-z0-9_]*\.(?:network|metadata_network|installer_network|workspace|safe_workspace|java|progress|logger|timeouts?|deadline|cancellation|check_control|root|scoped_workspace)\s*\(/;
const PROVIDER_TYPED_PORT_CODE_PATTERNS = [
  [/\breqwest\b/g, "direct reqwest API use"],
  [/\bzip\s*::/g, "direct zip API use"],
  [/\btauri(?:_[A-Za-z0-9_]+)?\s*::/g, "direct Tauri path"],
  [/\b(?:AppHandle|Emitter)\b/g, "Tauri host handle/emitter"],
  [/\b(?:std\s*::\s*)?(?:fs|process|net)\s*::/g, "raw filesystem/process/network path"],
  [/\bCommand\s*::\s*new\s*\(/g, "process spawning"],
  [/\bFile\b/g, "direct File API"],
  [/\b(?:OpenOptions|TcpStream|TcpListener|UdpSocket|UnixStream|UnixListener)\b/g, "direct socket/filesystem API"],
  [/\bflint_(?:mod|loader)_provider_[A-Za-z0-9_]+\b/g, "provider-to-provider import"],
  [/\bflint_feature_[A-Za-z0-9_]+\b/g, "feature crate import"],
  [/\b(?:flint|flint_lib)\s*::/g, "host crate import"],
];
const PROVIDER_CONCRETE_CORE_ADAPTER_PATTERN = /\b(?:ReqwestNetwork|SafeWorkspace|CommandProcessPort|ProcessJavaRunner|NoopLogger|NoopProgress)\b/g;
const PROVIDER_REGISTRY_CONSTRUCTION_PATTERN = /\bProviderRegistry(?:\s*::\s*<[^;{}()]*>)?\s*::\s*new\s*\(/g;
export function evaluateProviderTypedPortSource(providerKind, sourceText, { requireContract = true } = {}) {
  const issues = [];
  const add = (rule, message, index = 0) => issues.push({ rule, line: lineOf(sourceText, index), message });
  const expectedTrait = providerKind === "loader" ? "LoaderProvider" : providerKind === "mod" ? "ModProvider" : null;
  if (!expectedTrait) {
    add("RUST-PROVIDER-TYPED-PORT-001", `unknown provider kind '${providerKind}'.`);
    return issues;
  }
  if (requireContract) {
    const implementation = new RegExp(`\\bimpl\\s+(?:flint_provider_api\\s*::\\s*(?:[A-Za-z_][A-Za-z0-9_]*\\s*::\\s*)?)?${expectedTrait}\\s+for\\b`);
    if (!implementation.test(sourceText)) {
      add("RUST-PROVIDER-TYPED-PORT-001", `${providerKind} provider must implement flint_provider_api::${expectedTrait}.`);
    }
    if (!/\bflint_provider_api\b/.test(sourceText)) {
      add("RUST-PROVIDER-TYPED-PORT-001", `${providerKind} provider must use the reviewed flint-provider-api contract.`);
    }
    if (!/\bflint_core\s*::/.test(sourceText)) {
      add("RUST-PROVIDER-TYPED-PORT-001", `${providerKind} provider must use flint-core typed policy.`);
    }
    if (!PROVIDER_CONTEXT_PORT_ACCESSOR.test(sourceText)) {
      add("RUST-PROVIDER-TYPED-PORT-001", `${providerKind} provider must consume typed context ports instead of raw host APIs.`);
    }
  }
  for (const declaration of extractRustUseDeclarations(sourceText)) {
    const root = firstUseRoot(declaration.payload);
    if (!root) {
      add("RUST-PROVIDER-IMPORT-001", "unparseable provider use declaration.", declaration.index);
      continue;
    }
    let forbidden = null;
    if (root === "reqwest") forbidden = "direct reqwest import";
    else if (root === "zip") forbidden = "direct zip import";
    else if (root === "tauri" || root.startsWith("tauri_")) forbidden = "Tauri import";
    else if (["fs", "process", "net"].includes(root)) forbidden = `${root} import`;
    else if (root === "std" && /\b(?:fs|process|net)\b/.test(declaration.payload)) forbidden = "std filesystem/process/network import";
    else if (root === "tokio" && /\bprocess\b/.test(declaration.payload)) forbidden = "tokio process import";
    else if (root.startsWith("flint_mod_provider_") || root.startsWith("flint_loader_provider_")) forbidden = "provider-to-provider import";
    else if (root.startsWith("flint_feature_")) forbidden = "feature crate import";
    else if (root === "flint" || root === "flint_lib" || root.startsWith("flint::") || root.startsWith("flint_lib::")) forbidden = "host crate import";
    else if (HOST_PRIVATE_MODULES.has(root)) forbidden = "host-private module import";
    if (forbidden) add("RUST-PROVIDER-IMPORT-001", `forbidden provider import (${forbidden}).`, declaration.index);
  }
  for (const [pattern, label] of PROVIDER_TYPED_PORT_CODE_PATTERNS) {
    pattern.lastIndex = 0;
    let match;
    while ((match = pattern.exec(sourceText)) !== null) {
      add("RUST-PROVIDER-IO-001", `forbidden provider code: ${label}.`, match.index);
    }
  }
  PROVIDER_CONCRETE_CORE_ADAPTER_PATTERN.lastIndex = 0;
  let adapterMatch;
  while ((adapterMatch = PROVIDER_CONCRETE_CORE_ADAPTER_PATTERN.exec(sourceText)) !== null) {
    add("RUST-PROVIDER-CORE-ADAPTER-001", `provider leaves must consume typed context ports and may not reference or construct concrete core adapter '${adapterMatch[0]}'.`, adapterMatch.index);
  }
  PROVIDER_REGISTRY_CONSTRUCTION_PATTERN.lastIndex = 0;
  let registryMatch;
  while ((registryMatch = PROVIDER_REGISTRY_CONSTRUCTION_PATTERN.exec(sourceText)) !== null) {
    add("RUST-PROVIDER-REGISTRY-CONSTRUCTION-001", "provider leaves may not construct ProviderRegistry; immutable registries are composed only in src-tauri/src/provider_composition.rs.", registryMatch.index);
  }
  return issues;
}
const FEATURE_REGISTRY_CONSUMER_SPECS = new Map([
  ["flint-feature-mods", { trait: "ModProvider", forbiddenTrait: "LoaderProvider" }],
  ["flint-feature-loaders", { trait: "LoaderProvider", forbiddenTrait: "ModProvider" }],
]);
const CONCRETE_PROVIDER_TYPE_PATTERN = /\b(?:CurseForgeModProvider|ModrinthModProvider|FabricLoaderProvider|QuiltLoaderProvider|ForgeLoaderProvider|NeoForgeLoaderProvider|LiteLoaderProvider|OptiFineProvider)\b/g;
export function evaluateFeatureProviderRegistryConsumer(featureName, sourceText) {
  const spec = FEATURE_REGISTRY_CONSUMER_SPECS.get(featureName);
  if (!spec) {
    return [{ rule: "RUST-FEATURE-PROVIDER-REGISTRY-001", line: 1, message: `unknown provider-registry consumer '${featureName}'.` }];
  }
  const issues = [];
  const add = (message, index = 0) => issues.push({ rule: "RUST-FEATURE-PROVIDER-REGISTRY-001", line: lineOf(sourceText, index), message });
  if (!/\bflint_provider_api\b/.test(sourceText)) {
    add(`feature '${featureName}' must import the reviewed flint-provider-api contract.`);
  }
  const registryType = new RegExp(`\\bProviderRegistry\\s*<\\s*dyn\\s+${spec.trait}\\s*>`);
  if (!registryType.test(sourceText)) {
    add(`feature '${featureName}' must receive ProviderRegistry<dyn ${spec.trait}>.`);
  }
  const registryConstruction = /\bProviderRegistry\s*::\s*new\s*\(/.exec(sourceText);
  if (registryConstruction) {
    add("feature crates may not construct provider registries; construction belongs only in provider_composition.", registryConstruction.index);
  }
  const forbiddenTrait = new RegExp(`\\b${spec.forbiddenTrait}\\b`).exec(sourceText);
  if (forbiddenTrait) {
    add(`feature '${featureName}' imports the wrong provider contract '${spec.forbiddenTrait}'.`, forbiddenTrait.index);
  }
  for (const declaration of extractRustUseDeclarations(sourceText)) {
    const root = firstUseRoot(declaration.payload);
    if (!root) {
      add("unparseable feature use declaration.", declaration.index);
      continue;
    }
    if (root === "tauri" || root.startsWith("tauri_")) {
      add("feature provider consumers may not import Tauri.", declaration.index);
    } else if (root === "flint" || root === "flint_lib") {
      add("feature provider consumers may not import the host crate.", declaration.index);
    } else if (root.startsWith("flint_mod_provider_") || root.startsWith("flint_loader_provider_")) {
      add("feature provider consumers may not import concrete provider crates.", declaration.index);
    } else if (HOST_PRIVATE_MODULES.has(root) || FEATURE_PRIVATE_MODULE_NAMES.has(root)) {
      add(`feature provider consumer imports host-private module '${root}'.`, declaration.index);
    }
  }
  const tauriCode = /\btauri(?:_[A-Za-z0-9_]+)?\s*::|\b(?:AppHandle|Emitter)\b/.exec(sourceText);
  if (tauriCode) add("feature provider consumers may not use Tauri host APIs.", tauriCode.index);
  CONCRETE_PROVIDER_TYPE_PATTERN.lastIndex = 0;
  const concreteType = CONCRETE_PROVIDER_TYPE_PATTERN.exec(sourceText);
  if (concreteType) add(`feature provider consumer imports concrete provider '${concreteType[0]}'.`, concreteType.index);
  return issues;
}
export function evaluateFoundationCrateSource(crateName, sourceText) {
  if (!["flint-core", "flint-feature-api", "flint-provider-api"].includes(crateName)) {
    return [{ rule: "RUST-FOUNDATION-IMPORT-001", line: 1, message: `unknown foundation source crate '${crateName}'.` }];
  }
  const issues = [];
  const add = (message, index = 0) => issues.push({ rule: "RUST-FOUNDATION-IMPORT-001", line: lineOf(sourceText, index), message });
  for (const declaration of extractRustUseDeclarations(sourceText)) {
    const root = firstUseRoot(declaration.payload);
    if (!root) {
      add("unparseable foundation use declaration.", declaration.index);
      continue;
    }
    if (root === "tauri" || root.startsWith("tauri_")) {
      add("core/provider API sources may not import Tauri.", declaration.index);
    } else if (root === "flint" || root === "flint_lib") {
      add("core/provider API sources may not import the host crate.", declaration.index);
    } else if (root.startsWith("flint_feature_") || root.startsWith("flint_mod_provider_") || root.startsWith("flint_loader_provider_")) {
      add("core/provider API sources may not import feature or concrete provider crates.", declaration.index);
    } else if (crateName === "flint-core" && (root === "flint_provider_api" || root === "flint_feature_api")) {
      add("flint-core may not import provider or feature API crates.", declaration.index);
    } else if (crateName === "flint-feature-api" && root === "flint_provider_api") {
      add("flint-feature-api may not import the provider API crate.", declaration.index);
    }
  }
  const forbiddenCode = /\btauri(?:_[A-Za-z0-9_]+)?\s*::|\b(?:AppHandle|Emitter)\b|\bflint_feature_[A-Za-z0-9_]+\b|\bflint_(?:mod|loader)_provider_[A-Za-z0-9_]+\b|\b(?:flint|flint_lib)\s*::/g;
  let match;
  while ((match = forbiddenCode.exec(sourceText)) !== null) {
    add(`forbidden foundation source import '${match[0]}'.`, match.index);
  }
  if (crateName === "flint-core") {
    const apiImport = /\b(?:flint_provider_api|flint_feature_api)\b/g;
    while ((match = apiImport.exec(sourceText)) !== null) {
      add("flint-core may not import provider or feature API crates.", match.index);
    }
  }
  return issues;
}
function providerTargetAllowed(phase, target, localTarget) {
  if (target.localTargets.has(localTarget)) return true;
  const waiver = target.transitionalLocalTargets.get(localTarget);
  return Boolean(waiver && phase < waiver.removeByPhase);
}
function forbiddenProviderUse(payload, targets) {
  const root = firstUseRoot(payload);
  if (!root) return "unparseable use declaration";
  if (root === "tauri" || root.startsWith("tauri_")) return "Tauri import";
  if (["fs", "process", "net", "settings", "paths"].includes(root)) return `${root} import`;
  if (root === "std" && /\b(?:fs|process|net)\b/.test(payload)) return "std filesystem/process/network import";
  if (root === "tokio" && /\bprocess\b/.test(payload)) return "tokio process import";
  if ([...targets].some((target) => HOST_PRIVATE_MODULES.has(target))) return "host-private module import";
  return null;
}
function checkProviderTarget(phase, target, graph, report) {
  let record = graph.records.find((candidate) => candidate.relative === target.file);
  if (!record) {
    const absolutePath = path.join(REPO_ROOT, target.file);
    if (!fs.existsSync(absolutePath)) return;
    try {
      record = {
        absolutePath,
        relative: target.file,
        moduleId: "crate",
        productionText: prepareRustProductionSource(readText(absolutePath)),
      };
    } catch (error) {
      report("RUST-LEX-001", target.file, 1, `cannot statically mask Rust source: ${error.message}`);
      return;
    }
  }
  const localTargets = new Set();
  for (const declaration of extractRustUseDeclarations(record.productionText)) {
    const root = firstUseRoot(declaration.payload);
    if (!root) continue;
    const local =
      root === "crate" || root === "super" || root === "self" ||
      graph.moduleSet.has(root) || graph.moduleSet.has(`${record.moduleId}::${root}`);
    const declarationTargets = local
      ? moduleTargetsFromUse(declaration.payload, record.moduleId, graph.moduleSet)
      : new Set();
    for (const localTarget of declarationTargets) localTargets.add(localTarget);
    const forbidden =
      target.role === "loader-provider-adapter"
        ? null
        : forbiddenProviderUse(declaration.payload, declarationTargets);
    if (forbidden) {
      report("RUST-PROVIDER-IMPORT-001", record.relative, declaration.line, `forbidden provider import (${forbidden}).`);
      continue;
    }
    if (!local && !target.externalRoots.has(root)) {
      report("RUST-PROVIDER-IMPORT-ALLOW-001", record.relative, declaration.line, `provider external crate root '${root}' is not in the exact reviewed allowlist (${[...target.externalRoots].sort().join(", ")}).`);
    }
  }
  for (const localTarget of localTargets) {
    if (!providerTargetAllowed(phase, target, localTarget)) {
      const waiver = target.transitionalLocalTargets.get(localTarget);
      report(
        "RUST-PROVIDER-IMPORT-ALLOW-001",
        record.relative,
        1,
        waiver && phase >= waiver.removeByPhase
          ? `provider local import '${localTarget}' waiver expired at phase ${waiver.removeByPhase}: ${waiver.removal}`
          : `provider local import '${localTarget}' is not in the exact reviewed allowlist.`,
      );
    }
  }
  if (
    (target.role.startsWith("mod-provider") || target.role.startsWith("loader-provider")) &&
    /\breqwest\s*::/.test(record.productionText)
  ) {
    report("RUST-PROVIDER-IO-001", record.relative, lineOf(record.productionText, record.productionText.search(/\breqwest\s*::/)), "forbidden provider code: direct reqwest path.");
  }
  if (/\bflint_(?:loader|mod)_provider_[a-z0-9_]+\b/.test(record.productionText)) {
    report("RUST-PROVIDER-IMPORT-001", record.relative, 1, "loader/mod provider leaves may not import one another or concrete provider crates.");
  }
  if (target.role !== "loader-provider-adapter" && /\bLoaderKind\b/.test(record.productionText)) {
    report("RUST-LOADER-DISPATCH-001", record.relative, lineOf(record.productionText, record.productionText.search(/\bLoaderKind\b/)), "LoaderKind belongs only in the sole host adapter src-tauri/src/loaders.rs.");
  }
  if (target.role !== "loader-provider-adapter") {
    for (const [pattern, label] of FORBIDDEN_PROVIDER_CODE) {
      pattern.lastIndex = 0;
      let match;
      while ((match = pattern.exec(record.productionText)) !== null) {
        report("RUST-PROVIDER-IO-001", record.relative, lineOf(record.productionText, match.index), `forbidden provider code: ${label}.`);
      }
    }
  }
}
function findRustFunctionSpans(text) {
  const spans = [];
  const pattern = /\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^>]*>)?\s*\([^)]*\)[^{;]*\{/g;
  let match;
  while ((match = pattern.exec(text)) !== null) {
    const openBrace = text.indexOf("{", match.index);
    const closeBrace = findMatchingBrace(text, openBrace);
    if (closeBrace === -1) continue;
    spans.push({ name: match[1], start: match.index, end: closeBrace + 1, line: lineOf(text, match.index) });
    pattern.lastIndex = closeBrace + 1;
  }
  return spans;
}
// Exact dispatch identifiers: `source`, or intentionally named `*_source`.
// Deliberately does not match unrelated identifiers such as `resource`.
const SOURCE_DISPATCH_IDENTIFIER = /\b(?:source|[A-Za-z0-9_]+_source)\b/;
export function findMatchSourceSites(text) {
  const sites = [];
  const pattern = /\bmatch\s+([^\{;]+)\{/g;
  let match;
  while ((match = pattern.exec(text)) !== null) {
    if (!SOURCE_DISPATCH_IDENTIFIER.test(match[1])) continue;
    sites.push({ index: match.index, kind: "match-source", line: lineOf(text, match.index) });
  }
  return sites;
}
function findRustEnumAliasDispatchSites(text, enumName) {
  const sites = [];
  const aliases = new Map();
  const add = (index, kind) => sites.push({ index, kind, line: lineOf(text, index) });
  const aliasImport = new RegExp(`\\buse\\b[^;]*?\\b${enumName}\\s+as\\s+([A-Za-z_][A-Za-z0-9_]*)`, "g");
  let match;
  while ((match = aliasImport.exec(text)) !== null) {
    aliases.set(match[1], "import");
    add(match.index, `${enumName}-import-alias`);
  }
  const typeAlias = new RegExp(`\\btype\\s+([A-Za-z_][A-Za-z0-9_]*)\\s*=\\s*(?:[A-Za-z_][A-Za-z0-9_]*\\s*::\\s*)*${enumName}\\s*;`, "g");
  while ((match = typeAlias.exec(text)) !== null) {
    aliases.set(match[1], "type");
    add(match.index, `${enumName}-type-alias`);
  }
  const variantImport = new RegExp(`\\buse\\b[^;]*?\\b${enumName}\\s*::`, "g");
  while ((match = variantImport.exec(text)) !== null) add(match.index, `${enumName}-variant-import`);
  for (const alias of aliases.keys()) {
    const aliasUse = new RegExp(`\\b${alias}\\s*::\\s*[A-Za-z_][A-Za-z0-9_]*`, "g");
    while ((match = aliasUse.exec(text)) !== null) add(match.index, `${enumName}-alias-variant`);
    const aliasMatch = new RegExp(`\\bmatch\\s+[^\\{;]*\\b${alias}\\b[^\\{;]*\\{`, "g");
    while ((match = aliasMatch.exec(text)) !== null) add(match.index, `${enumName}-alias-match`);
  }
  return sites;
}
export function findModSourceAliasDispatchSites(text) {
  return findRustEnumAliasDispatchSites(text, "ModSource");
}
export function findModSourceDispatchSites(text) {
  return [...findMatchSourceSites(text), ...findModSourceAliasDispatchSites(text)];
}
export function findLoaderKindDispatchSites(text) {
  const sites = [];
  const direct = /\bLoaderKind\s*::\s*[A-Za-z_][A-Za-z0-9_]*/g;
  let match;
  while ((match = direct.exec(text)) !== null) sites.push({ index: match.index, kind: "LoaderKind-variant", line: lineOf(text, match.index) });
  sites.push(...findRustEnumAliasDispatchSites(text, "LoaderKind"));
  const matchLoader = /\bmatch\s+([^\{;]+)\{/g;
  while ((match = matchLoader.exec(text)) !== null) {
    if (!/\b(?:loader|[A-Za-z0-9_]+_loader)\b/.test(match[1])) continue;
    const openBrace = text.indexOf("{", match.index);
    const closeBrace = findMatchingBrace(text, openBrace);
    if (closeBrace === -1) continue;
    const block = text.slice(openBrace, closeBrace + 1);
    if (/\b(?:select_for_loader|build_registry|LoaderProvider|ProviderRequirement)\b/.test(block)) {
      sites.push({ index: match.index, kind: "LoaderKind-provider-match", line: lineOf(text, match.index) });
    }
  }
  return sites;
}
const LOADER_PROVIDER_CONSTRUCTORS = [
  "FabricLoaderProvider::new",
  "ForgeLoaderProvider::new",
  "LiteLoaderProvider::new",
  "NeoForgeLoaderProvider::new",
  "OptiFineProvider::new",
  "QuiltLoaderProvider::new",
];
const LOADER_PROVIDER_DESCRIPTOR_ALIASES = [
  "FABRIC_LOADER_DESCRIPTOR",
  "FORGE_LOADER_DESCRIPTOR",
  "LITELOADER_LOADER_DESCRIPTOR",
  "NEOFORGE_LOADER_DESCRIPTOR",
  "OPTIFINE_LOADER_DESCRIPTOR",
  "QUILT_LOADER_DESCRIPTOR",
];
const LOADER_PROVIDER_SELECTION_KEY_NAMES = [
  "FABRIC_SELECTION_KEY",
  "FORGE_SELECTION_KEY",
  "LITELOADER_SELECTION_KEY",
  "NEOFORGE_SELECTION_KEY",
  "OPTIFINE_SELECTION_KEY",
  "QUILT_SELECTION_KEY",
];
export function evaluateLoaderProviderComposition(text) {
  const issues = [];
  const spans = findRustFunctionSpans(text);
  const descriptorSpan = spans.find((span) => span.name === "loader_provider_descriptors");
  const requirementSpan = spans.find((span) => span.name === "loader_provider_requirements");
  const registrySpan = spans.find((span) => span.name === "build_loader_provider_registry");
  const stateDeclarations = (text.match(/\bstruct\s+LoaderProviderState\b/g) ?? []).length;
  if (stateDeclarations !== 1 || !text.includes("pub struct LoaderProviderState")) {
    issues.push({ line: 1, message: "loader provider composition must expose exactly one LoaderProviderState." });
  }
  if (!text.includes("Arc<ProviderRegistry<dyn LoaderProvider>>")) {
    issues.push({ line: 1, message: "LoaderProviderState must own an Arc<ProviderRegistry<dyn LoaderProvider>>." });
  }
  for (const descriptor of LOADER_PROVIDER_DESCRIPTOR_ALIASES) {
    if (!text.includes(descriptor)) issues.push({ line: 1, message: `loader provider composition must expose leaf descriptor '${descriptor}'.` });
  }
  for (const key of LOADER_PROVIDER_SELECTION_KEY_NAMES) {
    if (!text.includes(key)) issues.push({ line: 1, message: `loader provider composition must declare requirement key '${key}'.` });
  }
  if (!descriptorSpan) {
    issues.push({ line: 1, message: "loader provider composition must define loader_provider_descriptors()." });
  } else {
    const block = text.slice(descriptorSpan.start, descriptorSpan.end);
    for (const descriptor of LOADER_PROVIDER_DESCRIPTOR_ALIASES) {
      if (!block.includes(descriptor)) issues.push({ line: descriptorSpan.line, message: `loader_provider_descriptors() must use leaf descriptor '${descriptor}'.` });
    }
  }
  if (!requirementSpan) {
    issues.push({ line: 1, message: "loader provider composition must define loader_provider_requirements()." });
  } else {
    const block = text.slice(requirementSpan.start, requirementSpan.end);
    for (const key of LOADER_PROVIDER_SELECTION_KEY_NAMES) {
      if (!block.includes(key)) issues.push({ line: requirementSpan.line, message: `loader_provider_requirements() must use key '${key}'.` });
    }
  }
  if (!registrySpan) {
    issues.push({ line: 1, message: "loader provider composition must define build_loader_provider_registry()." });
  } else {
    const block = text.slice(registrySpan.start, registrySpan.end);
    for (const constructor of LOADER_PROVIDER_CONSTRUCTORS) {
      if (!block.includes(constructor)) issues.push({ line: registrySpan.line, message: `loader provider registry must register '${constructor}'.` });
    }
    const constructions = (block.match(/\bProviderRegistry\s*::\s*new\s*\(/g) ?? []).length;
    if (constructions !== 1) issues.push({ line: registrySpan.line, message: `build_loader_provider_registry() must construct exactly one provider registry, found ${constructions}.` });
  }
  return issues;
}
function checkProviderCompositionBoundaries(graph, report) {
  const compositionRelative = "src-tauri/src/provider_composition.rs";
  const composition = graph.records.find((record) => record.relative === compositionRelative);
  if (!composition) {
    report("RUST-PHASE-TARGET-001", compositionRelative, 1, "the explicit host provider composition target is missing.");
    return;
  }
  const production = composition.productionText;
  for (const constructor of ["CurseForgeModProvider::new", "ModrinthModProvider::new"]) {
    if (!production.includes(constructor)) {
      report("RUST-PROVIDER-COMPOSITION-001", compositionRelative, 1, `host composition must register '${constructor}'.`);
    }
  }
  const registryConstructions = (production.match(/\bProviderRegistry\s*::\s*new\s*\(/g) ?? []).length;
  if (registryConstructions !== 2) {
    report("RUST-PROVIDER-COMPOSITION-001", compositionRelative, 1, `host composition must construct one mod and one loader provider registry, found ${registryConstructions} registry constructions.`);
  }
  const modRegistrySpan = findRustFunctionSpans(production).find((span) => span.name === "build_mod_provider_registry");
  const modConstructions = modRegistrySpan
    ? (production.slice(modRegistrySpan.start, modRegistrySpan.end).match(/\bProviderRegistry\s*::\s*new\s*\(/g) ?? []).length
    : 0;
  if (!modRegistrySpan || modConstructions !== 1) {
    report("RUST-PROVIDER-COMPOSITION-001", compositionRelative, modRegistrySpan?.line ?? 1, "build_mod_provider_registry() must construct exactly one provider registry.");
  }
  for (const issue of evaluateLoaderProviderComposition(production)) {
    report("RUST-PROVIDER-COMPOSITION-001", compositionRelative, issue.line, issue.message);
  }
  for (const record of graph.records) {
    if (record.relative === compositionRelative) continue;
    if (/\b(?:CurseForgeModProvider|ModrinthModProvider|FabricLoaderProvider|ForgeLoaderProvider|LiteLoaderProvider|NeoForgeLoaderProvider|OptiFineProvider|QuiltLoaderProvider)\b|flint_(?:mod|loader)_provider_[a-z0-9_]+\b/.test(record.productionText)) {
      report("RUST-PROVIDER-COMPOSITION-001", record.relative, 1, "concrete provider construction/imports must stay in src-tauri/src/provider_composition.rs.");
    }
    if (record.productionText.includes("ModProvider") && /\bProviderRegistry\s*::\s*new\s*\(/.test(record.productionText)) {
      report("RUST-PROVIDER-COMPOSITION-001", record.relative, 1, "the mod provider registry may only be constructed in src-tauri/src/provider_composition.rs.");
    }
    if (record.productionText.includes("LoaderProvider") && /\bProviderRegistry\s*::\s*new\s*\(/.test(record.productionText)) {
      report("RUST-PROVIDER-COMPOSITION-001", record.relative, 1, "the loader provider registry may only be constructed in src-tauri/src/provider_composition.rs.");
    }
  }
}

function checkModSourceBoundaries(graph, report) {
  const bindingRelative = "src-tauri/modules/flint-feature-mods/src/lib.rs";
  let bindingRecord = graph.records.find((record) => record.relative === bindingRelative);
  if (!bindingRecord) {
    const absolute = path.join(REPO_ROOT, bindingRelative);
    if (!fs.existsSync(absolute)) {
      report("RUST-PHASE-TARGET-001", bindingRelative, 1, "the sole ModSource-to-provider binding target is missing.");
      return;
    }
    bindingRecord = {
      relative: bindingRelative,
      productionText: prepareRustProductionSource(readText(absolute)),
    };
  }
  const sourceBinding = findRustFunctionSpans(bindingRecord.productionText)
    .find((span) => span.name === "source_binding");
  const bindingSites = findModSourceDispatchSites(bindingRecord.productionText);
  const allowedSites = bindingSites.filter(
    (site) => sourceBinding && site.index >= sourceBinding.start && site.index < sourceBinding.end,
  );
  const unexpectedBindingSites = bindingSites.filter((site) => !allowedSites.includes(site));
  if (!sourceBinding || allowedSites.length !== 1 || unexpectedBindingSites.length > 0) {
    report(
      "RUST-MODSOURCE-ADAPTER-001",
      bindingRelative,
      sourceBinding?.line ?? 1,
      "expected exactly one reviewed ModSource-to-provider binding inside source_binding in the mods feature crate and no other production ModSource dispatch.",
    );
  }
  for (const record of graph.records) {
    if (record.relative === bindingRelative) continue;
    for (const site of findModSourceDispatchSites(record.productionText)) {
      report(
        "RUST-MODSOURCE-DISPATCH-001",
        record.relative,
        site.line,
        `ModSource dispatch form '${site.kind}' is forbidden outside the sole source_binding in the mods feature crate.`,
      );
    }
  }
}

export function requiredLoaderFiles(phase, report, exists = (file) => fs.existsSync(path.join(REPO_ROOT, file))) {
  if (phase < 2) return [];
  const required = [...LOADER_API_FILES, ...LOADER_ADAPTER_FILES];
  const present = [];
  for (const file of required) {
    if (exists(file)) {
      present.push(file);
    } else {
      report("RUST-PHASE-TARGET-001", file, 1, `phase ${phase} requires the exact reviewed loader-provider API/adapter target '${file}'.`);
    }
  }
  for (const legacy of [
    "src-tauri/src/loader_api.rs",
    "src-tauri/src/loader_providers.rs",
    "src-tauri/src/loaders/provider_api.rs",
    "src-tauri/src/loaders/providers.rs",
  ]) {
    if (exists(legacy)) {
      report("RUST-LOADER-ADAPTER-001", legacy, 1, "legacy host-local loader adapter/API target is forbidden; use the extracted provider API and src-tauri/src/loaders.rs.");
    }
  }
  return present;
}
function rustFilesUnder(relativeDirectory) {
  const files = [];
  const root = path.join(REPO_ROOT, relativeDirectory);
  if (!fs.existsSync(root)) return files;
  const stack = [root];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const absolute = path.join(current, entry.name);
      if (entry.isDirectory()) stack.push(absolute);
      else if (entry.isFile() && entry.name.endsWith(".rs")) files.push(relFromRepo(absolute));
    }
  }
  return files.sort();
}
export function evaluateFeatureRegistryConstruction(text) {
  const match = /\bProviderRegistry\s*::\s*new\s*\(/.exec(text);
  return match
    ? [{ line: lineOf(text, match.index), message: "feature crates may not construct provider registries; construction belongs only in provider_composition." }]
    : [];
}

function checkFeatureTarget(phase, target, graph, report) {
  let record = graph.records.find((candidate) => candidate.relative === target.file);
  if (!record) {
    const absolutePath = path.join(REPO_ROOT, target.file);
    if (!fs.existsSync(absolutePath)) return;
    try {
      record = {
        absolutePath,
        relative: target.file,
        moduleId: "crate",
        productionText: prepareRustProductionSource(readText(absolutePath)),
      };
    } catch (error) {
      report("RUST-LEX-001", target.file, 1, `cannot statically mask feature source: ${error.message}`);
      return;
    }
  }
  for (const declaration of extractRustUseDeclarations(record.productionText)) {
    const root = firstUseRoot(declaration.payload);
    if (!root) {
      report("RUST-FEATURE-IMPORT-001", record.relative, declaration.line, "unparseable feature use declaration.");
      continue;
    }
    if (root === "crate" || root === "super" || root === "self") {
      const identifiers = [...declaration.payload.matchAll(/\b([A-Za-z_][A-Za-z0-9_]*)\b/g)]
        .map((match) => match[1])
        .filter((name) => !["crate", "self", "super"].includes(name));
      const privateImport = identifiers.find(
        (name) => FEATURE_PRIVATE_MODULE_NAMES.has(name) && !target.localTargets.has(name),
      );
      if (privateImport) {
        report("RUST-FEATURE-IMPORT-001", record.relative, declaration.line, `feature crate imports host/private crate module '${privateImport}'.`);
      }
      continue;
    }
    if (target.localTargets.has(root)) {
      continue;
    }
    if (root === "tauri" || root.startsWith("tauri_")) {
      report("RUST-FEATURE-IO-001", record.relative, declaration.line, "feature crates may not import Tauri.");
      continue;
    }
    if (graph.moduleSet.has(root) || HOST_PRIVATE_MODULES.has(root)) {
      report("RUST-FEATURE-IMPORT-001", record.relative, declaration.line, `feature crate imports host-private module '${root}'.`);
      continue;
    }
    if (!target.externalRoots.has(root)) {
      report("RUST-FEATURE-IMPORT-ALLOW-001", record.relative, declaration.line, `feature external crate root '${root}' is not in the exact reviewed allowlist (${[...target.externalRoots].sort().join(", ")}).`);
    }
  }
  const tauriUse = /\btauri(?:_[A-Za-z0-9_]+)?\s*::/.exec(record.productionText);
  if (tauriUse) {
    report("RUST-FEATURE-IO-001", record.relative, lineOf(record.productionText, tauriUse.index), "feature crates may not use Tauri paths.");
  }
  const hostHandle = /\b(?:AppHandle|Emitter)\b/.exec(record.productionText);
  if (hostHandle) {
    report("RUST-FEATURE-IO-001", record.relative, lineOf(record.productionText, hostHandle.index), "feature crates may not use Tauri host handles or emitters.");
  }
  for (const issue of evaluateFeatureRegistryConstruction(record.productionText)) {
    report("RUST-FEATURE-COMPOSITION-001", record.relative, issue.line, issue.message);
  }
}
function checkFeatureCrateGate(phase, graph, report) {
  if (phase < 3) return;
  const modulesRoot = path.join(REPO_ROOT, "src-tauri/modules");
  if (fs.existsSync(modulesRoot)) {
    const directories = fs
      .readdirSync(modulesRoot, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name);
    for (const issue of evaluateFeatureDirectoryNames(directories)) {
      report("RUST-FEATURE-TARGET-001", "src-tauri/modules", 1, issue);
    }
  }
  for (const spec of FEATURE_CRATE_SPECS) {
    if (!fs.existsSync(path.join(REPO_ROOT, spec.source))) {
      report("RUST-PHASE-TARGET-001", spec.source, 1, `phase ${phase} requires the exact feature leaf '${spec.crateName}'.`);
      continue;
    }
    const target = FEATURE_TARGET_BY_SOURCE.get(spec.source);
    if (!target) continue;
    checkFeatureTarget(phase, target, graph, report);
    for (const file of rustFilesUnder(path.dirname(spec.source))) {
      if (file === spec.source) continue;
      checkFeatureTarget(phase, { ...target, file }, graph, report);
    }
  }
  const modsAdapter = path.join(REPO_ROOT, "src-tauri/src/mods.rs");
  const loadersAdapter = path.join(REPO_ROOT, "src-tauri/src/loaders.rs");
  if (!fs.existsSync(modsAdapter) || !fs.existsSync(loadersAdapter)) {
    if (!fs.existsSync(modsAdapter)) report("RUST-FEATURE-TARGET-001", "src-tauri/src/mods.rs", 1, "host mods adapter is missing.");
    if (!fs.existsSync(loadersAdapter)) report("RUST-FEATURE-TARGET-001", "src-tauri/src/loaders.rs", 1, "host loaders adapter is missing.");
  } else {
    for (const issue of evaluateHostModLoaderAdapters(readText(modsAdapter), readText(loadersAdapter))) {
      report("RUST-FEATURE-HOST-ADAPTER-001", "src-tauri/src", issue.line, issue.message);
    }
  }
  const vanilla = FEATURE_CRATE_SPECS.find((spec) => spec.crateName === "flint-feature-vanilla");
  if (vanilla && fs.existsSync(path.join(REPO_ROOT, vanilla.source))) {
    const hostAdapter = path.join(REPO_ROOT, "src-tauri/src/install.rs");
    if (!fs.existsSync(hostAdapter)) {
      report("RUST-FEATURE-VANILLA-BOUNDARY-001", "src-tauri/src/install.rs", 1, "host install adapter is missing.");
    } else {
      const vanillaInstall = path.join(path.dirname(vanilla.source), "install.rs");
      if (!fs.existsSync(vanillaInstall)) {
        report("RUST-FEATURE-VANILLA-BOUNDARY-001", "src-tauri/modules/flint-feature-vanilla/src/install.rs", 1, "vanilla install feature module is missing.");
      } else {
        for (const issue of evaluateVanillaInstallBoundary(readText(vanillaInstall), readText(hostAdapter))) {
          report("RUST-FEATURE-VANILLA-BOUNDARY-001", "src-tauri/modules/flint-feature-vanilla", issue.line, issue.message);
        }
      }
    }
  }
  for (const legacy of [
    "src-tauri/src/settings.rs",
    "src-tauri/src/saves.rs",
    "src-tauri/src/auth.rs",
    "src-tauri/src/java.rs",
    "src-tauri/src/version.rs",
    "src-tauri/src/launch.rs",
  ]) {
    if (fs.existsSync(path.join(REPO_ROOT, legacy))) {
      report("RUST-FEATURE-LEGACY-001", legacy, 1, "settings/saves/accounts/vanilla/launch implementations must live only in their extracted feature crates.");
    }
  }
}
function checkLoaderProviderGate(phase, graph, report) {
  if (phase < 2) return;
  const files = requiredLoaderFiles(phase, report);
  const providersRoot = path.join(REPO_ROOT, "src-tauri/providers");
  if (fs.existsSync(providersRoot)) {
    const directories = fs
      .readdirSync(providersRoot, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name);
    for (const issue of evaluateLoaderProviderDirectoryNames(directories)) {
      report("RUST-PROVIDER-COMPOSITION-001", "src-tauri/providers", 1, issue);
    }
  }
  const targets = LOADER_PROVIDER_TARGETS.filter((target) => files.includes(target.file));
  for (const target of targets) checkProviderTarget(phase, target, graph, report);
  for (const spec of LOADER_PROVIDER_CRATE_SPECS) {
    if (!fs.existsSync(path.join(REPO_ROOT, spec.source))) {
      report("RUST-PHASE-TARGET-001", spec.source, 1, `phase ${phase} requires the exact loader provider leaf '${spec.crateName}'.`);
      continue;
    }
    const target = LOADER_PROVIDER_TARGET_BY_SOURCE.get(spec.source);
    if (target) {
      checkProviderTarget(phase, target, graph, report);
      for (const file of rustFilesUnder(path.dirname(spec.source))) {
        if (file === spec.source) continue;
        checkProviderTarget(phase, { ...target, file }, graph, report);
      }
    }
  }
  const adapterRelative = LOADER_BINDING_SOURCE;
  let adapterRecord = graph.records.find((record) => record.relative === adapterRelative);
  if (!adapterRecord) {
    const absolute = path.join(REPO_ROOT, adapterRelative);
    if (!fs.existsSync(absolute)) {
      report("RUST-PHASE-TARGET-001", adapterRelative, 1, "the sole LoaderKind-to-provider binding target is missing.");
    } else {
      adapterRecord = {
        relative: adapterRelative,
        productionText: prepareRustProductionSource(readText(absolute)),
      };
    }
  }
  if (adapterRecord) {
    const binding = findRustFunctionSpans(adapterRecord.productionText)
      .find((span) => span.name === "loader_binding");
    const sites = findLoaderKindDispatchSites(adapterRecord.productionText);
    const allowed = sites.filter((site) => binding && site.index >= binding.start && site.index < binding.end);
    const outside = sites.filter((site) => !allowed.includes(site));
    const variants = allowed.filter((site) => site.kind === "LoaderKind-variant");
    const bindingBlock = binding
      ? adapterRecord.productionText.slice(binding.start, binding.end)
      : "";
    const hasBindingFields =
      /\brequirement\s*:/.test(bindingBlock) &&
      /\bprovider_id\s*:/.test(bindingBlock) &&
      /\bdisplay_name\s*:/.test(bindingBlock);
    if (!binding || outside.length > 0 || variants.length !== 6 || !hasBindingFields) {
      report(
        "RUST-LOADER-ADAPTER-001",
        adapterRelative,
        binding?.line ?? 1,
        "expected exactly one LoaderKind-to-provider binding inside loader_binding() with six variants and no other production LoaderKind dispatch.",
      );
    }
    for (const record of graph.records) {
      if (record.relative === adapterRelative) continue;
      const reportedLines = new Set();
      for (const site of findLoaderKindDispatchSites(record.productionText)) {
        if (reportedLines.has(site.line)) continue;
        reportedLines.add(site.line);
        report("RUST-LOADER-DISPATCH-001", record.relative, site.line, `LoaderKind dispatch form '${site.kind}' is forbidden outside the sole loader feature binding.`);
      }
    }
  }
}
// ---------------------------------------------------------------------------
// Phase 4 all-workspace source and typed-port gate
// ---------------------------------------------------------------------------
const PROVIDER_TYPED_PORT_SPECS = [
  ...MOD_PROVIDER_CRATE_SPECS.map((spec) => ({ ...spec, providerKind: "mod" })),
  ...LOADER_PROVIDER_CRATE_SPECS.map((spec) => ({ ...spec, providerKind: "loader" })),
];
function checkWorkspaceCrateDirectories(report) {
  for (const group of WORKSPACE_CRATE_GROUP_SPECS) {
    const absoluteRoot = path.join(REPO_ROOT, group.root);
    if (!fs.existsSync(absoluteRoot)) {
      report("RUST-WORKSPACE-CRATE-DIR-001", group.root, 1, "required workspace crate group directory is missing.");
      continue;
    }
    const rootStat = fs.lstatSync(absoluteRoot);
    if (rootStat.isSymbolicLink()) {
      report("ARCH-SYMLINK-001", group.root, 1, "symlinked workspace crate group roots are forbidden.");
      continue;
    }
    if (!rootStat.isDirectory()) {
      report("RUST-WORKSPACE-CRATE-DIR-001", group.root, 1, "workspace crate group root is not a directory.");
      continue;
    }
    const directories = [];
    for (const entry of fs.readdirSync(absoluteRoot, { withFileTypes: true }).sort((left, right) => left.name.localeCompare(right.name))) {
      if (entry.isSymbolicLink()) {
        report("ARCH-SYMLINK-001", relFromRepo(path.join(absoluteRoot, entry.name)), 1, "symlinked workspace crate directories are forbidden.");
        continue;
      }
      if (entry.isDirectory()) directories.push(entry.name);
    }
    for (const issue of evaluateWorkspaceCrateGroupDirectories(group.root, directories)) {
      report("RUST-WORKSPACE-CRATE-DIR-001", group.root, 1, issue);
    }
  }
}
function buildWorkspaceRustSources(report) {
  checkWorkspaceCrateDirectories(report);
  const records = [];
  const availableRoots = [];
  for (const spec of WORKSPACE_CRATE_SOURCE_SPECS) {
    const absoluteRoot = path.join(REPO_ROOT, spec.sourceRoot);
    if (!fs.existsSync(absoluteRoot)) continue;
    const rootStat = fs.lstatSync(absoluteRoot);
    if (rootStat.isSymbolicLink()) {
      report("ARCH-SYMLINK-001", spec.sourceRoot, 1, "symlinked workspace source roots are forbidden.");
      continue;
    }
    if (!rootStat.isDirectory()) {
      report("RUST-WORKSPACE-SOURCE-001", spec.sourceRoot, 1, "workspace crate source root is not a directory.");
      continue;
    }
    availableRoots.push(spec.sourceRoot);
    const sourceFiles = walkFilesFailClosed(absoluteRoot, new Set([".rs"]), report);
    if (sourceFiles.length === 0) {
      report("RUST-WORKSPACE-SOURCE-001", spec.sourceRoot, 1, `workspace crate '${spec.crateName}' has no production Rust sources.`);
    }
    const extraFiles = [];
    for (const extraSource of spec.extraSources) {
      const absolute = path.join(REPO_ROOT, extraSource);
      if (!fs.existsSync(absolute)) {
        report("RUST-WORKSPACE-SOURCE-001", extraSource, 1, `workspace crate '${spec.crateName}' is missing required production Rust source.`);
        continue;
      }
      const extraStat = fs.lstatSync(absolute);
      if (extraStat.isSymbolicLink()) {
        report("ARCH-SYMLINK-001", extraSource, 1, "symlinked workspace source files are forbidden.");
        continue;
      }
      if (!extraStat.isFile()) {
        report("RUST-WORKSPACE-SOURCE-001", extraSource, 1, "workspace extra source is not a regular file.");
        continue;
      }
      extraFiles.push(absolute);
    }
    for (const absolute of [...sourceFiles, ...extraFiles].sort((left, right) => left.localeCompare(right))) {
      const relative = relFromRepo(absolute);
      let productionText;
      try {
        productionText = prepareRustProductionSource(readText(absolute));
      } catch (error) {
        report("RUST-LEX-001", relative, 1, `cannot statically mask Rust source: ${error.message}`);
        continue;
      }
      records.push({ crateName: spec.crateName, layer: spec.layer, relative, productionText });
    }
  }
  for (const issue of evaluateWorkspaceSourceRoots(availableRoots)) {
    report("RUST-WORKSPACE-SOURCE-001", "src-tauri", 1, issue);
  }
  return records;
}
function checkFoundationWorkspaceSources(records, report) {
  for (const record of records) {
    if (!["flint-core", "flint-feature-api", "flint-provider-api"].includes(record.crateName)) continue;
    for (const issue of evaluateFoundationCrateSource(record.crateName, record.productionText)) {
      report(issue.rule, record.relative, issue.line, issue.message);
    }
  }
}
function checkProviderTypedPortGate(records, report) {
  for (const spec of PROVIDER_TYPED_PORT_SPECS) {
    const crateRecords = records.filter((record) => record.crateName === spec.crateName);
    if (crateRecords.length === 0) {
      report("RUST-PHASE-TARGET-001", spec.source, 1, `phase 4 requires loader/mod provider source for '${spec.crateName}'.`);
      continue;
    }
    for (const record of crateRecords) {
      const requireContract = record.relative === spec.source;
      for (const issue of evaluateProviderTypedPortSource(spec.providerKind, record.productionText, { requireContract })) {
        report(issue.rule, record.relative, issue.line, issue.message);
      }
    }
  }
}
function checkFeatureProviderRegistryGate(records, report) {
  for (const featureName of FEATURE_REGISTRY_CONSUMER_SPECS.keys()) {
    const spec = FEATURE_CRATE_SPECS.find((candidate) => candidate.crateName === featureName);
    const record = spec ? records.find((candidate) => candidate.relative === spec.source) : null;
    if (!record) {
      report("RUST-PHASE-TARGET-001", spec?.source ?? `src-tauri/modules/${featureName}/src/lib.rs`, 1, `phase 4 requires provider-registry consumer source for '${featureName}'.`);
      continue;
    }
    for (const issue of evaluateFeatureProviderRegistryConsumer(featureName, record.productionText)) {
      report(issue.rule, record.relative, issue.line, issue.message);
    }
  }
}
function checkPhase4WorkspaceSourceGate(records, report) {
  if (records.length === 0) {
    report("RUST-WORKSPACE-SOURCE-001", "src-tauri", 1, "phase 4 did not discover any workspace production Rust sources.");
    return;
  }
  checkFoundationWorkspaceSources(records, report);
  checkProviderTypedPortGate(records, report);
  checkFeatureProviderRegistryGate(records, report);
}
// ---------------------------------------------------------------------------
// Phase 4 Cargo workspace metadata gate
// ---------------------------------------------------------------------------
const FEATURE_CRATES = ["flint-feature-accounts", "flint-feature-launch", "flint-feature-vanilla", "flint-feature-loaders", "flint-feature-mods", "flint-feature-saves", "flint-feature-settings"];
const PROVIDER_CRATES = ["flint-mod-provider-curseforge", "flint-mod-provider-modrinth", "flint-loader-provider-fabric", "flint-loader-provider-quilt", "flint-loader-provider-forge", "flint-loader-provider-neoforge", "flint-loader-provider-liteloader", "flint-loader-provider-optifine"];
const WORKSPACE_CRATE_NAMES = ["flint", "flint-core", "flint-feature-api", "flint-provider-api", ...FEATURE_CRATES, ...PROVIDER_CRATES];
const CROSS_FEATURE_PUBLIC_EDGES = new Set([]);
const EXTRACTED_FEATURE_DEPENDENCIES = new Map([
  ["flint-feature-settings", ["flint-core", "flint-feature-api"]],
  ["flint-feature-saves", ["flint-core", "flint-feature-api"]],
  ["flint-feature-accounts", ["flint-core", "flint-feature-api", "flint-feature-settings"]],
  ["flint-feature-launch", ["flint-core", "flint-feature-api", "flint-feature-accounts", "flint-feature-vanilla", "flint-feature-settings"]],
  ["flint-feature-mods", ["flint-core", "flint-feature-api", "flint-feature-settings", "flint-provider-api"]],
  ["flint-feature-loaders", ["flint-core", "flint-feature-api", "flint-feature-vanilla", "flint-provider-api"]],
  ["flint-feature-vanilla", ["flint-core", "flint-feature-api"]],
]);
const CARGO_RULES = new Map([
  ["flint", { layer: "host", dependencies: new Set(WORKSPACE_CRATE_NAMES) }],
  ["flint-core", { layer: "core", dependencies: new Set() }],
  ["flint-feature-api", { layer: "feature-api", dependencies: new Set(["flint-core"]) }],
  ["flint-provider-api", { layer: "provider-api", dependencies: new Set(["flint-core"]) }],
  ...FEATURE_CRATES.map((name) => [name, {
    layer: "feature",
    dependencies: new Set(
      EXTRACTED_FEATURE_DEPENDENCIES.get(name)
        ?? ["flint-core", "flint-feature-api", "flint-provider-api"],
    ),
  }]),
  ...PROVIDER_CRATES.map((name) => [name, {
    layer: "provider",
    dependencies: new Set(["flint-core", "flint-provider-api"]),
  }]),
]);
const FOUNDATION_WORKSPACE_CRATES = ["flint-core", "flint-feature-api", "flint-provider-api"];
const WORKSPACE_CRATE_SOURCE_SPECS = Object.freeze([
  {
    crateName: "flint",
    layer: "host",
    manifest: "src-tauri/Cargo.toml",
    crateRoot: "src-tauri",
    sourceRoot: "src-tauri/src",
    extraSources: ["src-tauri/build.rs"],
  },
  ...FOUNDATION_WORKSPACE_CRATES.map((crateName) => ({
    crateName,
    layer: crateName === "flint-core" ? "core" : crateName === "flint-feature-api" ? "feature-api" : "provider-api",
    manifest: `src-tauri/crates/${crateName}/Cargo.toml`,
    crateRoot: `src-tauri/crates/${crateName}`,
    sourceRoot: `src-tauri/crates/${crateName}/src`,
    extraSources: [],
  })),
  ...FEATURE_CRATE_SPECS.map((spec) => ({
    crateName: spec.crateName,
    layer: "feature",
    manifest: spec.manifest,
    crateRoot: path.posix.dirname(path.posix.dirname(spec.source)),
    sourceRoot: path.posix.dirname(spec.source),
    extraSources: [],
  })),
  ...MOD_PROVIDER_CRATE_SPECS.map((spec) => ({
    crateName: spec.crateName,
    layer: "provider",
    manifest: spec.manifest,
    crateRoot: path.posix.dirname(path.posix.dirname(spec.source)),
    sourceRoot: path.posix.dirname(spec.source),
    extraSources: [],
  })),
  ...LOADER_PROVIDER_CRATE_SPECS.map((spec) => ({
    crateName: spec.crateName,
    layer: "provider",
    manifest: spec.manifest,
    crateRoot: path.posix.dirname(path.posix.dirname(spec.source)),
    sourceRoot: path.posix.dirname(spec.source),
    extraSources: [],
  })),
]);
const WORKSPACE_CRATE_SOURCE_SPEC_BY_NAME = new Map(
  WORKSPACE_CRATE_SOURCE_SPECS.map((spec) => [spec.crateName, spec]),
);
const WORKSPACE_CRATE_GROUP_SPECS = Object.freeze([
  {
    root: "src-tauri/crates",
    expectedDirectories: [...FOUNDATION_WORKSPACE_CRATES].sort(),
  },
  {
    root: "src-tauri/modules",
    expectedDirectories: [...FEATURE_CRATES].sort(),
  },
  {
    root: "src-tauri/providers",
    expectedDirectories: [...PROVIDER_CRATES].sort(),
  },
]);
export function reviewedWorkspaceCrateNames() {
  return [...WORKSPACE_CRATE_NAMES].sort();
}
export function reviewedWorkspaceCrateSpecs() {
  return WORKSPACE_CRATE_SOURCE_SPECS
    .map((spec) => ({
      crateName: spec.crateName,
      layer: spec.layer,
      manifest: spec.manifest,
      crateRoot: spec.crateRoot,
      sourceRoot: spec.sourceRoot,
      extraSources: [...spec.extraSources].sort(),
    }))
    .sort((left, right) => left.crateName.localeCompare(right.crateName));
}
export function reviewedWorkspaceCrateGroupDirectories() {
  return WORKSPACE_CRATE_GROUP_SPECS.map((group) => ({
    root: group.root,
    expectedDirectories: [...group.expectedDirectories].sort(),
  }));
}
export function evaluateWorkspaceSourceRoots(availableRoots) {
  const available = new Set(Array.from(availableRoots ?? [], toPosix));
  const issues = [];
  for (const spec of WORKSPACE_CRATE_SOURCE_SPECS) {
    if (!available.has(spec.sourceRoot)) {
      issues.push(`missing workspace source root '${spec.sourceRoot}' for crate '${spec.crateName}'.`);
    }
  }
  return issues;
}
export function evaluateWorkspaceCrateGroupDirectories(relativeRoot, directoryNames) {
  const group = WORKSPACE_CRATE_GROUP_SPECS.find((candidate) => candidate.root === toPosix(relativeRoot));
  if (!group) return [`unknown workspace crate group '${relativeRoot}'.`];
  const expected = new Set(group.expectedDirectories);
  const actual = new Set(directoryNames ?? []);
  const issues = [];
  for (const name of [...expected].sort()) {
    if (!actual.has(name)) issues.push(`missing workspace crate directory '${name}' under '${group.root}'.`);
  }
  for (const name of [...actual].sort()) {
    if (!expected.has(name)) issues.push(`unexpected workspace crate directory '${name}' under '${group.root}'.`);
  }
  return issues;
}
export function evaluateWorkspacePackageNames(packageNames) {
  const expected = new Set(WORKSPACE_CRATE_NAMES);
  const actual = new Set();
  const issues = [];
  for (const name of packageNames ?? []) {
    if (typeof name !== "string" || !name) {
      issues.push({ rule: "RUST-CARGO-WORKSPACE-001", message: "cargo metadata contains a workspace package with no valid name." });
      continue;
    }
    if (actual.has(name)) {
      issues.push({ rule: "RUST-CARGO-WORKSPACE-001", message: `workspace package '${name}' appears more than once.` });
      continue;
    }
    actual.add(name);
    if (!expected.has(name)) {
      issues.push({ rule: "RUST-CARGO-CRATE-ALLOW-001", message: `workspace package '${name}' is not in the exact reviewed crate allowlist.` });
    }
  }
  for (const name of WORKSPACE_CRATE_NAMES) {
    if (!actual.has(name)) {
      issues.push({ rule: "RUST-CARGO-WORKSPACE-001", message: `reviewed workspace package '${name}' is missing from cargo metadata.` });
    }
  }
  return issues;
}
const INTERNAL_CRATE_NAME_SET = new Set(WORKSPACE_CRATE_NAMES);
const FORBIDDEN_PROVIDER_EXTERNAL_DEPENDENCIES = new Set(["reqwest", "zip", "tokio", "process"]);
function externalDependencyRule(normal = [], build = [], dev = []) {
  const result = new Map();
  const add = (name, kind) => {
    if (!result.has(name)) result.set(name, new Set());
    result.get(name).add(kind);
  };
  for (const name of normal) add(name, "normal");
  for (const name of build) add(name, "build");
  for (const name of dev) add(name, "dev");
  return result;
}
const FEATURE_EXTERNAL_NORMAL_DEPENDENCIES = new Map(
  FEATURE_CRATE_SPECS.map((spec) => [
    spec.crateName,
    spec.dependencies.filter((dependency) => !INTERNAL_CRATE_NAME_SET.has(dependency)),
  ]),
);
const MOD_PROVIDER_EXTERNAL_NORMAL_DEPENDENCIES = new Map(
  MOD_PROVIDER_CRATE_SPECS.map((spec) => [
    spec.crateName,
    spec.dependencies.filter((dependency) => !INTERNAL_CRATE_NAME_SET.has(dependency)),
  ]),
);
const LOADER_PROVIDER_EXTERNAL_NORMAL_DEPENDENCIES = new Map(
  LOADER_PROVIDER_CRATE_SPECS.map((spec) => [
    spec.crateName,
    spec.dependencies.filter((dependency) => !INTERNAL_CRATE_NAME_SET.has(dependency)),
  ]),
);
const EXTERNAL_DEPENDENCY_RULES = new Map([
  ["flint", externalDependencyRule(
    ["tauri", "tauri-plugin-dialog", "tauri-plugin-opener", "semver", "serde", "serde_json", "dirs", "reqwest", "zip", "sha1", "regex", "md5", "flate2", "base64", "tokio"],
    ["tauri-build"],
    [],
  )],
  ["flint-core", externalDependencyRule(["dirs", "reqwest", "regex", "serde_json", "sha1"])],
  ["flint-feature-api", externalDependencyRule(["semver", "serde"], [], ["serde_json"])],
  ["flint-provider-api", externalDependencyRule(["semver"])],
  ...FEATURE_CRATE_SPECS.map((spec) => [
    spec.crateName,
    externalDependencyRule(FEATURE_EXTERNAL_NORMAL_DEPENDENCIES.get(spec.crateName) ?? []),
  ]),
  ...MOD_PROVIDER_CRATE_SPECS.map((spec) => [
    spec.crateName,
    externalDependencyRule(MOD_PROVIDER_EXTERNAL_NORMAL_DEPENDENCIES.get(spec.crateName) ?? []),
  ]),
  ...LOADER_PROVIDER_CRATE_SPECS.map((spec) => [
    spec.crateName,
    externalDependencyRule(LOADER_PROVIDER_EXTERNAL_NORMAL_DEPENDENCIES.get(spec.crateName) ?? []),
  ]),
]);
export function reviewedExternalDependencyRules() {
  return [...EXTERNAL_DEPENDENCY_RULES.entries()]
    .map(([packageName, dependencies]) => ({
      packageName,
      dependencies: [...dependencies.entries()]
        .map(([name, kinds]) => ({ name, kinds: [...kinds].sort() }))
        .sort((left, right) => left.name.localeCompare(right.name)),
    }))
    .sort((left, right) => left.packageName.localeCompare(right.packageName));
}
function normalizeCargoDependencyKind(kind) {
  return kind === undefined || kind === null ? "normal" : String(kind);
}
export function evaluateExternalDependencyAllowlist(packageName, dependencies) {
  const allowed = EXTERNAL_DEPENDENCY_RULES.get(packageName);
  if (!allowed) {
    return [{
      rule: "RUST-CARGO-CRATE-ALLOW-001",
      message: `package '${packageName}' has no reviewed external dependency allowlist.`,
    }];
  }
  if (!Array.isArray(dependencies)) {
    return [{
      rule: "RUST-CARGO-METADATA-001",
      message: `package '${packageName}' cargo metadata dependencies are not an array.`,
    }];
  }
  const issues = [];
  const observed = new Map();
  const providerLayer = CARGO_RULES.get(packageName)?.layer === "provider";
  for (const dependency of dependencies) {
    if (!dependency || typeof dependency.name !== "string" || !dependency.name.trim()) {
      issues.push({
        rule: "RUST-CARGO-METADATA-001",
        message: `package '${packageName}' has a cargo dependency without a valid package name.`,
      });
      continue;
    }
    const dependencyName = dependency.name.trim();
    if (INTERNAL_CRATE_NAME_SET.has(dependencyName)) continue;
    const dependencyKind = normalizeCargoDependencyKind(dependency.kind);
    const allowedKinds = allowed.get(dependencyName);
    if (!allowedKinds?.has(dependencyKind)) {
      const providerForbidden = providerLayer && FORBIDDEN_PROVIDER_EXTERNAL_DEPENDENCIES.has(dependencyName);
      issues.push({
        rule: providerForbidden ? "RUST-CARGO-PROVIDER-EXTERNAL-001" : "RUST-CARGO-EXTERNAL-ALLOW-001",
        message: `external dependency '${dependencyName}' (${dependencyKind}) for '${packageName}' is not in the exact reviewed allowlist.`,
      });
      continue;
    }
    if (!observed.has(dependencyName)) observed.set(dependencyName, new Set());
    observed.get(dependencyName).add(dependencyKind);
  }
  for (const [dependencyName, allowedKinds] of allowed) {
    for (const dependencyKind of allowedKinds) {
      if (!observed.get(dependencyName)?.has(dependencyKind)) {
        issues.push({
          rule: "RUST-CARGO-EXTERNAL-MISSING-001",
          message: `reviewed external dependency '${dependencyName}' (${dependencyKind}) is missing from '${packageName}'.`,
        });
      }
    }
  }
  return issues;
}
export function evaluateCargoInternalEdge(sourceName, dependencyName) {
  const sourceRule = CARGO_RULES.get(sourceName);
  if (!sourceRule) {
    return {
      allowed: false,
      rule: "RUST-CARGO-CRATE-ALLOW-001",
      message: `workspace crate '${sourceName}' is not in the exact reviewed crate allowlist.`,
    };
  }
  const crossFeature =
    sourceRule.layer === "feature" &&
    FEATURE_CRATES.includes(dependencyName) &&
    CROSS_FEATURE_PUBLIC_EDGES.has(`${sourceName}->${dependencyName}`);
  if (sourceRule.dependencies.has(dependencyName) || crossFeature) {
    return { allowed: true, rule: null, message: null };
  }
  const rule = sourceRule.layer === "provider"
    ? "RUST-CARGO-PROVIDER-EDGE-001"
    : sourceRule.layer === "feature" && FEATURE_CRATES.includes(dependencyName)
      ? "RUST-CARGO-CROSS-FEATURE-001"
      : sourceRule.layer !== "host" && dependencyName === "flint"
        ? "RUST-CARGO-HOST-REVERSE-001"
        : "RUST-CARGO-DEP-ALLOW-001";
  return {
    allowed: false,
    rule,
    message: `unreviewed workspace edge '${sourceName}' -> '${dependencyName}'.`,
  };
}
function findLockfiles(root, report) {
  const lockfiles = [];
  const stack = [root];
  while (stack.length > 0) {
    const directory = stack.pop();
    if (!fs.existsSync(directory)) continue;
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      if (entry.isSymbolicLink()) {
        report("ARCH-SYMLINK-001", relFromRepo(path.join(directory, entry.name)), 1, "symlinks are forbidden in the Cargo workspace tree.");
        continue;
      }
      if (entry.isDirectory()) {
        if (!["target", ".git", "node_modules"].includes(entry.name)) stack.push(path.join(directory, entry.name));
      } else if (entry.name === "Cargo.lock") {
        lockfiles.push(path.join(directory, entry.name));
      }
    }
  }
  return lockfiles;
}
export function findCargoDependencyCycles(graph) {
  const cycles = [];
  const state = new Map();
  const stack = [];
  function visit(name) {
    state.set(name, 1);
    stack.push(name);
    for (const dependency of graph.get(name) ?? []) {
      if (!graph.has(dependency)) continue;
      if (state.get(dependency) === 1) {
        const start = stack.indexOf(dependency);
        cycles.push([...stack.slice(start), dependency]);
      } else if (state.get(dependency) !== 2) {
        visit(dependency);
      }
    }
    stack.pop();
    state.set(name, 2);
  }
  for (const name of graph.keys()) if (!state.has(name)) visit(name);
  return cycles;
}
function checkCargoDependencyCycles(graph, packages, report) {
  for (const cycle of findCargoDependencyCycles(graph)) {
    report("RUST-CARGO-CYCLE-001", relFromRepo(packages.get(cycle[0])?.manifest_path ?? RUST_MANIFEST), 1, `Cargo dependency cycle: ${cycle.join(" -> ")}`);
  }
}
function manifestSectionText(manifestText, sectionName) {
  const lines = manifestText.split(/\r?\n/);
  const sectionLines = [];
  let found = false;
  let active = false;
  for (const line of lines) {
    const header = /^\s*\[([^\]]+)\]\s*(?:#.*)?$/.exec(line);
    if (header) {
      active = header[1].trim() === sectionName;
      if (active) found = true;
      continue;
    }
    if (active) sectionLines.push(line);
  }
  return found ? sectionLines.join("\n") : null;
}
function manifestSectionStringValue(manifestText, sectionName, key) {
  const section = manifestSectionText(manifestText, sectionName);
  if (section === null) return null;
  const pattern = new RegExp(`^\\s*${key}\\s*=\\s*["']([^"']+)["']\\s*(?:#.*)?$`, "m");
  return pattern.exec(section)?.[1] ?? null;
}
export function evaluateWorkspaceVersionCoherence(hostVersion, workspaceVersion, packages) {
  const issues = [];
  if (!hostVersion) issues.push({ rule: "RUST-CARGO-VERSION-001", message: "host manifest must declare a package version." });
  if (!workspaceVersion) issues.push({ rule: "RUST-CARGO-VERSION-001", message: "workspace manifest must declare [workspace.package] version." });
  if (hostVersion && workspaceVersion && hostVersion !== workspaceVersion) {
    issues.push({
      rule: "RUST-CARGO-VERSION-001",
      message: `host package version '${hostVersion}' does not match workspace package version '${workspaceVersion}'.`,
    });
  }
  if (!Array.isArray(packages)) {
    issues.push({ rule: "RUST-CARGO-METADATA-001", message: "workspace package versions are not available from cargo metadata." });
    return issues;
  }
  for (const pkg of packages) {
    if (!pkg || typeof pkg.name !== "string" || typeof pkg.version !== "string") {
      issues.push({ rule: "RUST-CARGO-METADATA-001", message: "cargo metadata contains a workspace package without a valid name/version." });
    } else if (hostVersion && pkg.version !== hostVersion) {
      issues.push({
        rule: "RUST-CARGO-VERSION-001",
        message: `workspace package '${pkg.name}' version '${pkg.version}' does not match host version '${hostVersion}'.`,
      });
    }
  }
  return issues;
}
export function evaluateWorkspaceLockEntries(lockText, expectedVersion = null) {
  let version = expectedVersion;
  const issues = [];
  if (!version) {
    const host = /\[\[package\]\]\s*\r?\nname = "flint"\s*\r?\nversion = "([^"]+)"/.exec(lockText);
    if (!host) return [{ rule: "RUST-CARGO-LOCK-001", message: "Cargo.lock must contain the host package 'flint' with its workspace version." }];
    version = host[1];
  }
  for (const name of WORKSPACE_CRATE_NAMES) {
    const pattern = new RegExp(`\\[\\[package\\]\\]\\r?\\nname = "${name}"\\r?\\nversion = "([^"]+)"`, "g");
    const matches = [...lockText.matchAll(pattern)];
    if (matches.length === 0) {
      issues.push({ rule: "RUST-CARGO-LOCK-001", message: `Cargo.lock must contain local package '${name}'.` });
    } else if (matches.length > 1) {
      issues.push({ rule: "RUST-CARGO-LOCK-001", message: `Cargo.lock contains duplicate local package entry '${name}'.` });
    }
    for (const match of matches) {
      if (match[1] !== version) {
        issues.push({
          rule: "RUST-CARGO-LOCK-001",
          message: `Cargo.lock package '${name}' version '${match[1]}' does not match workspace version '${version}'.`,
        });
      }
    }
  }
  return issues;
}
export function parseFoundationWorkspaceManifest(manifestText) {
  const section = manifestSectionText(manifestText, "workspace");
  if (section === null) return { present: false, resolver: null, members: null };
  const resolverMatch = /^\s*resolver\s*=\s*("([^"]*)"|'([^']*)')\s*(?:#.*)?$/m.exec(section);
  const membersMatch = /^\s*members\s*=\s*\[([\s\S]*?)\]\s*(?:#.*)?$/m.exec(section);
  let members = null;
  if (membersMatch) {
    const rawMembers = membersMatch[1];
    const quotedMembers = [...rawMembers.matchAll(/"((?:[^"\\]|\\.)*)"|'([^']*)'/g)];
    if (quotedMembers.every((match) => !(match[1] ?? match[2]).includes("\\"))) {
      const remainder = rawMembers
        .replace(/"(?:[^"\\]|\\.)*"|'[^']*'/g, "")
        .replace(/,/g, "")
        .replace(/\s+/g, "");
      if (remainder === "") members = quotedMembers.map((match) => match[1] ?? match[2]);
    }
  }
  return {
    present: true,
    resolver: resolverMatch ? (resolverMatch[2] ?? resolverMatch[3]) : null,
    members,
  };
}
function checkFoundationWorkspaceManifest(manifestText, report) {
  const foundation = parseFoundationWorkspaceManifest(manifestText);
  if (!foundation.present) return;
  if (foundation.resolver !== "2") {
    report("RUST-CARGO-WORKSPACE-001", "src-tauri/Cargo.toml", 1, 'the reviewed workspace foundation must declare resolver = "2".');
  }
  const expectedMembers = JSON.stringify(FOUNDATION_WORKSPACE_MEMBERS);
  if (foundation.members === null || JSON.stringify(foundation.members) !== expectedMembers) {
    report(
      "RUST-CARGO-WORKSPACE-001",
      "src-tauri/Cargo.toml",
      1,
      `the reviewed workspace foundation members must be exactly ${JSON.stringify(FOUNDATION_WORKSPACE_MEMBERS)}.`,
    );
  }
  for (const issue of evaluateHostLoaderDependencyDeclarations(manifestText)) {
    report("RUST-CARGO-LOADER-DEP-001", "src-tauri/Cargo.toml", issue.line, issue.message);
  }
  for (const issue of evaluateHostFeatureDependencyDeclarations(manifestText)) {
    report("RUST-CARGO-FEATURE-DEP-001", "src-tauri/Cargo.toml", issue.line, issue.message);
  }
}
function checkCargoGate(phase, report) {
  if (!fs.existsSync(RUST_MANIFEST)) {
    report("RUST-CARGO-MANIFEST-001", "src-tauri/Cargo.toml", 1, "required Cargo manifest is missing.");
    return;
  }
  const manifestText = readText(RUST_MANIFEST);
  const hasWorkspace = /^\s*\[workspace\]\s*(?:#.*)?$/m.test(manifestText);
  if (hasWorkspace) checkFoundationWorkspaceManifest(manifestText, report);
  if (phase < 4) {
    const lockRelative = "src-tauri/Cargo.lock";
    const lockPath = path.join(REPO_ROOT, lockRelative);
    if (!fs.existsSync(lockPath)) {
      report("RUST-CARGO-LOCK-001", lockRelative, 1, "the single workspace lockfile is missing.");
    } else {
      for (const issue of evaluateLoaderProviderLockEntries(readText(lockPath))) {
        report("RUST-CARGO-LOCK-001", lockRelative, issue.line, issue.message);
      }
      for (const issue of evaluateFeatureLockEntries(readText(lockPath))) {
        report("RUST-CARGO-LOCK-001", lockRelative, issue.line, issue.message);
      }
    }
    return;
  }
  if (!hasWorkspace) {
    report("RUST-CARGO-WORKSPACE-001", "src-tauri/Cargo.toml", 1, `phase ${phase} requires src-tauri/Cargo.toml to be the reviewed workspace root.`);
    return;
  }
  const result = spawnSync(
    "cargo",
    ["metadata", "--manifest-path", "src-tauri/Cargo.toml", "--no-deps", "--format-version", "1"],
    { cwd: REPO_ROOT, encoding: "utf8", maxBuffer: 32 * 1024 * 1024, windowsHide: true },
  );
  if (result.error || result.status !== 0) {
    report("RUST-CARGO-METADATA-001", "src-tauri/Cargo.toml", 1, `cargo metadata failed; crate checks were not claimed: ${result.error?.message ?? result.stderr?.trim() ?? result.status}`);
    return;
  }
  let metadata;
  try {
    metadata = JSON.parse(result.stdout);
  } catch (error) {
    report("RUST-CARGO-METADATA-001", "src-tauri/Cargo.toml", 1, `cargo metadata JSON is invalid: ${error.message}`);
    return;
  }
  if (!metadata.workspace_root || !samePath(metadata.workspace_root, path.join(REPO_ROOT, "src-tauri"))) {
    report("RUST-CARGO-METADATA-001", "src-tauri/Cargo.toml", 1, "cargo metadata workspace root is not src-tauri.");
  }
  const members = new Set(metadata.workspace_members ?? []);
  if (members.size !== WORKSPACE_CRATE_NAMES.length) {
    report("RUST-CARGO-WORKSPACE-001", "src-tauri/Cargo.toml", 1, `cargo metadata must expose exactly ${WORKSPACE_CRATE_NAMES.length} workspace members, found ${members.size}.`);
  }
  const packages = (metadata.packages ?? []).filter((pkg) => members.has(pkg.id));
  for (const issue of evaluateWorkspacePackageNames(packages.map((pkg) => pkg.name))) {
    report(issue.rule, "src-tauri/Cargo.toml", 1, issue.message);
  }
  const packagesByName = new Map(packages.map((pkg) => [pkg.name, pkg]));
  const graph = new Map();
  for (const pkg of packages) {
    const rule = CARGO_RULES.get(pkg.name);
    if (!rule) {
      report("RUST-CARGO-CRATE-ALLOW-001", relFromRepo(pkg.manifest_path), 1, `workspace crate '${pkg.name}' is not in the exact reviewed crate allowlist.`);
      continue;
    }
    const expectedSpec = WORKSPACE_CRATE_SOURCE_SPEC_BY_NAME.get(pkg.name);
    if (!expectedSpec || !samePath(pkg.manifest_path, path.join(REPO_ROOT, expectedSpec.manifest))) {
      report("RUST-CARGO-CRATE-ALLOW-001", relFromRepo(pkg.manifest_path), 1, `workspace crate '${pkg.name}' must use the exact reviewed manifest '${expectedSpec?.manifest ?? "<missing>"}'.`);
    }
    for (const issue of evaluateExternalDependencyAllowlist(pkg.name, pkg.dependencies ?? [])) {
      report(issue.rule, relFromRepo(pkg.manifest_path), 1, issue.message);
    }
    const internalDependencies = [];
    for (const dependency of pkg.dependencies ?? []) {
      if (INTERNAL_CRATE_NAME_SET.has(dependency.name)) {
        internalDependencies.push(dependency.name);
        const edge = evaluateCargoInternalEdge(pkg.name, dependency.name);
        if (!edge.allowed) report(edge.rule, relFromRepo(pkg.manifest_path), 1, edge.message);
      } else if (rule.layer !== "host" && /^tauri(?:[-_]|$)/i.test(dependency.name)) {
        report("RUST-CARGO-TAURI-001", relFromRepo(pkg.manifest_path), 1, `non-host crate '${pkg.name}' depends on Tauri package '${dependency.name}'.`);
      }
    }
    graph.set(pkg.name, internalDependencies);
  }
  checkCargoDependencyCycles(graph, packagesByName, report);
  const hostVersion = manifestSectionStringValue(manifestText, "package", "version");
  const workspaceVersion = manifestSectionStringValue(manifestText, "workspace.package", "version");
  for (const issue of evaluateWorkspaceVersionCoherence(hostVersion, workspaceVersion, packages)) {
    report(issue.rule, "src-tauri/Cargo.toml", 1, issue.message);
  }
  const rootLock = path.join(REPO_ROOT, "src-tauri", "Cargo.lock");
  const lockfiles = findLockfiles(path.join(REPO_ROOT, "src-tauri"), report);
  if (!fs.existsSync(rootLock)) {
    report("RUST-CARGO-LOCK-001", "src-tauri/Cargo.lock", 1, "the single workspace lockfile is missing.");
  } else {
    for (const issue of evaluateWorkspaceLockEntries(readText(rootLock), hostVersion)) {
      report(issue.rule, "src-tauri/Cargo.lock", 1, issue.message);
    }
  }
  for (const lockfile of lockfiles) {
    if (!samePath(lockfile, rootLock)) report("RUST-CARGO-LOCK-001", relFromRepo(lockfile), 1, "member-local Cargo.lock files are forbidden.");
  }
  const host = packages.find((pkg) => samePath(pkg.manifest_path, RUST_MANIFEST));
  const hostLib = host?.targets?.find((target) => target.name === "flint_lib");
  const hostLibraryKinds = new Set(["lib", "staticlib", "cdylib", "rlib"]);
  if (!host || host.name !== "flint" || !hostLib || !(hostLib.kind ?? []).some((kind) => hostLibraryKinds.has(kind))) {
    report("RUST-CARGO-HOST-001", "src-tauri/Cargo.toml", 1, "host package 'flint' with reviewed library target 'flint_lib' must remain at src-tauri/Cargo.toml.");
  }
}
// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------
function printReviewedAllowlists(phase) {
  const activeTauri = TAURI_TRANSITIONAL_WAIVERS.filter((waiver) => phase < waiver.removeByPhase);
  if (activeTauri.length === 0) {
    console.log("[allowlist] direct @tauri-apps imports: none (phase-5 strict).");
  } else {
    console.log("[allowlist] transitional direct @tauri-apps imports (exact file + exact specifier):");
    for (const waiver of activeTauri) {
      console.log(`  - ${waiver.file} :: ${waiver.specifiers.join(", ")} | owner=${waiver.owner} | remove-by=phase ${waiver.removeByPhase} | ${waiver.removal}`);
    }
  }
  console.log(`[allowlist] Rust module IDs: ${[...RUST_MODULE_ALLOWLIST].sort().join(", ")}`);
  let edgeCount = 0;
  for (const targets of RUST_MODULE_EDGE_ALLOWLIST.values()) edgeCount += targets.length;
  console.log(`[allowlist] exact reviewed Rust in-crate edges: ${edgeCount}; no directory wildcards.`);
  console.log(`[allowlist] host command gates: ${FEATURE_COMMAND_GATES.length} feature commands across ${new Set(FEATURE_COMMAND_GATES.map((gate) => gate.featureId)).size} features; ungated=${UNGATED_HOST_COMMANDS.join(",")}.`);
  for (const target of [...MOD_PROVIDER_TARGETS, ...LOADER_PROVIDER_TARGETS, ...FEATURE_LEAF_TARGETS]) {
    if (!fs.existsSync(path.join(REPO_ROOT, target.file))) continue;
    const local = [...target.localTargets].sort();
    const transitional = [...target.transitionalLocalTargets.keys()].sort();
    console.log(`[allowlist] ${target.file} (${target.role}) local={${local.join(", ")}} transitional={${transitional.join(", ")}} external={${[...target.externalRoots].sort().join(", ")}}`);
  }
  if (phase >= 4) {
    const sourceRoots = reviewedWorkspaceCrateSpecs().map((spec) => spec.sourceRoot);
    console.log(`[allowlist] phase-4 workspace source roots (${sourceRoots.length}): ${sourceRoots.join(", ")}`);
    const externalRules = reviewedExternalDependencyRules();
    console.log(`[allowlist] exact external Cargo dependency rules cover ${externalRules.length} workspace packages; all normal/build/dev kinds are kind-specific.`);
  }
}
function runRequestedPhase(phase) {
  const { violations, report } = createReporter();
  console.log(`Flint architecture check: requested phase ${phase}; fail-closed, no warn mode.`);
  if (!fs.existsSync(FRONTEND_ROOT)) report("ARCH-ROOT-001", "src", 1, "frontend source root is missing.");
  if (!fs.existsSync(RUST_SOURCE_ROOT)) report("ARCH-ROOT-001", "src-tauri/src", 1, "Rust source root is missing.");
  const workspaceSources = phase >= 4 ? buildWorkspaceRustSources(report) : [];
  const graph = buildRustGraph(report);
  checkModProviderCrateManifests(report);
  checkLoaderProviderCrateManifests(report);
  checkFeatureCrateManifests(report);
  checkProviderCompositionBoundaries(graph, report);
  for (const target of MOD_PROVIDER_TARGETS) {
    if (!fs.existsSync(path.join(REPO_ROOT, target.file))) {
      report("RUST-PHASE-TARGET-001", target.file, 1, `phase ${phase} requires the reviewed provider boundary target.`);
    } else {
      checkProviderTarget(phase, target, graph, report);
    }
  }
  checkModSourceBoundaries(graph, report);
  checkLoaderProviderGate(phase, graph, report);
  checkFeatureCrateGate(phase, graph, report);
  checkFrontend(phase, report);
  checkCargoGate(phase, report);
  if (phase >= 3) checkHostCommandGates(report);
  if (phase >= 4) checkPhase4WorkspaceSourceGate(workspaceSources, report);
  printReviewedAllowlists(phase);
  if (phase >= 3) console.log("[scope] phase-3 contribution checks are static manifest/import/registry-shape checks; runtime duplicate and navigation-target validation is not executed.");
  if (phase >= 5) {
    console.log("[scope] phase-5 strict waiver expiry is applied; Phase 5 hardening acceptance is not claimed by this invocation.");
  } else if (phase >= 4) {
    console.log("[scope] phase-4 workspace/Cargo/typed-port gate is active: cargo metadata supplies package/dependency truth, all 19 production source roots are scanned, and Phase 5 remains pending and is not evaluated.");
  } else {
    console.log(`[scope] phases ${phase + 1}-4 are not evaluated in this invocation; the requested phase still fails on any absent mandatory target. Phase 5 remains pending.`);
  }
  if (violations.length > 0) {
    violations.sort((left, right) =>
      left.file.localeCompare(right.file) ||
      left.line - right.line ||
      left.rule.localeCompare(right.rule) ||
      left.message.localeCompare(right.message),
    );
    for (const violation of violations) {
      console.error(`${violation.file}:${violation.line} [${violation.rule}] ${violation.message}`);
    }
    console.error(`Architecture check FAILED for requested phase ${phase}: ${violations.length} violation(s).`);
    return 1;
  }
  console.log(`Architecture check PASSED for requested phase ${phase}: 0 violations.`);
  return 0;
}
export function main(argv = process.argv.slice(2)) {
  try {
    return runRequestedPhase(parsePhase(argv));
  } catch (error) {
    if (error instanceof CliError) {
      console.error(`[CLI-ARG-001] ${error.message}`);
      console.error("Usage: node scripts/check-architecture.mjs [--phase <1..5>] (release gate: --phase 4; --phase 5 is reserved for future checks)");
      return 2;
    }
    console.error(`[ARCH-CHECK-001] architecture checker failed closed: ${error.message}`);
    return 1;
  }
}
const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : null;
if (invokedPath && samePath(invokedPath, fileURLToPath(import.meta.url))) {
  process.exitCode = main();
}
