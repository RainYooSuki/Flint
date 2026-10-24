#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import ts from "typescript";
import {
  classifyTauriImport,
  collectFrontendModuleReferences,
  collectHtmlInjectionSites,
  collectImportMetaMembers,
  collectUnsafeCodeSites,
  evaluateFrontendBoundary,
  evaluatePhase3Orchestration,
  findLoaderKindDispatchSites,
  findMatchSourceSites,
  firstUseRoot,
  findModSourceAliasDispatchSites,
  findModSourceDispatchSites,
  getExportedFeatureDescriptors,
  moduleTargetsFromUse,
  packageRootFromPackageId,
  parseFoundationWorkspaceManifest,
  parsePhase,
  parseTypeScriptSource,
  policyPackageRoot,
  prepareRustProductionSource,
  requiredLoaderFiles,
  resolveFrontendModule,
  reviewedHostRustModuleIds,
  reviewedProviderTargets,
} from "./check-architecture.mjs";

test("phase parsing defaults to 1 and rejects unsupported/unknown input", () => {
  assert.equal(parsePhase([]), 1);
  assert.equal(parsePhase(["--phase", "2"]), 2);
  assert.equal(parsePhase(["--phase=5"]), 5);
  assert.throws(() => parsePhase(["--phase", "0"]), /Unsupported phase/);
  assert.throws(() => parsePhase(["--warn"]), /Unknown argument/);
});

test("phase 2 fails closed when mandatory loader targets are absent", () => {
  const violations = [];
  const report = (rule, file, line, message) => violations.push({ rule, file, line, message });
  const files = requiredLoaderFiles(2, report, () => false);
  assert.deepEqual(files, []);
  assert.equal(violations.length, 2);
  assert.equal(violations.every((violation) => violation.rule === "RUST-PHASE-TARGET-001"), true);
});

test("provider API policy targets the extracted crate and drops the host-local module", () => {
  const providerApi = "src-tauri/crates/flint-provider-api/src/lib.rs";
  const loaderAdapter = "src-tauri/src/loaders/providers.rs";
  const targets = reviewedProviderTargets();
  const providerApiTargets = targets.filter((target) => target.file === providerApi);
  assert.deepEqual(
    providerApiTargets.map((target) => target.role).sort(),
    ["loader-provider-api", "mod-provider-api"],
  );
  const genericApi = providerApiTargets.find((target) => target.role === "mod-provider-api");
  assert.ok(genericApi);
  assert.deepEqual(genericApi.localTargets, []);
  assert.deepEqual(genericApi.externalRoots, ["flint_core", "semver", "std"]);
  assert.equal(targets.some((target) => target.file === "src-tauri/src/provider_api.rs"), false);
  assert.equal(reviewedHostRustModuleIds().includes("provider_api"), false);

  const modAdapter = targets.find((target) => target.role === "mod-provider-adapter");
  assert.ok(modAdapter);
  assert.deepEqual(modAdapter.localTargets, ["models"]);
  assert.deepEqual(modAdapter.externalRoots, ["flint_provider_api", "reqwest", "std"]);
  const loaderAdapterTarget = targets.find(
    (target) => target.role === "loader-provider-adapter" && target.file === loaderAdapter,
  );
  assert.ok(loaderAdapterTarget);
  assert.deepEqual(loaderAdapterTarget.localTargets, ["loader_api", "models"]);
  assert.deepEqual(loaderAdapterTarget.externalRoots, ["flint_provider_api", "reqwest", "std"]);

  const violations = [];
  const report = (rule, file, line, message) => violations.push({ rule, file, line, message });
  const files = requiredLoaderFiles(2, report, (file) => file === providerApi || file === loaderAdapter);
  assert.deepEqual(files, [providerApi, loaderAdapter]);
  assert.deepEqual(violations, []);
});

test("phase-3 workspace foundation parser recognizes the exact reviewed members", () => {
  const manifest = [
    "[package]",
    'name = "flint"',
    'version = "2026.10.1-d1cce64"',
    "",
    "[workspace]",
    'resolver = "2"',
    'members = [".", "crates/flint-core", "crates/flint-provider-api"]',
    "",
    "[dependencies]",
    'flint-core = { path = "crates/flint-core" }',
  ].join("\n");
  assert.deepEqual(parseFoundationWorkspaceManifest(manifest), {
    present: true,
    resolver: "2",
    members: [".", "crates/flint-core", "crates/flint-provider-api"],
  });
  assert.deepEqual(parseFoundationWorkspaceManifest('[package]\nname = "flint"'), {
    present: false,
    resolver: null,
    members: null,
  });
});

