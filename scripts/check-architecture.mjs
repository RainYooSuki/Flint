#!/usr/bin/env node
/**
 * Flint architecture boundary checker.
 *
 * Static, phase-aware, and fail-closed. Frontend parsing and module resolution
 * use the installed TypeScript compiler API and the repository tsconfig.
 * Rust checks are narrow source checks: comments/strings are masked first, and
 * only `#[cfg(test)] mod ... { ... }` scopes are removed. Production inline
 * modules are intentionally scanned.
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
const FOUNDATION_WORKSPACE_MEMBERS = [".", "crates/flint-core", "crates/flint-feature-api", "crates/flint-provider-api"];
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
const RUST_MODULE_ALLOWLIST = new Set(["crate", "main", "auth", "error", "feature_host", "install", "java", "launch", "loaders", "models", "mods", "mods::providers", "net", "paths", "saves", "settings", "version", "loader_api", "loader_providers", "loaders::provider_api", "loaders::providers"]);
const RUST_MODULE_EDGE_ALLOWLIST = new Map(Object.entries({
  crate: ["auth", "error", "feature_host", "install", "java", "launch", "loaders", "models", "mods", "paths", "saves", "settings", "version", "loader_api", "loader_providers"],
  main: [],
  auth: ["models", "net", "paths", "settings"],
  error: [],
  feature_host: ["loaders", "mods"],
  install: ["models", "paths", "version"],
  java: ["models"],
  launch: ["auth", "java", "models", "paths", "settings", "version"],
  loaders: ["install", "java", "models", "net", "paths", "version", "loaders::provider_api", "loaders::providers"],
  models: ["paths"],
  mods: ["models", "net", "paths", "mods::providers", "settings"],
  "mods::providers": ["models", "mods"],
  net: ["paths"],
  paths: [],
  saves: ["models", "net", "paths"],
  settings: ["models", "paths"],
  version: ["models", "paths"],
  loader_api: ["models"],
  loader_providers: ["loader_api", "loaders", "models"],
  "loaders::provider_api": ["models"],
  "loaders::providers": ["loader_api", "loaders", "models"],
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
function extractRustUseDeclarations(productionText) {
  const declarations = [];
  const pattern = /\buse\b([\s\S]*?);/g;
  let match;
  while ((match = pattern.exec(productionText)) !== null) {
    declarations.push({
      payload: match[1],
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
  "auth", "error", "feature_host", "install", "java", "launch", "net", "saves", "settings", "paths", "version",
]);
// Exact provider files/import roots. transitional entries are named and expire at phase 5.
const providerTarget = (file, role, localTargets, transitional = {}, external = ["std", "semver", "flint_core"]) => ({
  file, role, localTargets: new Set(localTargets), transitionalLocalTargets: new Map(Object.entries(transitional)), externalRoots: new Set(external),
});
const MOD_PROVIDER_TARGETS = [
  providerTarget(PROVIDER_API_SOURCE, "mod-provider-api", []),
  providerTarget("src-tauri/src/mods/providers.rs", "mod-provider-adapter", ["models"], {
    mods: { owner: "mod-provider-pilot", removeByPhase: 5, removal: "Extract shared helper leaves behind typed provider API/core ports." },
  }, ["std", "reqwest", "flint_provider_api"]),
];
const LOADER_API_FILES = [PROVIDER_API_SOURCE, "src-tauri/src/loader_api.rs", "src-tauri/src/loaders/provider_api.rs"];
const LOADER_ADAPTER_FILES = ["src-tauri/src/loader_providers.rs", "src-tauri/src/loaders/providers.rs"];
const LOADER_PROVIDER_TARGETS = [
  ...LOADER_API_FILES.map((file) => providerTarget(file, "loader-provider-api", file === PROVIDER_API_SOURCE ? [] : ["models", "provider_api"])),
  ...LOADER_ADAPTER_FILES.map((file) => providerTarget(file, "loader-provider-adapter", ["loader_api", "models"], {
    loaders: { owner: "loader-provider-pilot", removeByPhase: 5, removal: "Extract loader helper leaves behind typed loader API/core ports." },
  }, ["std", "reqwest", "flint_provider_api"])),
];
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
  [/\b(?:settings|paths)\s*::/g, "direct settings/paths module path"],
  [/\bCommand\s*::\s*new\s*\(/g, "process spawning"],
  [/\bFile\s*::\s*(?:open|create|create_new)\s*\(/g, "direct filesystem file access"],
  [/\b(?:TcpStream|TcpListener|UdpSocket)\b/g, "direct socket access"],
];
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
    const forbidden = forbiddenProviderUse(declaration.payload, declarationTargets);
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
  for (const [pattern, label] of FORBIDDEN_PROVIDER_CODE) {
    pattern.lastIndex = 0;
    let match;
    while ((match = pattern.exec(record.productionText)) !== null) {
      report("RUST-PROVIDER-IO-001", record.relative, lineOf(record.productionText, match.index), `forbidden provider code: ${label}.`);
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
// Exact shared payload-mapper functions may tag DTO struct fields with source: ModSource::Variant.
// They may not select providers; any other production ModSource:: reference fails below.
const MODSOURCE_DTO_CONSTRUCTOR_FUNCTIONS = new Set([
  "modrinth_search",
  "modrinth_project_from_search",
  "modrinth_version_from_value",
  "modrinth_dependency_from_value",
  "curseforge_search",
  "curseforge_project_from_search",
  "curseforge_version_from_value",
  "curseforge_dependency_from_value",
]);
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
function checkModSourceBoundaries(graph, report) {
  const adapterRelative = "src-tauri/src/mods/providers.rs";
  const adapterRecord = graph.records.find((record) => record.relative === adapterRelative);
  if (!adapterRecord) {
    report("RUST-PHASE-TARGET-001", adapterRelative, 1, "the single ModSource adapter target is missing.");
    return;
  }
  const functionSpans = findRustFunctionSpans(adapterRecord.productionText);
  const sourceBinding = functionSpans.find((span) => span.name === "source_binding");
  const adapterMatchSites = findMatchSourceSites(adapterRecord.productionText);
  if (!sourceBinding || adapterMatchSites.length !== 1 || adapterMatchSites[0].index < sourceBinding.start || adapterMatchSites[0].index >= sourceBinding.end) {
    report("RUST-MODSOURCE-ADAPTER-001", adapterRelative, adapterMatchSites[0]?.line ?? 1, "expected exactly one match-source adapter inside the reviewed source_binding function.");
  }
  for (const record of graph.records) {
    if (record.relative === adapterRelative) continue;
    for (const site of findModSourceDispatchSites(record.productionText)) {
      report("RUST-MODSOURCE-DISPATCH-001", record.relative, site.line, `ModSource dispatch form '${site.kind}' is forbidden outside src-tauri/src/mods/providers.rs.`);
    }
    const spans = findRustFunctionSpans(record.productionText);
    const references = [...record.productionText.matchAll(/\bModSource\s*::\s*[A-Za-z_][A-Za-z0-9_]*/g)];
    for (const reference of references) {
      const containingFunction = spans.find((span) => reference.index >= span.start && reference.index < span.end);
      const before = record.productionText.slice(Math.max(0, reference.index - 80), reference.index);
      const isReviewedDtoTag =
        containingFunction &&
        MODSOURCE_DTO_CONSTRUCTOR_FUNCTIONS.has(containingFunction.name) &&
        /source\s*:\s*(?:Some\s*\(\s*)?$/.test(before);
      if (!isReviewedDtoTag) {
        report(
          "RUST-MODSOURCE-REFERENCE-001",
          record.relative,
          lineOf(record.productionText, reference.index),
          "production ModSource:: reference outside the adapter is not an exact reviewed provider-DTO tag.",
        );
      }
    }
  }
}
export function requiredLoaderFiles(phase, report, exists = (file) => fs.existsSync(path.join(REPO_ROOT, file))) {
  if (phase < 2) return [];
  const apiFiles = LOADER_API_FILES.filter(exists);
  const adapterFiles = LOADER_ADAPTER_FILES.filter(exists);
  if (apiFiles.length === 0) {
    report("RUST-PHASE-TARGET-001", LOADER_API_FILES[0], 1, `phase ${phase} requires a reviewed loader-provider API target (${LOADER_API_FILES.join(" or ")}).`);
  }
  if (adapterFiles.length === 0) {
    report("RUST-PHASE-TARGET-001", LOADER_ADAPTER_FILES[0], 1, `phase ${phase} requires a reviewed loader-provider adapter target (${LOADER_ADAPTER_FILES.join(" or ")}).`);
  }
  if (adapterFiles.length > 1) {
    report("RUST-LOADER-ADAPTER-001", adapterFiles[1], 1, "exactly one reviewed loader-provider adapter target may exist.");
  }
  return [...apiFiles, ...adapterFiles];
}
function checkLoaderProviderGate(phase, graph, report) {
  if (phase < 2) return;
  const files = requiredLoaderFiles(phase, report);
  const targets = LOADER_PROVIDER_TARGETS.filter((target) => files.includes(target.file));
  for (const target of targets) checkProviderTarget(phase, target, graph, report);
  const adapterFiles = files.filter((file) => LOADER_ADAPTER_FILES.includes(file));
  if (adapterFiles.length === 1) {
    for (const record of graph.records) {
      if (record.relative === adapterFiles[0]) continue;
      const reportedLines = new Set();
      for (const site of findLoaderKindDispatchSites(record.productionText)) {
        if (reportedLines.has(site.line)) continue;
        reportedLines.add(site.line);
        report("RUST-LOADER-DISPATCH-001", record.relative, site.line, `LoaderKind dispatch form '${site.kind}' is forbidden outside the reviewed loader-provider adapter.`);
      }
    }
  }
}
// ---------------------------------------------------------------------------
// Phase 4 Cargo workspace metadata gate
// ---------------------------------------------------------------------------
const FEATURE_CRATES = ["flint-feature-accounts", "flint-feature-launch", "flint-feature-vanilla", "flint-feature-loaders", "flint-feature-mods", "flint-feature-saves", "flint-feature-settings"];
const PROVIDER_CRATES = ["flint-mod-provider-curseforge", "flint-mod-provider-modrinth", "flint-loader-provider-fabric", "flint-loader-provider-quilt", "flint-loader-provider-forge", "flint-loader-provider-neoforge", "flint-loader-provider-liteloader", "flint-loader-provider-optifine"];
const WORKSPACE_CRATE_NAMES = ["flint", "flint-core", "flint-feature-api", "flint-provider-api", ...FEATURE_CRATES, ...PROVIDER_CRATES];
const CROSS_FEATURE_PUBLIC_EDGES = new Set([]);
const CARGO_RULES = new Map([
  ["flint", { layer: "host", dependencies: new Set(WORKSPACE_CRATE_NAMES) }],
  ["flint-core", { layer: "core", dependencies: new Set() }],
  ["flint-feature-api", { layer: "feature-api", dependencies: new Set(["flint-core"]) }],
  ["flint-provider-api", { layer: "provider-api", dependencies: new Set(["flint-core"]) }],
  ...FEATURE_CRATES.map((name) => [name, {
    layer: "feature",
    dependencies: new Set(["flint-core", "flint-feature-api", "flint-provider-api"]),
  }]),
  ...PROVIDER_CRATES.map((name) => [name, {
    layer: "provider",
    dependencies: new Set(["flint-core", "flint-provider-api"]),
  }]),
]);
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
function checkCargoDependencyCycles(graph, packages, report) {
  const state = new Map();
  const stack = [];
  function visit(name) {
    state.set(name, 1);
    stack.push(name);
    for (const dependency of graph.get(name) ?? []) {
      if (!graph.has(dependency)) continue;
      if (state.get(dependency) === 1) {
        const start = stack.indexOf(dependency);
        const cycle = [...stack.slice(start), dependency];
        report("RUST-CARGO-CYCLE-001", relFromRepo(packages.get(dependency)?.manifest_path ?? RUST_MANIFEST), 1, `Cargo dependency cycle: ${cycle.join(" -> ")}`);
      } else if (state.get(dependency) !== 2) {
        visit(dependency);
      }
    }
    stack.pop();
    state.set(name, 2);
  }
  for (const name of graph.keys()) if (!state.has(name)) visit(name);
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
    report("RUST-CARGO-WORKSPACE-001", "src-tauri/Cargo.toml", 1, 'the phase-3 workspace foundation must declare resolver = "2".');
  }
  const expectedMembers = JSON.stringify(FOUNDATION_WORKSPACE_MEMBERS);
  if (foundation.members === null || JSON.stringify(foundation.members) !== expectedMembers) {
    report(
      "RUST-CARGO-WORKSPACE-001",
      "src-tauri/Cargo.toml",
      1,
      `the phase-3 workspace foundation members must be exactly ${JSON.stringify(FOUNDATION_WORKSPACE_MEMBERS)}.`,
    );
  }
}
function checkCargoGate(phase, report) {
  if (!fs.existsSync(RUST_MANIFEST)) {
    report("RUST-CARGO-MANIFEST-001", "src-tauri/Cargo.toml", 1, "required Cargo manifest is missing.");
    return;
  }
  const manifestText = readText(RUST_MANIFEST);
  const hasWorkspace = /^\s*\[workspace\]\s*(?:#.*)?$/m.test(manifestText);
  if (phase < 4) {
    if (hasWorkspace) checkFoundationWorkspaceManifest(manifestText, report);
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
  const packages = (metadata.packages ?? []).filter((pkg) => members.has(pkg.id));
  const names = new Set(packages.map((pkg) => pkg.name));
  const packagesByName = new Map(packages.map((pkg) => [pkg.name, pkg]));
  const graph = new Map();
  for (const pkg of packages) {
    const rule = CARGO_RULES.get(pkg.name);
    if (!rule) {
      report("RUST-CARGO-CRATE-ALLOW-001", relFromRepo(pkg.manifest_path), 1, `workspace crate '${pkg.name}' is not in the exact reviewed crate allowlist.`);
      continue;
    }
    const internalDependencies = [];
    for (const dependency of pkg.dependencies ?? []) {
      if (names.has(dependency.name)) {
        internalDependencies.push(dependency.name);
        const crossFeature =
          rule.layer === "feature" &&
          FEATURE_CRATES.includes(dependency.name) &&
          CROSS_FEATURE_PUBLIC_EDGES.has(`${pkg.name}->${dependency.name}`);
        if (!rule.dependencies.has(dependency.name) && !crossFeature) {
          const ruleId = rule.layer === "provider"
            ? "RUST-CARGO-PROVIDER-EDGE-001"
            : rule.layer === "feature" && FEATURE_CRATES.includes(dependency.name)
              ? "RUST-CARGO-CROSS-FEATURE-001"
              : rule.layer !== "host" && dependency.name === "flint"
                ? "RUST-CARGO-HOST-REVERSE-001"
                : "RUST-CARGO-DEP-ALLOW-001";
          report(ruleId, relFromRepo(pkg.manifest_path), 1, `unreviewed workspace edge '${pkg.name}' -> '${dependency.name}'.`);
        }
      } else if (rule.layer !== "host" && /^tauri(?:[-_]|$)/i.test(dependency.name)) {
        report("RUST-CARGO-TAURI-001", relFromRepo(pkg.manifest_path), 1, `non-host crate '${pkg.name}' depends on Tauri package '${dependency.name}'.`);
      }
    }
    graph.set(pkg.name, internalDependencies);
  }
  checkCargoDependencyCycles(graph, packagesByName, report);
  const rootLock = path.join(REPO_ROOT, "src-tauri", "Cargo.lock");
  const lockfiles = findLockfiles(path.join(REPO_ROOT, "src-tauri"), report);
  if (!fs.existsSync(rootLock)) report("RUST-CARGO-LOCK-001", "src-tauri/Cargo.lock", 1, "the single workspace lockfile is missing.");
  for (const lockfile of lockfiles) {
    if (!samePath(lockfile, rootLock)) report("RUST-CARGO-LOCK-001", relFromRepo(lockfile), 1, "member-local Cargo.lock files are forbidden.");
  }
  const host = packages.find((pkg) => samePath(pkg.manifest_path, RUST_MANIFEST));
  const hostLib = host?.targets?.find((target) => target.name === "flint_lib");
  if (!host || host.name !== "flint" || !hostLib || !(hostLib.kind ?? []).includes("lib")) {
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
  for (const target of [...MOD_PROVIDER_TARGETS, ...LOADER_PROVIDER_TARGETS]) {
    if (!fs.existsSync(path.join(REPO_ROOT, target.file))) continue;
    const local = [...target.localTargets].sort();
    const transitional = [...target.transitionalLocalTargets.keys()].sort();
    console.log(`[allowlist] ${target.file} (${target.role}) local={${local.join(", ")}} transitional={${transitional.join(", ")}} external={${[...target.externalRoots].sort().join(", ")}}`);
  }
}
function runRequestedPhase(phase) {
  const { violations, report } = createReporter();
  console.log(`Flint architecture check: requested phase ${phase}; fail-closed, no warn mode.`);
  if (!fs.existsSync(FRONTEND_ROOT)) report("ARCH-ROOT-001", "src", 1, "frontend source root is missing.");
  if (!fs.existsSync(RUST_SOURCE_ROOT)) report("ARCH-ROOT-001", "src-tauri/src", 1, "Rust source root is missing.");
  const graph = buildRustGraph(report);
  for (const target of MOD_PROVIDER_TARGETS) {
    if (!fs.existsSync(path.join(REPO_ROOT, target.file))) {
      report("RUST-PHASE-TARGET-001", target.file, 1, `phase ${phase} requires the reviewed provider boundary target.`);
    } else {
      checkProviderTarget(phase, target, graph, report);
    }
  }
  checkModSourceBoundaries(graph, report);
  checkLoaderProviderGate(phase, graph, report);
  checkFrontend(phase, report);
  checkCargoGate(phase, report);
  if (phase >= 3) checkHostCommandGates(report);
  printReviewedAllowlists(phase);
  if (phase >= 3) console.log("[scope] phase-3 contribution checks are static manifest/import/registry-shape checks; runtime duplicate and navigation-target validation is not executed.");
  if (phase < 5) {
    console.log(`[scope] phases ${phase + 1}-5 are not evaluated in this invocation; the requested phase still fails on any absent mandatory target.`);
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
      console.error("Usage: node scripts/check-architecture.mjs [--phase <1..5>]");
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