test("Rust use roots accept normal and leading-:: paths", () => {
  assert.equal(firstUseRoot(" semver::{Version, VersionReq}"), "semver");
  assert.equal(firstUseRoot(" ::std::collections::BTreeMap"), "std");
  assert.equal(firstUseRoot(" r#crate::models"), "crate");
});

test("Rust crate-root use paths resolve to imported modules rather than graph sentinels", () => {
  const modules = new Set(["crate", "main", "models", "mods", "mods::providers"]);
  assert.deepEqual(
    [...moduleTargetsFromUse("use crate::models::{ModSource};", "mods::providers", modules)].sort(),
    ["models"],
  );
  assert.deepEqual(
    [...moduleTargetsFromUse("use super::{helper};", "mods::providers", modules)].sort(),
    ["mods"],
  );
});

test("cfg(test) modules are stripped while production inline modules remain", () => {
  const source = `
mod production_one {
    fn read() { let _ = crate::net::read(); }
}
#[cfg(test)]
mod tests {
    use crate::settings::load_settings;
    fn test_only() { let _ = crate::fs::read_to_string("x"); }
}
mod production_two {
    fn paths() { let _ = crate::paths::data_dir(); }
}
`;
  const production = prepareRustProductionSource(source);
  assert.match(production, /mod production_one/);
  assert.match(production, /mod production_two/);
  assert.doesNotMatch(production, /mod tests/);
  assert.doesNotMatch(production, /crate::settings::load_settings/);
  assert.doesNotMatch(production, /crate::fs::read_to_string/);
  assert.match(production, /crate::net::read/);
  assert.match(production, /crate::paths::data_dir/);
});

test("TypeScript module resolution follows tsconfig aliases and rejects escapes", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "flint-architecture-"));
  const outside = path.join(os.tmpdir(), `${path.basename(root)}-outside`);
  try {
    const src = path.join(root, "src");
    const app = path.join(src, "app");
    const core = path.join(src, "core");
    fs.mkdirSync(app, { recursive: true });
    fs.mkdirSync(core, { recursive: true });
    fs.writeFileSync(path.join(core, "foo.ts"), "export const foo = 1;\n");
    fs.writeFileSync(path.join(app, "composition.ts"), "export const composition = 1;\n");
    fs.writeFileSync(path.join(root, "outside.ts"), "export const outside = 1;\n");
    fs.mkdirSync(outside, { recursive: true });
    fs.writeFileSync(path.join(outside, "secret.ts"), "export const secret = 1;\n");

    const options = {
      module: ts.ModuleKind.ESNext,
      moduleResolution: ts.ModuleResolutionKind.Bundler,
      target: ts.ScriptTarget.ES2020,
      baseUrl: root,
      paths: { "@/*": ["src/*"], "@escape/*": [`../${path.basename(outside)}/*`] },
    };
    const containing = path.join(app, "composition.ts");
    const local = resolveFrontendModule("@/core/foo", containing, options, src);
    assert.equal(local.kind, "local");
    assert.equal(path.basename(local.resolvedFile), "foo.ts");

    const missingAlias = resolveFrontendModule("@/core/missing", containing, options, src);
    assert.equal(missingAlias.kind, "unresolved-alias");

    const missingRelative = resolveFrontendModule("./missing", containing, options, src);
    assert.equal(missingRelative.kind, "unresolved");

    const escaped = resolveFrontendModule("../../outside", containing, options, src);
    assert.equal(escaped.kind, "escape");

    const aliasEscaped = resolveFrontendModule("@escape/secret", containing, options, src);
    assert.equal(aliasEscaped.kind, "escape");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(outside, { recursive: true, force: true });
  }
});

test("package roots normalize PackageId objects and safe string/malformed fallbacks", () => {
  assert.equal(
    packageRootFromPackageId({ name: "@tauri-apps/api", subModuleName: "", version: "2.5.0" }),
    "@tauri-apps/api",
  );
  assert.equal(
    packageRootFromPackageId({ name: "typescript", subModuleName: "lib", version: "5.7.2" }),
    "typescript",
  );
  assert.equal(packageRootFromPackageId("@tauri-apps/api@2.5.0"), "@tauri-apps/api");
  assert.equal(packageRootFromPackageId("typescript@5.7.2"), "typescript");
  assert.equal(packageRootFromPackageId("react/jsx-runtime"), "react");

  for (const malformed of [null, undefined, 42, Symbol("package"), {}, [], { name: "" }, { name: 42 }, "", "@scope", "@/missing-package"]) {
    assert.doesNotThrow(() => packageRootFromPackageId(malformed));
    assert.equal(packageRootFromPackageId(malformed), null);
  }
});
test("policy package roots map matching TypeScript @types resolutions without borrowing aliases", () => {
  const reactTypesRoot = packageRootFromPackageId({ name: "@types/react", subModuleName: "", version: "18.3.12" });
  assert.equal(reactTypesRoot, "@types/react");
  assert.equal(policyPackageRoot("react", reactTypesRoot), "react");
  assert.equal(policyPackageRoot("react/jsx-runtime", reactTypesRoot), "react");
  assert.equal(
    policyPackageRoot("react-dom", packageRootFromPackageId({ name: "@types/react-dom", subModuleName: "", version: "18.3.1" })),
    "react-dom",
  );
  assert.equal(
    policyPackageRoot("@scope/runtime", packageRootFromPackageId({ name: "@types/scope__runtime", subModuleName: "", version: "1.0.0" })),
    "@scope/runtime",
  );

  // Direct declaration imports must never be translated into an allowlisted runtime package.
  assert.equal(policyPackageRoot("@types/react", reactTypesRoot), "@types/react");
  assert.equal(policyPackageRoot("@types/react", "react"), "@types/react");
  assert.notEqual(policyPackageRoot("@types/react", reactTypesRoot), "react");

  // An alias must not borrow a different allowlisted runtime identity from a declaration resolution.
  assert.equal(policyPackageRoot("react", "@types/react-dom"), "@types/react-dom");
  assert.equal(policyPackageRoot("react-alias", reactTypesRoot), "@types/react");

  // Non-declaration resolutions keep the package identity reported by the resolver.
  assert.equal(policyPackageRoot("react", "scheduler"), "scheduler");
  assert.equal(policyPackageRoot("react", null), null);
});
test("frontend boundaries reject cross-feature, app shell, and shared escapes", () => {
  assert.equal(
    evaluateFrontendBoundary(1, "src/modules/about/index.tsx", "src/modules/saves/index.tsx").rule,
    "FE-FEATURE-CROSS-IMPORT-001",
  );
  assert.equal(
    evaluateFrontendBoundary(1, "src/app/App.tsx", "src/modules/about/index.tsx").rule,
    "FE-APP-MODULE-IMPORT-001",
  );
  assert.equal(
    evaluateFrontendBoundary(1, "src/core/contributions.ts", "src/app/App.tsx").rule,
    "FE-CORE-SHARED-IMPORT-001",
  );
  assert.equal(
    evaluateFrontendBoundary(1, "src/app/composition.ts", "src/modules/about/AboutView.tsx").rule,
    "FE-COMPOSITION-ENTRY-001",
  );
  assert.equal(
    evaluateFrontendBoundary(1, "src/modules/about/index.tsx", "src/core/contributions.ts"),
    null,
  );
});

test("AST collection sees static, type, re-export, dynamic, require, and invalid dynamic imports", () => {
  const source = parseTypeScriptSource(
    "sample.ts",
    [
      'import "./side";',
      'import type { T } from "./types";',
      'export * from "./reexport";',
      'const lazy = import("./lazy");',
      'const invalid = import(name);',
      'const legacy = require("./legacy");',
    ].join("\n"),
  );
  const references = collectFrontendModuleReferences(source);
  assert.deepEqual(references.map((reference) => reference.kind), [
    "static", "static", "reexport", "dynamic", "dynamic", "require",
  ]);
  assert.deepEqual(references.map((reference) => reference.valid), [true, true, true, true, false, true]);
});

test("import.meta glob members are surfaced for fail-closed policy", () => {
  const source = parseTypeScriptSource(
    "sample.ts",
    'const lazy = import.meta.glob("./pages/*.tsx");\nconst eager = import.meta.globEager("./pages/*.tsx");',
  );
  assert.deepEqual(collectImportMetaMembers(source).map((entry) => entry.member), ["glob", "globEager"]);
});

test("HTML injection writes/usages are surfaced with dynamic code evaluation", () => {
  const source = parseTypeScriptSource(
    "sample.tsx",
    [
      'element.innerHTML = "x";',
      'element.outerHTML += "x";',
      'element["srcdoc"] = "x";',
      'element.insertAdjacentHTML("beforeend", "x");',
      'document.write("x");',
      'document["write"]("x");',
      'const config = { dangerouslySetInnerHTML: { __html: "x" } };',
      'const jsx = <div dangerouslySetInnerHTML={{ __html: "x" }} />;',
      'eval("x");',
      'new Function("return 1");',
    ].join("\n"),
  );
  const sinks = new Set(collectHtmlInjectionSites(source).map((site) => site.sink));
  for (const expected of ["innerHTML", "outerHTML", "srcdoc", "insertAdjacentHTML", "document.write", "dangerouslySetInnerHTML"]) {
    assert.equal(sinks.has(expected), true, `missing sink ${expected}`);
  }
  const unsafe = new Set(collectUnsafeCodeSites(source).map((site) => site.kind));
  assert.equal(unsafe.has("eval"), true);
  assert.equal(unsafe.has("Function-constructor"), true);
});

test("safe HTML sink reads are not reported as injection", () => {
  const source = parseTypeScriptSource(
    "safe.tsx",
    [
      'const html = element.innerHTML;',
      'const outer = element.outerHTML;',
      'const frameSource = element.srcdoc;',
      'const inserter = element.insertAdjacentHTML;',
      'const writer = document.write;',
      'const prop = props.dangerouslySetInnerHTML;',
      'const bracket = element["innerHTML"];',
    ].join("\n"),
  );
  assert.deepEqual(collectHtmlInjectionSites(source), []);
});

test("Tauri waiver is exact by file/specifier and expires at phase 5", () => {
  assert.equal(classifyTauriImport(1, "src/components/TitleBar.tsx", "@tauri-apps/api/window"), null);
  assert.equal(
    classifyTauriImport(1, "src/components/NewChrome.tsx", "@tauri-apps/api/window").rule,
    "FE-TAURI-DIRECT-001",
  );
  assert.equal(
    classifyTauriImport(1, "src/components/TitleBar.tsx", "@tauri-apps/api/event").rule,
    "FE-TAURI-DIRECT-001",
  );
  assert.equal(
    classifyTauriImport(5, "src/components/TitleBar.tsx", "@tauri-apps/api/window").rule,
    "FE-TAURI-DIRECT-001",
  );
});

test("match-source detection does not require a literal ModSource arm", () => {
  assert.equal(findMatchSourceSites("match source { _ => fallback() }").length, 1);
  assert.equal(findMatchSourceSites("match selected_source.as_str() { _ => fallback() }").length, 1);
  assert.equal(findMatchSourceSites("match resource { _ => fallback() }").length, 0);
  assert.equal(findMatchSourceSites("match loader { _ => fallback() }").length, 0);
});
test("ModSource aliases, type aliases, wildcard imports, and alias dispatch are detected", () => {
  const importAlias = [
    "use crate::models::ModSource as SourceAlias;",
    "if selected == SourceAlias::Modrinth { select_provider(); }",
    "matches!(selected, SourceAlias::Curseforge);",
  ].join("\n");
  const importSites = findModSourceAliasDispatchSites(importAlias);
  assert.equal(importSites.some((site) => site.kind === "ModSource-import-alias"), true);
  assert.equal(importSites.some((site) => site.kind === "ModSource-alias-variant"), true);

  const typeAlias = [
    "type SourceType = ModSource;",
    "match selected { SourceType::Modrinth => select_provider(), _ => fallback() }",
  ].join("\n");
  const typeSites = findModSourceAliasDispatchSites(typeAlias);
  assert.equal(typeSites.some((site) => site.kind === "ModSource-type-alias"), true);
  assert.equal(typeSites.some((site) => site.kind === "ModSource-alias-variant"), true);

  const wildcard = findModSourceAliasDispatchSites("use crate::models::ModSource::*;");
  assert.equal(wildcard.some((site) => site.kind === "ModSource-variant-import"), true);

  assert.equal(findModSourceDispatchSites("match data_source { _ => fallback() }").length >= 1, true);
  assert.equal(findModSourceAliasDispatchSites("use crate::models::ModSource;").length, 0);
  assert.equal(findModSourceAliasDispatchSites("source: ModSource::Modrinth,").length, 0);
});

test("LoaderKind direct, alias, conditional, match, and matches dispatch is detected without generic loader false positives", () => {
  const positive = [
    "if selected == LoaderKind::Forge { select_provider(); }",
    "matches!(selected, LoaderKind::Fabric);",
    "match loader { LoaderKind::Quilt => select_provider(), _ => fallback() }",
    "match loader { _ => select_for_loader(registry, loader) }",
    "use crate::models::LoaderKind as KindAlias;",
    "if selected == KindAlias::Forge { select_provider(); }",
    "matches!(selected, KindAlias::Fabric);",
    "type KindAlias2 = LoaderKind;",
    "match loader { KindAlias2::Quilt => select_provider(), _ => fallback() }",
    "use crate::models::LoaderKind::*;",
  ].join("\n");
  const sites = findLoaderKindDispatchSites(positive);
  assert.equal(sites.some((site) => site.kind === "LoaderKind-variant"), true);
  assert.equal(sites.some((site) => site.kind === "LoaderKind-import-alias"), true);
  assert.equal(sites.some((site) => site.kind === "LoaderKind-type-alias"), true);
  assert.equal(sites.some((site) => site.kind === "LoaderKind-alias-variant"), true);
  assert.equal(sites.some((site) => site.kind === "LoaderKind-variant-import"), true);
  assert.equal(sites.some((site) => site.kind === "LoaderKind-provider-match"), true);

  const negative = [
    'match loader { "forge" => parse_forge(), "fabric" => parse_fabric(), _ => fallback() }',
    'match loader.trim().to_ascii_lowercase().as_str() { "forge" => parse_forge(), _ => fallback() }',
    "fn parse_input_loader(loader: &str) -> u32 { match loader { _ => 0 } }",
    "use crate::models::LoaderKind;",
    "fn label(kind: LoaderKind) -> &'static str { \"loader\" }",
  ].join("\n");
  assert.deepEqual(findLoaderKindDispatchSites(negative), []);
});
function phase3Fixture(sourceText, { descriptors = new Map(), bindings = new Map() } = {}) {
  const alpha = "src/modules/alpha/index.tsx";
  const beta = "src/modules/beta/index.tsx";
  const manifests = new Map([["alpha", alpha], ["beta", beta]]);
  const descriptorByManifest = new Map([
    [alpha, { hasDescriptor: true, names: new Set(["alphaFeature"]) }],
    [beta, { hasDescriptor: true, names: new Set(["betaFeature"]) }],
  ]);
  for (const [key, value] of descriptors) descriptorByManifest.set(key, value);
  const importBindings = new Map([
    ["buildContributionRegistry", { target: "src/core/contributions.ts", importedName: "buildContributionRegistry" }],
    ["alphaFeature", { target: alpha, importedName: "alphaFeature" }],
    ["betaFeature", { target: beta, importedName: "betaFeature" }],
  ]);
  for (const [key, value] of bindings) {
    if (value === null) importBindings.delete(key);
    else importBindings.set(key, value);
  }
  return {
    manifests,
    descriptorByManifest,
    bindings: importBindings,
    builderNames: new Set(["buildContributionRegistry"]),
    compositionFile: parseTypeScriptSource("src/app/composition.ts", sourceText),
    compositionRelative: "src/app/composition.ts",
    alpha,
    beta,
  };
}

test("phase-3 orchestration requires every manifest and exported descriptor", () => {
  const valid = phase3Fixture("buildContributionRegistry([alphaFeature, betaFeature]);");
  assert.deepEqual(evaluatePhase3Orchestration(valid), []);

  const missingImport = phase3Fixture("buildContributionRegistry([alphaFeature]);", {
    bindings: new Map([["betaFeature", null]]),
  });
  assert.equal(
    evaluatePhase3Orchestration(missingImport).some((diagnostic) => diagnostic.rule === "FE-COMPOSITION-MANIFEST-001"),
    true,
  );

  const missingDescriptor = phase3Fixture("buildContributionRegistry([alphaFeature, betaFeature]);");
  missingDescriptor.descriptorByManifest.set(missingDescriptor.alpha, { hasDescriptor: false, names: new Set(["alphaFeature"]) });
  assert.equal(
    evaluatePhase3Orchestration(missingDescriptor).some((diagnostic) => diagnostic.rule === "FE-FEATURE-DESCRIPTOR-001"),
    true,
  );
});

test("phase-3 registry orchestration requires exactly one call with one literal array argument", () => {
  const twoCalls = phase3Fixture("buildContributionRegistry([alphaFeature, betaFeature]); buildContributionRegistry([alphaFeature, betaFeature]);");
  assert.equal(
    evaluatePhase3Orchestration(twoCalls).some((diagnostic) => diagnostic.rule === "FE-COMPOSITION-REGISTRY-001"),
    true,
  );

  const twoArguments = phase3Fixture("buildContributionRegistry([alphaFeature, betaFeature], extraFeature);");
  assert.equal(
    evaluatePhase3Orchestration(twoArguments).some((diagnostic) => diagnostic.rule === "FE-COMPOSITION-REGISTRY-001"),
    true,
  );

  const nonLiteral = phase3Fixture("const features = [alphaFeature, betaFeature]; buildContributionRegistry(features);");
  assert.equal(
    evaluatePhase3Orchestration(nonLiteral).some((diagnostic) => diagnostic.rule === "FE-COMPOSITION-REGISTRY-001"),
    true,
  );
});

test("phase-3 registry coverage rejects missing, extra, and duplicate feature manifests", () => {
  const missingCoverage = phase3Fixture("buildContributionRegistry([alphaFeature]);");
  assert.equal(
    evaluatePhase3Orchestration(missingCoverage).some((diagnostic) => diagnostic.rule === "FE-COMPOSITION-FEATURE-001"),
    true,
  );

  const extraCoverage = phase3Fixture("buildContributionRegistry([alphaFeature, betaFeature, extraFeature]);", {
    bindings: new Map([["extraFeature", { target: "src/modules/extra/index.tsx", importedName: "extraFeature" }]]),
  });
  assert.equal(
    evaluatePhase3Orchestration(extraCoverage).some((diagnostic) => diagnostic.rule === "FE-COMPOSITION-FEATURE-001"),
    true,
  );

  const duplicateIdentifier = phase3Fixture("buildContributionRegistry([alphaFeature, alphaFeature, betaFeature]);");
  assert.equal(
    evaluatePhase3Orchestration(duplicateIdentifier).some((diagnostic) => diagnostic.rule === "FE-COMPOSITION-FEATURE-001"),
    true,
  );

  const duplicateManifest = phase3Fixture("buildContributionRegistry([alphaFeature, alphaAlias, betaFeature]);", {
    bindings: new Map([["alphaAlias", { target: "src/modules/alpha/index.tsx", importedName: "alphaFeature" }]]),
  });
  const duplicateDiagnostics = evaluatePhase3Orchestration(duplicateManifest);
  assert.equal(
    duplicateDiagnostics.some((diagnostic) => diagnostic.rule === "FE-COMPOSITION-FEATURE-001" && /appears more than once/.test(diagnostic.message)),
    true,
  );
});
test("feature manifests export a recognizable descriptor value", () => {
  const source = parseTypeScriptSource(
    "index.tsx",
    [
      'import type { FeatureDescriptor } from "../../core/contributions";',
      "export const exampleFeature: FeatureDescriptor = {",
      '  id: "flint.example", featureVersion: "1.0.0", requires: [],',
      '  contributions: { pages: [], navigation: [], settingsSections: [], instanceActions: [] }',
      "};",
      "export default exampleFeature;",
    ].join("\n"),
  );
  const descriptors = getExportedFeatureDescriptors(source);
  assert.equal(descriptors.hasDescriptor, true);
  assert.equal(descriptors.hasDefault, true);
  assert.equal(descriptors.names.has("exampleFeature"), true);
  assert.equal(descriptors.names.has("default"), true);
});
