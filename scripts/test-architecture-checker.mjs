#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import ts from "typescript";
import {
  auditHostCommandGates,
  cargoDeclaredDependencyNames,
  classifyTauriImport,
  collectFrontendModuleReferences,
  collectHtmlInjectionSites,
  collectImportMetaMembers,
  collectImportMetaTauriLoadingSites,
  collectUnsafeCodeSites,
  evaluateCargoInternalEdge,
  evaluateExactLoaderKindParser,
  evaluateExternalDependencyAllowlist,
  evaluateFeatureCrateManifest,
  evaluateFeatureDirectoryNames,
  evaluateFeatureLockEntries,
  evaluateFeatureProviderRegistryConsumer,
  evaluateFeatureRegistryConstruction,
  evaluateFoundationCrateSource,
  evaluateFrontendBoundary,
  evaluateHostFeatureDependencyDeclarations,
  evaluateHostLoaderDependencyDeclarations,
  evaluateHostModLoaderAdapters,
  evaluateLoaderProviderComposition,
  evaluateLoaderProviderCrateManifest,
  evaluateLoaderProviderDirectoryNames,
  evaluateLoaderProviderLockEntries,
  evaluateOverlayContributionContract,
  evaluatePhase3Orchestration,
  evaluatePhase5FrontendContractSources,
  evaluatePhase5HostServicesFactorySource,
  evaluatePhase5ProductionShimTokens,
  evaluatePhase5ProviderRuntimeSources,
  evaluatePhase5TauriAdapterImport,
  evaluateProviderTypedPortSource,
  evaluateTauriReferencePolicy,
  evaluateVanillaInstallBoundary,
  evaluateWorkspaceCrateGroupDirectories,
  evaluateWorkspaceLockEntries,
  evaluateWorkspacePackageNames,
  evaluateWorkspaceSourceRoots,
  evaluateWorkspaceVersionCoherence,
  findCargoDependencyCycles,
  findLoaderKindDispatchSites,
  findMatchSourceSites,
  findModSourceAliasDispatchSites,
  findModSourceDispatchSites,
  firstUseRoot,
  getExportedFeatureDescriptors,
  isReviewedTauriAdapterImport,
  isTauriPackageSpecifier,
  moduleTargetsFromUse,
  packageRootFromPackageId,
  parseFoundationWorkspaceManifest,
  parsePhase,
  parseTypeScriptSource,
  policyPackageRoot,
  prepareRustProductionSource,
  requiredLoaderFiles,
  resolveFrontendModule,
  reviewedExternalDependencyRules,
  reviewedFeatureCommandGates,
  reviewedFeatureCrateSpecs,
  reviewedFeatureTargets,
  reviewedHostRustModuleIds,
  reviewedLoaderProviderCrateSpecs,
  reviewedProviderTargets,
  reviewedTauriAdapterAllowlist,
  reviewedWorkspaceCrateGroupDirectories,
  reviewedWorkspaceCrateNames,
  reviewedWorkspaceCrateSpecs,
} from "./check-architecture.mjs";

function hostCommandFixture(gates = reviewedFeatureCommandGates()) {
  const gatedFunctions = gates.map((gate) => [
    "#[tauri::command]",
    `fn ${gate.command}(runtime: tauri::State<'_, feature_host::HostRuntime>) -> Result<(), String> {`,
    `    runtime.ensure_feature_ready(feature_host::${gate.constant})?;`,
    "    drop(runtime);",
    "    Ok(())",
    "}",
  ].join("\n"));
  const commands = [
    "#[tauri::command]\nfn launcher_info() -> () {}",
    "#[tauri::command]\nfn get_host_availability(runtime: tauri::State<'_, feature_host::HostRuntime>) -> () { runtime.snapshot() }",
    ...gatedFunctions,
  ];
  const handlers = ["launcher_info", "get_host_availability", ...gates.map((gate) => gate.command)];
  const featureConstants = [...new Map(gates.map((gate) => [gate.constant, gate.featureId])).entries()]
    .map(([constant, featureId]) => `pub(crate) const ${constant}: &str = "${featureId}";`);
  return {
    gates,
    source: [
      ...featureConstants,
      "",
      ...commands,
      "",
      "#[cfg_attr(mobile, tauri::mobile_entry_point)]",
      "pub fn run() {",
      "    tauri::Builder::default().invoke_handler(tauri::generate_handler![",
      `        ${handlers.join(",\n        ")}`,
      "    ]);",
      "}",
      "",
    ].join("\n"),
  };
}

test("phase parsing defaults to 1 and rejects unsupported/unknown input", () => {
  assert.equal(parsePhase([]), 1);
  assert.equal(parsePhase(["--phase", "2"]), 2);
  assert.equal(parsePhase(["--phase", "4"]), 4);
  assert.equal(parsePhase(["--phase", "5"]), 5);
  assert.equal(parsePhase(["--phase=5"]), 5);
  assert.throws(() => parsePhase(["--phase", "0"]), /Unsupported phase/);
  assert.throws(() => parsePhase(["--warn"]), /Unknown argument/);
});

test("phase 2 fails closed when mandatory loader targets are absent", () => {
  const violations = [];
  const report = (rule, file, line, message) => violations.push({ rule, file, line, message });
  const files = requiredLoaderFiles(2, report, () => false);
  assert.deepEqual(files, []);
  assert.equal(violations.length, 3);
  assert.equal(violations.every((violation) => violation.rule === "RUST-PHASE-TARGET-001"), true);
});

test("phase-5 gate covers the exact nineteen workspace crates and strict release workflow", () => {
  const names = reviewedWorkspaceCrateNames();
  assert.equal(names.length, 19);
  const specs = reviewedWorkspaceCrateSpecs();
  assert.equal(specs.length, 19);
  assert.deepEqual(specs.map((spec) => spec.crateName), names);
  assert.equal(new Set(specs.map((spec) => spec.sourceRoot)).size, 19);
  assert.equal(new Set(specs.map((spec) => spec.manifest)).size, 19);
  assert.deepEqual(evaluateWorkspacePackageNames(names), []);

  const missingPackage = evaluateWorkspacePackageNames(names.filter((name) => name !== "flint-core"));
  assert.equal(missingPackage.some((issue) => /flint-core.*missing/.test(issue.message)), true);
  const extraPackage = evaluateWorkspacePackageNames([...names, "flint-unreviewed"]);
  assert.equal(extraPackage.some((issue) => issue.rule === "RUST-CARGO-CRATE-ALLOW-001" && /flint-unreviewed/.test(issue.message)), true);

  const sourceRoots = specs.map((spec) => spec.sourceRoot);
  assert.deepEqual(evaluateWorkspaceSourceRoots(sourceRoots), []);
  const missingRoot = specs.find((spec) => spec.crateName === "flint-core").sourceRoot;
  const missingSource = evaluateWorkspaceSourceRoots(sourceRoots.filter((root) => root !== missingRoot));
  assert.equal(missingSource.length, 1);
  assert.match(missingSource[0], /flint-core/);

  const groups = reviewedWorkspaceCrateGroupDirectories();
  assert.equal(groups.length, 3);
  for (const group of groups) {
    assert.deepEqual(evaluateWorkspaceCrateGroupDirectories(group.root, group.expectedDirectories), []);
  }
  const crateGroup = groups.find((group) => group.root === "src-tauri/crates");
  assert.deepEqual(
    evaluateWorkspaceCrateGroupDirectories(crateGroup.root, [...crateGroup.expectedDirectories, "flint-unreviewed"]),
    ["unexpected workspace crate directory 'flint-unreviewed' under 'src-tauri/crates'."],
  );
  const modulesGroup = groups.find((group) => group.root === "src-tauri/modules");
  assert.deepEqual(
    evaluateWorkspaceCrateGroupDirectories(
      modulesGroup.root,
      modulesGroup.expectedDirectories.filter((name) => name !== "flint-feature-mods"),
    ),
    ["missing workspace crate directory 'flint-feature-mods' under 'src-tauri/modules'."],
  );

  const workflow = fs.readFileSync(new URL("../.github/workflows/flint-release.yml", import.meta.url), "utf8");
  const checkerPhases = [...workflow.matchAll(/node scripts\/check-architecture\.mjs\s+--phase\s+([1-5])/g)]
    .map((match) => match[1]);
  assert.deepEqual(checkerPhases, ["5"]);
  assert.match(workflow, /node --test scripts\/test-architecture-checker\.mjs scripts\/test-frontend-contracts\.mjs/);
  assert.equal(fs.existsSync(new URL("../scripts/test-frontend-contracts.mjs", import.meta.url)), true);
  const checkerSource = fs.readFileSync(new URL("../scripts/check-architecture.mjs", import.meta.url), "utf8");
  assert.match(checkerSource, /strict release gate: --phase 5/);
  assert.doesNotMatch(checkerSource, /Phase 5 remains a strict future phase|reserved for future checks/);
});

test("phase-4 external dependency allowlists are exact by package and Cargo kind", () => {
  const rules = reviewedExternalDependencyRules();
  assert.equal(rules.length, 19);
  const hostRule = rules.find((rule) => rule.packageName === "flint");
  assert.deepEqual(hostRule.dependencies.find((dependency) => dependency.name === "tauri-build").kinds, ["build"]);
  const liteloaderRule = rules.find((rule) => rule.packageName === "flint-loader-provider-liteloader");
  assert.deepEqual(
    liteloaderRule.dependencies.map((dependency) => [dependency.name, dependency.kinds]),
    [["regex", ["normal"]], ["serde_json", ["normal"]]],
  );
  const metadataDependencies = (packageName) => {
    const rule = rules.find((candidate) => candidate.packageName === packageName);
    assert.ok(rule, `missing external rule for ${packageName}`);
    return rule.dependencies.flatMap((dependency) =>
      dependency.kinds.map((kind) => ({ name: dependency.name, kind: kind === "normal" ? null : kind })),
    );
  };

  assert.deepEqual(evaluateExternalDependencyAllowlist("flint", metadataDependencies("flint")), []);
  assert.deepEqual(evaluateExternalDependencyAllowlist("flint-feature-api", metadataDependencies("flint-feature-api")), []);
  assert.deepEqual(
    evaluateExternalDependencyAllowlist("flint-mod-provider-curseforge", metadataDependencies("flint-mod-provider-curseforge")),
    [],
  );

  const hostDependencies = metadataDependencies("flint");
  const wrongBuildKind = hostDependencies.map((dependency) =>
    dependency.name === "tauri-build" ? { name: "tauri-build", kind: "normal" } : dependency,
  );
  const wrongKindIssues = evaluateExternalDependencyAllowlist("flint", wrongBuildKind);
  assert.equal(wrongKindIssues.some((issue) => issue.rule === "RUST-CARGO-EXTERNAL-ALLOW-001"), true);
  assert.equal(wrongKindIssues.some((issue) => issue.rule === "RUST-CARGO-EXTERNAL-MISSING-001"), true);

  const featureApiWithWrongKind = metadataDependencies("flint-feature-api").map((dependency) =>
    dependency.name === "serde_json" ? { name: "serde_json", kind: "normal" } : dependency,
  );
  assert.equal(
    evaluateExternalDependencyAllowlist("flint-feature-api", featureApiWithWrongKind)
      .some((issue) => issue.rule === "RUST-CARGO-EXTERNAL-ALLOW-001"),
    true,
  );

  const providerDependencies = [
    ...metadataDependencies("flint-mod-provider-curseforge"),
    { name: "reqwest", kind: "normal" },
    { name: "zip", kind: "normal" },
  ];
  const providerIssues = evaluateExternalDependencyAllowlist("flint-mod-provider-curseforge", providerDependencies);
  assert.equal(providerIssues.filter((issue) => issue.rule === "RUST-CARGO-PROVIDER-EXTERNAL-001").length, 2);
});

test("phase-4 internal Cargo edges and cycles are fail-closed", () => {
  assert.equal(evaluateCargoInternalEdge("flint", "flint-core").allowed, true);
  assert.equal(evaluateCargoInternalEdge("flint-core", "flint-feature-api").allowed, false);
  assert.equal(
    evaluateCargoInternalEdge("flint-mod-provider-curseforge", "flint-loader-provider-fabric").rule,
    "RUST-CARGO-PROVIDER-EDGE-001",
  );
  assert.equal(
    evaluateCargoInternalEdge("flint-loader-provider-fabric", "flint-feature-mods").rule,
    "RUST-CARGO-PROVIDER-EDGE-001",
  );
  assert.equal(evaluateCargoInternalEdge("flint-feature-launch", "flint-feature-accounts").allowed, true);
  assert.equal(
    evaluateCargoInternalEdge("flint-feature-launch", "flint-feature-saves").rule,
    "RUST-CARGO-CROSS-FEATURE-001",
  );
  assert.equal(
    evaluateCargoInternalEdge("flint-feature-settings", "flint").rule,
    "RUST-CARGO-HOST-REVERSE-001",
  );

  const cycles = findCargoDependencyCycles(new Map([
    ["alpha", ["beta"]],
    ["beta", ["alpha"]],
    ["leaf", []],
  ]));
  assert.deepEqual(cycles, [["alpha", "beta", "alpha"]]);
  assert.deepEqual(findCargoDependencyCycles(new Map([["alpha", ["beta"]], ["beta", []]])), []);
});

test("phase-4 workspace versions and the single lockfile stay coherent", () => {
  const version = "2026.10.1-d1cce64";
  const packages = reviewedWorkspaceCrateNames().map((name) => ({ name, version }));
  assert.deepEqual(evaluateWorkspaceVersionCoherence(version, version, packages), []);
  assert.equal(
    evaluateWorkspaceVersionCoherence(version, "2026.10.2-deadbee", packages)
      .some((issue) => issue.rule === "RUST-CARGO-VERSION-001"),
    true,
  );
  assert.equal(
    evaluateWorkspaceVersionCoherence(version, version, packages.map((pkg) =>
      pkg.name === "flint-core" ? { ...pkg, version: "0.0.0" } : pkg,
    )).some((issue) => issue.rule === "RUST-CARGO-VERSION-001"),
    true,
  );

  const lock = [
    "[[package]]",
    'name = "flint"',
    `version = "${version}"`,
    "",
    ...reviewedWorkspaceCrateNames().filter((name) => name !== "flint").flatMap((name) => [
      "[[package]]",
      `name = "${name}"`,
      `version = "${version}"`,
      "",
    ]),
  ].join("\n");
  assert.deepEqual(evaluateWorkspaceLockEntries(lock, version), []);
  const missingLockEntry = lock.replace(
    'name = "flint-core"\nversion = "2026.10.1-d1cce64"',
    'name = "flint-core"\nversion = "0.0.0"',
  );
  assert.equal(
    evaluateWorkspaceLockEntries(missingLockEntry, version)
      .some((issue) => issue.rule === "RUST-CARGO-LOCK-001"),
    true,
  );
});

test("phase-4 provider typed-port evaluator requires contracts and rejects raw host APIs", () => {
  const validLoader = [
    "use flint_core::ports::{NetworkPort, PathPort};",
    "use flint_provider_api::{LoaderListContext, LoaderProvider};",
    "impl LoaderProvider for FabricLoaderProvider {",
    "    fn list_versions(&self) -> Result<(), String> { Ok(()) }",
    "}",
    "fn fetch_metadata(context: &LoaderListContext<'_>) { let _ = context.network(); }",
  ].join("\n");
  assert.deepEqual(evaluateProviderTypedPortSource("loader", validLoader), []);

  const validMod = [
    "use flint_core::ports::NetworkPort;",
    "use flint_provider_api::{ModProvider, ModProviderContext};",
    "impl ModProvider for ModrinthModProvider {}",
    "fn fetch(context: &ModProviderContext<'_>) { let _ = context.network(); }",
  ].join("\n");
  assert.deepEqual(evaluateProviderTypedPortSource("mod", validMod), []);

  const missingContract = evaluateProviderTypedPortSource("loader", "fn helper() {}");
  assert.equal(
    missingContract.some((issue) => issue.rule === "RUST-PROVIDER-TYPED-PORT-001" && /LoaderProvider/.test(issue.message)),
    true,
  );
  assert.equal(
    missingContract.some((issue) => /typed context ports/.test(issue.message)),
    true,
  );

  const forbidden = `${validMod}
use reqwest::Client;
use std::fs::File;
use flint_feature_mods::ModSource;
use flint_loader_provider_fabric::FabricLoaderProvider;
`;
  const forbiddenIssues = evaluateProviderTypedPortSource("mod", forbidden);
  assert.equal(forbiddenIssues.some((issue) => /reqwest/.test(issue.message)), true);
  assert.equal(forbiddenIssues.some((issue) => /filesystem/.test(issue.message)), true);
  assert.equal(forbiddenIssues.some((issue) => /feature crate import/.test(issue.message)), true);
  assert.equal(forbiddenIssues.some((issue) => /provider-to-provider/.test(issue.message)), true);
});

test("phase-4 provider typed-port evaluator rejects concrete core adapters and registry construction", () => {
  const cleanLoader = [
    "use flint_core::ports::{NetworkPort, PathPort};",
    "use flint_provider_api::{LoaderListContext, LoaderProvider};",
    "impl LoaderProvider for FabricLoaderProvider {",
    "    fn list_versions(&self) -> Result<(), String> { Ok(()) }",
    "}",
    "fn fetch(context: &LoaderListContext<'_>) { let _ = context.network(); }",
  ].join("\n");
  const cleanMod = [
    "use flint_core::ports::NetworkPort;",
    "use flint_provider_api::{ModProvider, ModProviderContext};",
    "impl ModProvider for ModrinthModProvider {}",
    "fn fetch(context: &ModProviderContext<'_>) { let _ = context.network(); }",
  ].join("\n");
  assert.deepEqual(evaluateProviderTypedPortSource("loader", cleanLoader), []);
  assert.deepEqual(evaluateProviderTypedPortSource("mod", cleanMod), []);

  for (const adapter of [
    "ReqwestNetwork",
    "SafeWorkspace",
    "CommandProcessPort",
    "ProcessJavaRunner",
    "NoopLogger",
    "NoopProgress",
  ]) {
    const source = `${cleanMod}
use flint_core::ports::${adapter};
`;
    const issues = evaluateProviderTypedPortSource("mod", source);
    const adapterIssues = issues.filter((issue) => issue.rule === "RUST-PROVIDER-CORE-ADAPTER-001");
    assert.equal(adapterIssues.length, 1, `missing concrete adapter rejection for ${adapter}`);
    assert.match(adapterIssues[0].message, new RegExp(`'${adapter}'`));
  }
  assert.equal(
    evaluateProviderTypedPortSource("loader", "use flint_core::ports::SafeWorkspace;", { requireContract: false })
      .some((issue) => issue.rule === "RUST-PROVIDER-CORE-ADAPTER-001"),
    true,
  );

  const directConstruction = `${cleanLoader}
fn compose_registry() { let _ = ProviderRegistry::new(vec![], 1); }
`;
  const directIssues = evaluateProviderTypedPortSource("loader", directConstruction)
    .filter((issue) => issue.rule === "RUST-PROVIDER-REGISTRY-CONSTRUCTION-001");
  assert.equal(directIssues.length, 1);
  assert.match(directIssues[0].message, /provider_composition\.rs/);

  const genericConstruction = `${cleanMod}
fn compose_registry() { let _ = ProviderRegistry::<dyn ModProvider>::new(vec![], 1); }
`;
  assert.equal(
    evaluateProviderTypedPortSource("mod", genericConstruction)
      .filter((issue) => issue.rule === "RUST-PROVIDER-REGISTRY-CONSTRUCTION-001").length,
    1,
  );

  const testOnlySource = `${cleanMod}
#[cfg(test)]
mod tests {
    use flint_core::ports::{ReqwestNetwork, SafeWorkspace, CommandProcessPort, ProcessJavaRunner, NoopLogger, NoopProgress};
    fn compose_in_test() { let _ = ProviderRegistry::<dyn ModProvider>::new(vec![], 1); }
}
`;
  const masked = prepareRustProductionSource(testOnlySource);
  assert.equal(masked.includes("ReqwestNetwork"), false);
  assert.equal(masked.includes("ProviderRegistry"), false);
  assert.deepEqual(evaluateProviderTypedPortSource("mod", masked), []);
});
test("phase-4 feature registry consumers and foundation sources enforce direction", () => {
  const validMods = [
    "use flint_provider_api::{ModProvider, ProviderRegistry};",
    "fn search(registry: &ProviderRegistry<dyn ModProvider>) { let _ = registry; }",
  ].join("\n");
  assert.deepEqual(evaluateFeatureProviderRegistryConsumer("flint-feature-mods", validMods), []);

  const invalidMods = `${validMods}
use tauri::AppHandle;
use flint_mod_provider_modrinth::ModrinthModProvider;
fn build() { let _ = ProviderRegistry::new(vec![], 1); }
`;
  const modIssues = evaluateFeatureProviderRegistryConsumer("flint-feature-mods", invalidMods);
  assert.equal(modIssues.some((issue) => /may not construct provider registries/.test(issue.message)), true);
  assert.equal(modIssues.some((issue) => /Tauri/.test(issue.message)), true);
  assert.equal(modIssues.some((issue) => /concrete provider/.test(issue.message)), true);

  assert.deepEqual(
    evaluateFoundationCrateSource("flint-provider-api", "use flint_core::ports::NetworkPort;"),
    [],
  );
  assert.deepEqual(
    evaluateFoundationCrateSource("flint-feature-api", "use flint_core::contract::ApiVersion;"),
    [],
  );
  assert.equal(
    evaluateFoundationCrateSource("flint-feature-api", "use flint_provider_api::Provider;")
      .some((issue) => /provider API crate/.test(issue.message)),
    true,
  );
  const foundationIssues = evaluateFoundationCrateSource(
    "flint-core",
    "use tauri::AppHandle;\nuse flint_feature_mods::ModSource;\nuse flint_provider_api::Provider;",
  );
  assert.equal(foundationIssues.some((issue) => /Tauri/.test(issue.message)), true);
  assert.equal(foundationIssues.some((issue) => /feature or concrete provider/.test(issue.message)), true);
  assert.equal(foundationIssues.some((issue) => /provider or feature API/.test(issue.message)), true);
});
test("provider API policy targets the extracted crate and drops the host-local module", () => {
  const providerApi = "src-tauri/crates/flint-provider-api/src/lib.rs";
  const loaderContract = "src-tauri/crates/flint-provider-api/src/loaders.rs";
  const loaderAdapter = "src-tauri/src/loaders.rs";
  const targets = reviewedProviderTargets();
  assert.equal(targets.every((target) => !Object.hasOwn(target, "transitionalLocalTargets")), true);
  const providerApiTargets = targets.filter((target) => target.file === providerApi);
  assert.deepEqual(
    [...new Set(providerApiTargets.map((target) => target.role))].sort(),
    ["loader-provider-api", "mod-provider-api"],
  );
  const modApiTargets = targets.filter((target) => target.role === "mod-provider-api");
  assert.equal(modApiTargets.length, 2);
  const genericApi = modApiTargets.find((target) => target.file === providerApi);
  assert.ok(genericApi);
  assert.deepEqual(genericApi.localTargets, ["loaders", "mods"]);
  assert.deepEqual(genericApi.externalRoots, ["flint_core", "semver", "std"]);
  const modContract = modApiTargets.find(
    (target) => target.file === "src-tauri/crates/flint-provider-api/src/mods.rs",
  );
  assert.ok(modContract);
  assert.deepEqual(modContract.localTargets, []);
  assert.deepEqual(modContract.externalRoots, ["flint_core", "std"]);
  assert.equal(targets.some((target) => target.file === "src-tauri/src/provider_api.rs"), false);
  assert.equal(reviewedHostRustModuleIds().includes("provider_api"), false);

  const modLeaves = targets.filter((target) => target.role === "mod-provider-leaf");
  assert.deepEqual(
    modLeaves.map((target) => target.file).sort(),
    [
      "src-tauri/providers/flint-mod-provider-curseforge/src/lib.rs",
      "src-tauri/providers/flint-mod-provider-modrinth/src/lib.rs",
    ],
  );
  for (const leaf of modLeaves) {
    assert.deepEqual(leaf.localTargets, []);
    assert.deepEqual(leaf.externalRoots, ["flint_core", "flint_provider_api", "serde_json", "std"]);
  }
  assert.equal(targets.some((target) => target.file === "src-tauri/src/mods/providers.rs"), false);
  assert.equal(reviewedHostRustModuleIds().includes("mods::providers"), false);
  const loaderApiTargets = targets.filter((target) => target.role === "loader-provider-api");
  assert.deepEqual(
    loaderApiTargets.map((target) => target.file).sort(),
    [loaderContract, providerApi].sort(),
  );
  const genericLoaderApi = loaderApiTargets.find((target) => target.file === providerApi);
  assert.ok(genericLoaderApi);
  assert.deepEqual(genericLoaderApi.localTargets, ["loaders", "mods"]);
  const loaderContractTarget = targets.find(
    (target) => target.role === "loader-provider-api" && target.file === loaderContract,
  );
  assert.ok(loaderContractTarget);
  assert.deepEqual(loaderContractTarget.localTargets, []);
  assert.deepEqual(loaderContractTarget.externalRoots, ["flint_core", "std"]);
  const loaderLeaves = targets.filter((target) => target.role === "loader-provider-leaf");
  assert.equal(loaderLeaves.length, 6);
  assert.deepEqual(
    loaderLeaves.map((target) => target.file).sort(),
    reviewedLoaderProviderCrateSpecs().map((spec) => spec.source).sort(),
  );
  const loaderAdapterTarget = targets.find(
    (target) => target.role === "loader-provider-adapter" && target.file === loaderAdapter,
  );
  assert.ok(loaderAdapterTarget);
  assert.deepEqual(loaderAdapterTarget.localTargets, ["models"]);
  assert.deepEqual(loaderAdapterTarget.externalRoots, ["flint_feature_loaders", "flint_provider_api", "std", "tauri"]);
  assert.equal(targets.some((target) => target.file === "src-tauri/src/loaders/providers.rs"), false);
  assert.equal(targets.some((target) => target.file === "src-tauri/src/loader_api.rs"), false);
  assert.equal(reviewedHostRustModuleIds().includes("loader_api"), false);
  assert.equal(reviewedHostRustModuleIds().includes("loader_providers"), false);
  assert.equal(reviewedHostRustModuleIds().includes("loaders::providers"), false);

  const violations = [];
  const report = (rule, file, line, message) => violations.push({ rule, file, line, message });
  const files = requiredLoaderFiles(
    2,
    report,
    (file) => file === providerApi || file === loaderContract || file === loaderAdapter,
  );
  assert.deepEqual(files, [providerApi, loaderContract, loaderAdapter]);
  assert.deepEqual(violations, []);
  const legacyViolations = [];
  const legacyReport = (rule, file, line, message) => legacyViolations.push({ rule, file, line, message });
  requiredLoaderFiles(
    2,
    legacyReport,
    (file) => file === "src-tauri/src/loaders/providers.rs",
  );
  assert.equal(
    legacyViolations.some(
      (violation) =>
        violation.rule === "RUST-LOADER-ADAPTER-001" &&
        violation.file === "src-tauri/src/loaders/providers.rs",
    ),
    true,
  );
});

test("provider manifest dependency parser covers normal, target, and dev sections", () => {
  const manifest = [
    "[package]",
    'name = "provider"',
    "",
    "[dependencies]",
    'flint-core = { path = "../../crates/flint-core" }',
    'flint-provider-api = { path = "../../crates/flint-provider-api" }',
    'serde_json = "1"',
    "",
    "[dev-dependencies]",
    'serde_json = "1"',
    "",
    "[target.'cfg(unix)'.dependencies]",
    'semver = "1"',
    "",
  ].join("\n");
  assert.deepEqual(cargoDeclaredDependencyNames(manifest), [
    "flint-core",
    "flint-provider-api",
    "semver",
    "serde_json",
  ]);
});

test("phase-3 workspace foundation parser recognizes the exact reviewed members", () => {
  const manifest = [
    "[package]",
    'name = "flint"',
    'version = "2026.10.1-d1cce64"',
    "",
    "[workspace]",
    'resolver = "2"',
    'members = [".", "crates/flint-core", "crates/flint-feature-api", "crates/flint-provider-api", "modules/flint-feature-accounts", "modules/flint-feature-launch", "modules/flint-feature-loaders", "modules/flint-feature-mods", "modules/flint-feature-vanilla", "modules/flint-feature-settings", "modules/flint-feature-saves", "providers/flint-mod-provider-curseforge", "providers/flint-mod-provider-modrinth", "providers/flint-loader-provider-fabric", "providers/flint-loader-provider-quilt", "providers/flint-loader-provider-forge", "providers/flint-loader-provider-neoforge", "providers/flint-loader-provider-liteloader", "providers/flint-loader-provider-optifine"]',
    "",
    "[dependencies]",
    'flint-core = { path = "crates/flint-core" }',
  ].join("\n");
  const foundation = parseFoundationWorkspaceManifest(manifest);
  assert.deepEqual(foundation, {
    present: true,
    resolver: "2",
    members: [".", "crates/flint-core", "crates/flint-feature-api", "crates/flint-provider-api", "modules/flint-feature-accounts", "modules/flint-feature-launch", "modules/flint-feature-loaders", "modules/flint-feature-mods", "modules/flint-feature-vanilla", "modules/flint-feature-settings", "modules/flint-feature-saves", "providers/flint-mod-provider-curseforge", "providers/flint-mod-provider-modrinth", "providers/flint-loader-provider-fabric", "providers/flint-loader-provider-quilt", "providers/flint-loader-provider-forge", "providers/flint-loader-provider-neoforge", "providers/flint-loader-provider-liteloader", "providers/flint-loader-provider-optifine"],
  });
  assert.equal(foundation.members.includes("crates/flint-feature-api"), true);
  assert.equal(foundation.members.some((member) => member.includes("*")), false);
  assert.deepEqual(parseFoundationWorkspaceManifest('[package]\nname = "flint"'), {
    present: false,
    resolver: null,
    members: null,
  });
});

test("exact six loader provider crate manifests and host path dependencies are checked", () => {
  const specs = reviewedLoaderProviderCrateSpecs();
  assert.equal(specs.length, 6);
  assert.deepEqual(
    specs.map((spec) => spec.crateName),
    [
      "flint-loader-provider-fabric",
      "flint-loader-provider-quilt",
      "flint-loader-provider-forge",
      "flint-loader-provider-neoforge",
      "flint-loader-provider-liteloader",
      "flint-loader-provider-optifine",
    ],
  );  assert.deepEqual(evaluateLoaderProviderDirectoryNames(specs.map((spec) => spec.crateName)), []);
  assert.deepEqual(
    evaluateLoaderProviderDirectoryNames([...specs.map((spec) => spec.crateName), "flint-loader-provider-extra"]),
    ["unexpected loader provider crate directory 'flint-loader-provider-extra'."],
  );
  for (const spec of specs) {
    const manifest = [
      "[package]",
      `name = "${spec.crateName}"`,
      "version.workspace = true",
      "publish = false",
      "",
      "[dependencies]",
      ...spec.dependencies.map((dependency) => `${dependency} = "1"`),
      "",
      "[dev-dependencies]",
      'flint-provider-api = { path = "../../crates/flint-provider-api", features = ["test-ports"] }',
      "",
    ].join("\n");
    assert.deepEqual(evaluateLoaderProviderCrateManifest(spec.manifest, manifest), []);
    const withExtraDependency = manifest.replace(
      "[dev-dependencies]",
      'unexpected = "1"\n\n[dev-dependencies]',
    );
    assert.equal(
      evaluateLoaderProviderCrateManifest(spec.manifest, withExtraDependency)
        .some((issue) => issue.message.includes("dependencies must be exactly")),
      true,
    );
    const wrongVersion = manifest.replace("version.workspace = true", 'version = "1.0.0"');
    assert.equal(
      evaluateLoaderProviderCrateManifest(spec.manifest, wrongVersion)
        .some((issue) => issue.message.includes("inherit the workspace version")),
      true,
    );
  }
  const declarations = specs.map((spec) => {
    const relativePath = spec.manifest
      .replace(/^src-tauri\//, "")
      .replace(/\/Cargo\.toml$/, "");
    return `${spec.crateName} = { path = "${relativePath}" }`;
  }).join("\n");
  assert.deepEqual(evaluateHostLoaderDependencyDeclarations(declarations), []);
  assert.equal(
    evaluateHostLoaderDependencyDeclarations(declarations.replace("providers/flint-loader-provider-quilt", "providers/wrong-quilt"))
      .length,
    1,
  );
});

test("loader provider lock entries share the host workspace version", () => {
  const specs = reviewedLoaderProviderCrateSpecs();
  const lock = [
    "[[package]]",
    'name = "flint"',
    'version = "2026.10.1-d1cce64"',
    "",
    ...specs.flatMap((spec) => [
      "[[package]]",
      `name = "${spec.crateName}"`,
      'version = "2026.10.1-d1cce64"',
      "",
    ]),
  ].join("\n");
  assert.deepEqual(evaluateLoaderProviderLockEntries(lock), []);
  assert.equal(
    evaluateLoaderProviderLockEntries(
      lock.replace(
        'name = "flint-loader-provider-quilt"\nversion = "2026.10.1-d1cce64"',
        'name = "flint-loader-provider-quilt"\nversion = "0.0.0"',
      ),
    ).length,
    1,
  );
});

test("feature production code cannot construct provider registries", () => {
  assert.deepEqual(evaluateFeatureRegistryConstruction("fn ok() { }"), []);
  const issues = evaluateFeatureRegistryConstruction("let registry = ProviderRegistry::new(providers, MOD_PROVIDER_API).unwrap();");
  assert.equal(issues.length, 1);
  assert.equal(issues[0].line, 1);
});

test("extracted feature waves use exact manifests, edges, directories, and locks", () => {
  const specs = reviewedFeatureCrateSpecs();
  assert.equal(specs.length, 7);
  assert.deepEqual(
    specs.map((spec) => spec.crateName),
    ["flint-feature-settings", "flint-feature-saves", "flint-feature-accounts", "flint-feature-vanilla", "flint-feature-launch", "flint-feature-mods", "flint-feature-loaders"],
  );
  assert.deepEqual(
    specs.find((spec) => spec.crateName === "flint-feature-settings").dependencies,
    ["flint-core", "flint-feature-api", "serde", "serde_json"],
  );
  assert.deepEqual(
    specs.find((spec) => spec.crateName === "flint-feature-saves").dependencies,
    ["base64", "flate2", "flint-core", "flint-feature-api", "serde", "zip"],
  );
  assert.deepEqual(
    specs.find((spec) => spec.crateName === "flint-feature-accounts").dependencies,
    ["flint-core", "flint-feature-api", "flint-feature-settings", "reqwest", "serde", "serde_json"],
  );
  assert.deepEqual(
    specs.find((spec) => spec.crateName === "flint-feature-vanilla").dependencies,
    ["dirs", "flint-core", "flint-feature-api", "regex", "reqwest", "serde", "serde_json", "sha1"],
  );
  assert.deepEqual(
    specs.find((spec) => spec.crateName === "flint-feature-launch").dependencies,
    ["flint-core", "flint-feature-accounts", "flint-feature-api", "flint-feature-settings", "flint-feature-vanilla", "md5", "serde", "serde_json", "zip"],
  );
  assert.deepEqual(
    specs.find((spec) => spec.crateName === "flint-feature-mods").dependencies,
    ["flint-core", "flint-feature-api", "flint-feature-settings", "flint-provider-api", "regex", "serde", "serde_json", "zip"],
  );
  assert.deepEqual(
    specs.find((spec) => spec.crateName === "flint-feature-loaders").dependencies,
    ["flint-core", "flint-feature-api", "flint-feature-vanilla", "flint-provider-api", "serde", "serde_json"],
  );
  for (const spec of specs) {
    const manifest = [
      "[package]",
      `name = "${spec.crateName}"`,
      "version.workspace = true",
      "publish = false",
      "",
      "[dependencies]",
      ...spec.dependencies.map((dependency) => `${dependency} = "1"`),
      "",
    ].join("\n");
    assert.deepEqual(evaluateFeatureCrateManifest(spec.manifest, manifest), []);
    const withExtraDependency = manifest.replace(
      "[dependencies]\n",
      '[dependencies]\nunexpected = "1"\n',
    );
    assert.equal(
      evaluateFeatureCrateManifest(spec.manifest, withExtraDependency)
        .some((issue) => issue.message.includes("dependencies must be exactly")),
      true,
    );
    const wrongVersion = manifest.replace("version.workspace = true", 'version = "1.0.0"');
    assert.equal(
      evaluateFeatureCrateManifest(spec.manifest, wrongVersion)
        .some((issue) => issue.message.includes("inherit the workspace version")),
      true,
    );
    const wrongPublish = manifest.replace("publish = false", "publish = true");
    assert.equal(
      evaluateFeatureCrateManifest(spec.manifest, wrongPublish)
        .some((issue) => issue.message.includes("publish = false")),
      true,
    );
  }
  assert.deepEqual(evaluateFeatureDirectoryNames(specs.map((spec) => spec.crateName)), []);
  assert.deepEqual(
    evaluateFeatureDirectoryNames([...specs.map((spec) => spec.crateName), "flint-feature-extra"]),
    ["unexpected feature crate directory 'flint-feature-extra'."],
  );
  const declarations = specs.map((spec) => {
    const relativePath = spec.manifest
      .replace(/^src-tauri\//, "")
      .replace(/\/Cargo\.toml$/, "");
    return `${spec.crateName} = { path = "${relativePath}" }`;
  }).join("\n");
  assert.deepEqual(evaluateHostFeatureDependencyDeclarations(declarations), []);
  assert.equal(
    evaluateHostFeatureDependencyDeclarations(
      declarations.replace("modules/flint-feature-saves", "modules/wrong-saves"),
    ).length,
    1,
  );
  const lock = [
    "[[package]]",
    'name = "flint"',
    'version = "2026.10.1-d1cce64"',
    "",
    ...specs.flatMap((spec) => [
      "[[package]]",
      `name = "${spec.crateName}"`,
      'version = "2026.10.1-d1cce64"',
      "",
    ]),
  ].join("\n");
  assert.deepEqual(evaluateFeatureLockEntries(lock), []);
  assert.equal(
    evaluateFeatureLockEntries(
      lock.replace(
        'name = "flint-feature-settings"\nversion = "2026.10.1-d1cce64"',
        'name = "flint-feature-settings"\nversion = "0.0.0"',
      ),
    ).length,
    1,
  );
  const targets = reviewedFeatureTargets();
  assert.equal(targets.length, 7);
  assert.equal(targets.every((target) => !Object.hasOwn(target, "transitionalLocalTargets")), true);
  for (const target of targets) {
    assert.equal(target.role, "feature-leaf");
    assert.equal(target.externalRoots.includes("tauri"), false);
  }
  assert.deepEqual(
    targets.find((target) => target.file.includes("flint-feature-settings")).externalRoots,
    ["flint_core", "flint_feature_api", "serde", "serde_json", "std"],
  );
  assert.deepEqual(
    targets.find((target) => target.file.includes("flint-feature-saves")).externalRoots,
    ["base64", "flate2", "flint_core", "flint_feature_api", "serde", "std", "zip"],
  );
  assert.deepEqual(
    targets.find((target) => target.file.includes("flint-feature-accounts")).externalRoots,
    ["flint_core", "flint_feature_api", "flint_feature_settings", "reqwest", "serde", "serde_json", "std"],
  );
  assert.deepEqual(
    targets.find((target) => target.file.includes("flint-feature-accounts")).localTargets,
    [],
  );
  assert.deepEqual(
    targets.find((target) => target.file.includes("flint-feature-vanilla")).externalRoots,
    ["dirs", "flint_core", "flint_feature_api", "regex", "reqwest", "serde", "serde_json", "sha1", "std"],
  );
  assert.deepEqual(
    targets.find((target) => target.file.includes("flint-feature-vanilla")).localTargets,
    ["install", "java", "version"],
  );
  assert.deepEqual(
    targets.find((target) => target.file.includes("flint-feature-launch")).externalRoots,
    ["flint_core", "flint_feature_accounts", "flint_feature_api", "flint_feature_settings", "flint_feature_vanilla", "md5", "serde", "serde_json", "std", "zip"],
  );
  assert.deepEqual(
    targets.find((target) => target.file.includes("flint-feature-launch")).localTargets,
    [],
  );
  assert.deepEqual(
    targets.find((target) => target.file.includes("flint-feature-mods")).externalRoots,
    ["flint_core", "flint_feature_api", "flint_feature_settings", "flint_provider_api", "regex", "serde", "serde_json", "std", "zip"],
  );
  assert.deepEqual(
    targets.find((target) => target.file.includes("flint-feature-mods")).localTargets,
    [],
  );
  assert.deepEqual(
    targets.find((target) => target.file.includes("flint-feature-loaders")).externalRoots,
    ["flint_core", "flint_feature_api", "flint_feature_vanilla", "flint_provider_api", "serde", "serde_json", "std"],
  );
  assert.deepEqual(
    targets.find((target) => target.file.includes("flint-feature-loaders")).localTargets,
    [],
  );
});

test("vanilla install keeps the Tauri sink/event boundary in the host adapter", () => {
  const feature = `
pub trait InstallProgressSink: Send + Sync {
    fn report(&self, progress: InstallProgress);
}
pub fn install_blocking(
    game_dir: &str,
    version_id: &str,
    sink: &dyn InstallProgressSink,
) -> Result<InstallResult, String> { unimplemented!() }
`;
  const host = `
const INSTALL_PROGRESS_EVENT: &str = "flint://install-progress";
impl InstallProgressSink for TauriInstallProgressSink<'_> {
    fn report(&self, progress: InstallProgress) {
        let _ = self.app.emit(INSTALL_PROGRESS_EVENT, progress);
    }
}
pub fn install_blocking(
    app: &AppHandle,
    game_dir: &str,
    version_id: &str,
) -> Result<InstallResult, String> {
    feature_install::install_blocking(game_dir, version_id, &sink)
}
`;
  assert.deepEqual(evaluateVanillaInstallBoundary(feature, host), []);
  assert.equal(
    evaluateVanillaInstallBoundary(feature.replace("Send + Sync", ""), host)
      .some((issue) => issue.message.includes("Send + Sync")),
    true,
  );
  assert.equal(
    evaluateVanillaInstallBoundary(`${feature}\nfn bad() { let _ = tauri::AppHandle; }`, host)
      .some((issue) => issue.message.includes("Tauri")),
    true,
  );
  assert.equal(
    evaluateVanillaInstallBoundary(feature, host.replace("flint://install-progress", "wrong"))
      .some((issue) => issue.message.includes("flint://install-progress")),
    true,
  );
});

test("mods and loaders host adapters stay thin Tauri sink wrappers", () => {
  const mods = `
const MOD_INSTALL_PROGRESS_EVENT: &str = "flint://mod-install-progress";
impl ModInstallProgressSink for TauriModInstallProgressSink<'_> {
    fn report(&self, progress: ModInstallProgress) {
        let _ = self.app.emit(MOD_INSTALL_PROGRESS_EVENT, progress);
    }
}
flint_feature_mods::install_mod_blocking(registry, &sink, game_dir, source, project_id, version_id, game_version, loader, install_dependencies)
`;
  const loaders = `
const LOADER_INSTALL_PROGRESS_EVENT: &str = "flint://loader-install-progress";
const INSTALL_PROGRESS_EVENT: &str = "flint://install-progress";
impl LoaderProgressSink for TauriLoaderProgressSink<'_> {
    fn report(&self, progress: LoaderInstallProgress) {
        let _ = self.app.emit(LOADER_INSTALL_PROGRESS_EVENT, progress);
    }
}
impl LoaderInstallProgressSink for TauriVanillaProgressSink<'_> {
    fn report(&self, progress: InstallProgress) {
        let _ = self.app.emit(INSTALL_PROGRESS_EVENT, progress);
    }
}
flint_feature_loaders::list_loader_versions_blocking(registry, loader, game_version)
flint_feature_loaders::install_loader_blocking(registry, request, &loader_sink, &vanilla_sink)
`;
  assert.deepEqual(evaluateHostModLoaderAdapters(mods, loaders), []);
  assert.equal(
    evaluateHostModLoaderAdapters(mods, loaders.replace("flint://install-progress", "wrong"))
      .some((issue) => issue.message.includes("vanilla progress event")),
    true,
  );
  assert.equal(
    evaluateHostModLoaderAdapters(`${mods}\nfn install_single_mod() {}`, loaders)
      .some((issue) => issue.message.includes("legacy implementation")),
    true,
  );
});
test("provider composition requires one shared runtime state, six keys, constructors, and one registry", () => {
  const keys = [
    "FABRIC_SELECTION_KEY",
    "FORGE_SELECTION_KEY",
    "LITELOADER_SELECTION_KEY",
    "NEOFORGE_SELECTION_KEY",
    "OPTIFINE_SELECTION_KEY",
    "QUILT_SELECTION_KEY",
  ];
  const constructors = [
    "FabricLoaderProvider::new",
    "ForgeLoaderProvider::new",
    "LiteLoaderProvider::new",
    "NeoForgeLoaderProvider::new",
    "OptiFineProvider::new",
    "QuiltLoaderProvider::new",
  ];
  const source = [
    ...keys.map((name) => `const ${name} = key;`),
    "type ModRegistry = ProviderRegistry<dyn ModProvider>;",
    "type LoaderRegistry = ProviderRegistry<dyn LoaderProvider>;",
    "struct ProviderRuntimeState { mod_registry: ModRegistry, loader_registry: LoaderRegistry }",
    "impl ProviderCatalog for ProviderRuntimeState {",
    "    fn initialize(&mut self, provider_id: &str) -> Result<(), String> { if self.provider_is_known(provider_id) { Ok(()) } else { Err(String::new()) } }",
    "    fn shutdown(&mut self, provider_id: &str) -> Result<(), String> { self.mark_unavailable(provider_id, \"stopped\"); Ok(()) }",
    "}",
    "fn loader_provider_requirements() -> Vec<ProviderRequirement> {",
    `    vec![${keys.map((key) => `requirement(${key})`).join(", ")}]`,
    "}",
    "fn build_loader_provider_registry() -> Result<ProviderRegistry<dyn LoaderProvider>, String> {",
    `    let providers = vec![${constructors.map((constructor) => `Box::new(${constructor})`).join(", ")}];`,
    "    ProviderRegistry::new(providers, LOADER_PROVIDER_API).map_err(|error| error.to_string())",
    "}",
  ].join("\n");
  assert.deepEqual(evaluateLoaderProviderComposition(source), []);
  assert.equal(
    evaluateLoaderProviderComposition(source.replaceAll("QUILT_SELECTION_KEY", "OTHER_KEY"))
      .some((issue) => issue.message.includes("QUILT_SELECTION_KEY")),
    true,
  );
  assert.equal(
    evaluateLoaderProviderComposition(source.replaceAll("QuiltLoaderProvider::new", "OtherProvider::new"))
      .some((issue) => issue.message.includes("QuiltLoaderProvider::new")),
    true,
  );
  assert.equal(
    evaluateLoaderProviderComposition(source.replace(
      "struct ProviderRuntimeState { mod_registry: ModRegistry, loader_registry: LoaderRegistry }",
      "struct OtherState { mod_registry: ModRegistry, loader_registry: LoaderRegistry }",
    )).some((issue) => issue.message.includes("ProviderRuntimeState")),
    true,
  );
  assert.equal(
    evaluateLoaderProviderComposition(
      source.replaceAll("ProviderRegistry<dyn LoaderProvider>", "ProviderRegistry<dyn OtherProvider>"),
    ).some((issue) => issue.message.includes("ProviderRegistry<dyn LoaderProvider>")),
    true,
  );
  assert.equal(
    evaluateLoaderProviderComposition(source.replace("mod_registry:", "metadata_mod:"))
      .some((issue) => issue.message.includes("mod_registry and loader_registry")),
    true,
  );
  assert.equal(
    evaluateLoaderProviderComposition(
      source.replace("impl ProviderCatalog for ProviderRuntimeState {", "impl OtherCatalog for ProviderRuntimeState {"),
    ).some((issue) => issue.message.includes("ProviderCatalog")),
    true,
  );
  assert.equal(
    evaluateLoaderProviderComposition(source.replace("self.provider_is_known(provider_id)", "true"))
      .some((issue) => issue.message.includes("initialize must consult")),
    true,
  );
  assert.equal(
    evaluateLoaderProviderComposition(
      source.replace(
        "ProviderRegistry::new(providers, LOADER_PROVIDER_API).map_err(|error| error.to_string())",
        "ProviderRegistry::new(providers, LOADER_PROVIDER_API); ProviderRegistry::new(providers, LOADER_PROVIDER_API)",
      ),
    ).some((issue) => issue.message.includes("exactly one provider registry")),
    true,
  );
});
test("phase-5 shared ProviderRuntimeState owns both registries and is initialized once", () => {
  const compositionSource = [
    "type ModRegistry = ProviderRegistry<dyn ModProvider>;",
    "type LoaderRegistry = ProviderRegistry<dyn LoaderProvider>;",
    "struct ProviderRuntimeState { mod_registry: ModRegistry, loader_registry: LoaderRegistry }",
    "impl ProviderCatalog for ProviderRuntimeState {",
    "    fn initialize(&mut self, provider_id: &str) -> Result<(), String> { if self.provider_is_known(provider_id) { Ok(()) } else { Err(String::new()) } }",
    "    fn shutdown(&mut self, provider_id: &str) -> Result<(), String> { self.mark_unavailable(provider_id, \"stopped\"); Ok(()) }",
    "}",
    "fn build_mod_provider_registry() -> Result<ModRegistry, String> {",
    "    ProviderRegistry::new(vec![Box::new(CurseForgeModProvider::new())], MOD_PROVIDER_API)",
    "}",
    "fn build_loader_provider_registry() -> Result<LoaderRegistry, String> {",
    "    ProviderRegistry::new(vec![Box::new(FabricLoaderProvider::new())], LOADER_PROVIDER_API)",
    "}",
  ].join("\n");
  const hostLibSource = [
    "let provider_runtime = ProviderRuntimeState::new();",
    "app.manage(provider_runtime.clone());",
    "app.manage(initialize_host_runtime(provider_runtime));",
  ].join("\n");
  const sources = [
    { relativePath: "src-tauri/src/provider_composition.rs", productionText: compositionSource },
    { relativePath: "src-tauri/src/lib.rs", productionText: hostLibSource },
  ];
  assert.deepEqual(evaluatePhase5ProviderRuntimeSources(sources), []);
  const workspaceRecords = sources.map(({ relativePath, ...source }) => ({ relative: relativePath, ...source }));
  assert.deepEqual(evaluatePhase5ProviderRuntimeSources(workspaceRecords), []);

  const duplicateInitialization = evaluatePhase5ProviderRuntimeSources([
    sources[0],
    { ...sources[1], productionText: `${hostLibSource}\nlet duplicate = ProviderRuntimeState::new();` },
  ]);
  assert.equal(duplicateInitialization.some((issue) => issue.rule === "RUST-PROVIDER-RUNTIME-001"), true);

  const duplicateRuntime = evaluatePhase5ProviderRuntimeSources([
    ...sources,
    { relativePath: "src-tauri/src/feature_host.rs", productionText: "struct HostProviderCatalog;" },
  ]);
  assert.equal(duplicateRuntime.some((issue) => issue.rule === "ARCH-PHASE5-SHIM-001"), true);

  const duplicateCatalog = evaluatePhase5ProviderRuntimeSources([
    ...sources,
    { relativePath: "src-tauri/src/feature_host.rs", productionText: "impl ProviderCatalog for MetadataCatalog {}" },
  ]);
  assert.equal(duplicateCatalog.some((issue) => issue.rule === "RUST-PROVIDER-RUNTIME-001"), true);

  const duplicateState = evaluatePhase5ProviderRuntimeSources([
    ...sources,
    { relativePath: "src-tauri/src/feature_host.rs", productionText: "struct ProviderRuntimeState { metadata_only: bool }" },
  ]);
  assert.equal(duplicateState.some((issue) => issue.rule === "RUST-PROVIDER-RUNTIME-001"), true);

  const constructionOutsideComposition = evaluatePhase5ProviderRuntimeSources([
    sources[0],
    { ...sources[1], productionText: `${hostLibSource}\nBox::new(CurseForgeModProvider::new());` },
  ]);
  assert.equal(constructionOutsideComposition.some((issue) => issue.rule === "RUST-PROVIDER-COMPOSITION-001"), true);
});test("host command gate map covers exactly the 28 feature commands and reviewed module", () => {
  const gates = reviewedFeatureCommandGates();
  assert.equal(gates.length, 28);
  assert.equal(new Set(gates.map((gate) => gate.command)).size, 28);
  assert.deepEqual(
    [...new Set(gates.map((gate) => gate.featureId))].sort(),
    ["flint.accounts", "flint.launch", "flint.loaders", "flint.mods", "flint.saves", "flint.settings", "flint.vanilla"],
  );
  const counts = Object.fromEntries(
    [...new Set(gates.map((gate) => gate.featureId))].map((featureId) => [
      featureId,
      gates.filter((gate) => gate.featureId === featureId).length,
    ]),
  );
  assert.deepEqual(counts, {
    "flint.settings": 2,
    "flint.accounts": 6,
    "flint.vanilla": 4,
    "flint.launch": 1,
    "flint.loaders": 2,
    "flint.mods": 7,
    "flint.saves": 6,
  });
  assert.equal(reviewedHostRustModuleIds().includes("feature_host"), true);
  assert.equal(reviewedHostRustModuleIds().includes("provider_composition"), true);
  assert.equal(reviewedHostRustModuleIds().includes("error"), false);
  assert.equal(reviewedHostRustModuleIds().includes("net"), false);
  assert.equal(reviewedHostRustModuleIds().includes("paths"), false);
  assert.equal(reviewedHostRustModuleIds().includes("settings"), false);
  assert.equal(reviewedHostRustModuleIds().includes("saves"), false);
  assert.equal(reviewedHostRustModuleIds().includes("auth"), false);
  assert.equal(reviewedHostRustModuleIds().includes("java"), false);
  assert.equal(reviewedHostRustModuleIds().includes("version"), false);
  assert.equal(reviewedHostRustModuleIds().includes("launch"), false);
});

test("host command gate audit requires exact feature ownership, managed state, and one handler per command", () => {
  const { gates, source } = hostCommandFixture();
  assert.deepEqual(auditHostCommandGates(source), []);

  const wrongGate = source.replace(
    "runtime.ensure_feature_ready(feature_host::FEATURE_SETTINGS)?;",
    "runtime.ensure_feature_ready(feature_host::FEATURE_ACCOUNTS)?;",
  );
  assert.ok(
    auditHostCommandGates(wrongGate).some((issue) =>
      issue.message.includes("command 'load_settings' must gate 'flint.settings'"),
    ),
  );

  const wrongFeatureId = source.replace(
    'pub(crate) const FEATURE_SETTINGS: &str = "flint.settings";',
    'pub(crate) const FEATURE_SETTINGS: &str = "flint.accounts";',
  );
  assert.ok(
    auditHostCommandGates(wrongFeatureId).some((issue) =>
      issue.message.includes("host feature ID constant 'FEATURE_SETTINGS' must equal 'flint.settings'"),
    ),
  );

  const missingGate = source.replace(
    "    runtime.ensure_feature_ready(feature_host::FEATURE_SETTINGS)?;\n",
    "",
  );
  assert.ok(
    auditHostCommandGates(missingGate).some((issue) =>
      issue.message.includes("must contain exactly one feature gate call, found 0"),
    ),
  );

  const missingHandler = source.replace("        load_settings,\n", "");
  assert.ok(
    auditHostCommandGates(missingHandler).some((issue) =>
      issue.message.includes("#[tauri::command] function 'load_settings' is missing from generate_handler!"),
    ),
  );

  const duplicateHandler = source.replace(
    "        get_host_availability,\n",
    "        get_host_availability,\n        get_host_availability,\n",
  );
  assert.ok(
    auditHostCommandGates(duplicateHandler).some((issue) =>
      issue.message.includes("generate_handler registers 'get_host_availability' more than once"),
    ),
  );

  assert.equal(gates[0].command, "load_settings");
});

test("Rust use roots accept normal and leading-:: paths", () => {
  assert.equal(firstUseRoot(" semver::{Version, VersionReq}"), "semver");
  assert.equal(firstUseRoot(" ::std::collections::BTreeMap"), "std");
  assert.equal(firstUseRoot(" r#crate::models"), "crate");
});

test("Rust crate-root use payloads resolve to imported modules rather than graph sentinels", () => {
  const modules = new Set([
    "crate",
    "main",
    "models",
    "mods",
    "provider_composition",
    "provider_composition::tests",
  ]);
  assert.deepEqual(
    [...moduleTargetsFromUse(
      " crate::models::{ModSource}",
      "provider_composition::tests",
      modules,
    )].sort(),
    ["models"],
  );
  assert.deepEqual(
    [...moduleTargetsFromUse(
      " super::{helper}",
      "provider_composition::tests",
      modules,
    )].sort(),
    ["provider_composition"],
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

test("phase-5 Tauri adapter allowlist is exact and rejects extra or local adapter imports", () => {
  const expectedAdapters = [
    { file: "src/core/api/tauriInvoke.ts", specifier: "@tauri-apps/api/core" },
    { file: "src/core/events/tauriEvents.ts", specifier: "@tauri-apps/api/event" },
    { file: "src/core/platform/tauriDialogs.ts", specifier: "@tauri-apps/plugin-dialog" },
    { file: "src/core/platform/tauriOpener.ts", specifier: "@tauri-apps/plugin-opener" },
    { file: "src/core/platform/tauriWindow.ts", specifier: "@tauri-apps/api/window" },
  ];
  assert.deepEqual(reviewedTauriAdapterAllowlist(), expectedAdapters);
  for (const adapter of expectedAdapters) {
    assert.equal(classifyTauriImport(5, adapter.file, adapter.specifier), null);
    assert.equal(isReviewedTauriAdapterImport(adapter.file, adapter.specifier), true);
    assert.equal(isTauriPackageSpecifier(adapter.specifier), true);
  }
  assert.equal(classifyTauriImport(5, "src/api.ts", "@tauri-apps/api/core").rule, "FE-TAURI-DIRECT-001");
  assert.equal(classifyTauriImport(5, "src/core/events/tauriEvents.ts", "@tauri-apps/api/window").rule, "FE-TAURI-DIRECT-001");
  assert.equal(classifyTauriImport(5, "src/core/events/tauriEvents.ts", "@tauri-apps/plugin-fs").rule, "FE-TAURI-DIRECT-001");
  assert.equal(classifyTauriImport(5, "src/api.ts", "react"), null);
  assert.deepEqual(evaluateTauriReferencePolicy("src/core/api/tauriInvoke.ts", "@tauri-apps/api/core", "static"), []);
  assert.equal(
    evaluateTauriReferencePolicy("src/core/api/tauriInvoke.ts", "@tauri-apps/api/core", "dynamic")[0].rule,
    "FE-TAURI-DYNAMIC-001",
  );
  assert.equal(
    evaluateTauriReferencePolicy("src/api.ts", "@tauri-apps/api/core", "static")[0].rule,
    "FE-TAURI-DIRECT-001",
  );

  assert.equal(evaluatePhase5TauriAdapterImport("src/api.ts", "src/core/api/tauriInvoke.ts"), null);
  assert.equal(evaluatePhase5TauriAdapterImport("src/core/platform/tauriServices.ts", "src/core/events/tauriEvents.ts"), null);
  assert.equal(evaluatePhase5TauriAdapterImport("src/main.tsx", "src/core/platform/tauriServices.ts"), null);
  assert.equal(
    evaluatePhase5TauriAdapterImport("src/modules/accounts/AccountDialog.tsx", "src/core/platform/tauriOpener.ts").rule,
    "FE-TAURI-ADAPTER-IMPORT-001",
  );
  assert.equal(
    evaluatePhase5TauriAdapterImport("src/api.ts", "src/core/events/tauriEvents.ts").rule,
    "FE-TAURI-ADAPTER-IMPORT-001",
  );
  assert.equal(
    evaluatePhase5TauriAdapterImport("src/app/App.tsx", "src/core/platform/tauriServices.ts").rule,
    "FE-TAURI-ADAPTER-IMPORT-001",
  );
  assert.equal(
    evaluateFrontendBoundary(5, "src/modules/accounts/AccountDialog.tsx", "src/core/platform/tauriOpener.ts").rule,
    "FE-TAURI-ADAPTER-IMPORT-001",
  );

  assert.deepEqual(evaluatePhase5HostServicesFactorySource("src/main.tsx", "createTauriHostServices()"), []);
  assert.deepEqual(evaluatePhase5HostServicesFactorySource("src/core/platform/tauriServices.ts", "export function createTauriHostServices() {}"), []);
  assert.equal(
    evaluatePhase5HostServicesFactorySource("src/modules/accounts/index.tsx", "createTauriHostServices()")[0].rule,
    "FE-TAURI-HOST-SERVICES-001",
  );

  const loadingSource = parseTypeScriptSource(
    "sample.ts",
    [
      'const lazy = import("@tauri-apps/api/core");',
      'const glob = import.meta.glob("@tauri-apps/api/event");',
    ].join("\n"),
  );
  const dynamicRefs = collectFrontendModuleReferences(loadingSource).filter(
    (reference) => reference.kind === "dynamic" && isTauriPackageSpecifier(reference.specifier),
  );
  assert.deepEqual(dynamicRefs.map((reference) => reference.specifier), ["@tauri-apps/api/core"]);
  assert.deepEqual(
    collectImportMetaTauriLoadingSites(loadingSource).map((site) => site.specifier),
    ["@tauri-apps/api/event"],
  );
});

test("phase-5 production shim and old-dispatch tokens fail closed while tests remain masked", () => {
  const sharedTokens = [
    "TAURI_TRANSITIONAL_WAIVERS",
    "transitionalLocalTargets",
    "providerTargetAllowed",
    "removeByPhase",
    "HostProviderCatalog",
  ];
  for (const token of sharedTokens) {
    assert.equal(
      evaluatePhase5ProductionShimTokens("src-tauri/src/feature_host.rs", `let ${token} = 1;`)
        .some((issue) => issue.rule === "ARCH-PHASE5-SHIM-001"),
      true,
      token,
    );
  }
  for (const token of ["AccountDialogSurface", "loaderKindFromValue", "neo_forge", "neo-forge", "lite_loader", "lite-loader"]) {
    const fixture = token.includes("-") ? `let _ = "${token}";` : `const ${token} = legacy;`;
    assert.equal(
      evaluatePhase5ProductionShimTokens("src/app/App.tsx", fixture)
        .some((issue) => issue.rule === "FE-PHASE5-SHIM-001"),
      true,
      token,
    );
  }
  for (const token of ["LoaderProviderContext", "normalize_loader", "neo_forge", "neo-forge", "lite_loader", "lite-loader"]) {
    const fixture = token.includes("-") ? `let _ = "${token}";` : `fn ${token}() {}`;
    assert.equal(
      evaluatePhase5ProductionShimTokens("src-tauri/modules/flint-feature-mods/src/lib.rs", fixture)
        .some((issue) => issue.rule === "RUST-PHASE5-SHIM-001"),
      true,
      token,
    );
  }
  assert.equal(
    evaluatePhase5ProductionShimTokens(
      "src-tauri/modules/flint-feature-loaders/src/lib.rs",
      "struct LoaderBinding { display_name: &'static str }",
    ).some((issue) => issue.rule === "RUST-PHASE5-SHIM-001"),
    true,
  );
  assert.equal(
    evaluatePhase5ProductionShimTokens(
      "src-tauri/modules/flint-feature-loaders/src/lib.rs",
      "let label = LoaderBinding.display_name;",
    ).some((issue) => issue.rule === "RUST-PHASE5-SHIM-001"),
    true,
  );
  assert.deepEqual(
    evaluatePhase5ProductionShimTokens("src-tauri/src/models.rs", "display_name: String,"),
    [],
  );

  const maskedTests = prepareRustProductionSource(`
fn production() {}
#[cfg(test)]
mod tests {
    struct HostProviderCatalog;
    struct LoaderBinding { display_name: &'static str }
    fn aliases() {
        let _ = normalize_loader("neo_forge");
        let _ = "lite_loader";
    }
}
`);
  for (const maskedPath of [
    "src-tauri/modules/flint-feature-mods/src/lib.rs",
    "src-tauri/modules/flint-feature-loaders/src/lib.rs",
  ]) {
    assert.deepEqual(evaluatePhase5ProductionShimTokens(maskedPath, maskedTests), []);
  }
});
test("phase-5 overlay contribution slot and exact loader parser are typed and strict", () => {
  const overlaySource = [
    "export interface OverlayContribution {",
    "  readonly id: string;",
    "  readonly featureId: string;",
    "  readonly order: number;",
    "  readonly component: PageComponent;",
    "  readonly visible?: (context: AppPageContext) => boolean;",
    "}",
    "export interface FeatureContributions {",
    "  readonly overlays: readonly OverlayContribution[];",
    "}",
    "export interface FrontendContributionRegistry {",
    "  readonly overlays: readonly OverlayContribution[];",
    "}",
    "if (!Array.isArray(feature.contributions.overlays)) throw new Error('bad');",
    "assertFunction(overlay.component, 'component');",
    "assertFunction(overlay.visible, 'visible');",
    "const frozen = freezeOrderedContributions(feature.contributions.overlays);",
    "const overlays = freezeOrderedContributions(orderedFeatures.flatMap((feature) => feature.contributions.overlays));",
  ].join("\n");
  assert.deepEqual(evaluateOverlayContributionContract(overlaySource), []);
  assert.equal(
    evaluateOverlayContributionContract(overlaySource.replace("readonly visible?: (context: AppPageContext) => boolean;", "visible: unknown;"))
      .some((issue) => issue.rule === "FE-PHASE5-OVERLAY-001" && /visible/.test(issue.message)),
    true,
  );
  assert.equal(
    evaluateOverlayContributionContract(overlaySource.replace("readonly overlays: readonly OverlayContribution[];", ""))
      .some((issue) => issue.rule === "FE-PHASE5-OVERLAY-001" && /overlays/.test(issue.message)),
    true,
  );

  const parserSource = [
    'export const LOADER_KINDS = ["fabric", "forge", "neoforge", "quilt", "liteloader", "optifine"];',
    "export function isLoaderKind(value: string): value is LoaderKind {",
    "  return (LOADER_KINDS as string[]).includes(value.trim().toLowerCase());",
    "}",
    "export function parseLoaderKind(value: string): LoaderKind | null {",
    "  const normalized = value.trim().toLowerCase();",
    '  if (!normalized || normalized === "vanilla" || normalized === "原版") return null;',
    "  return LOADER_KINDS.find((kind) => kind === normalized) ?? null;",
    "}",
  ].join("\n");
  assert.deepEqual(evaluateExactLoaderKindParser(parserSource), []);
  const fuzzyParser = parserSource.replace(
    "  return LOADER_KINDS.find((kind) => kind === normalized) ?? null;",
    '  if (normalized === "neo_forge" || normalized.startsWith("neo")) return "neoforge" as LoaderKind;\n  return null;',
  );
  const fuzzyIssues = evaluateExactLoaderKindParser(fuzzyParser);
  assert.equal(fuzzyIssues.some((issue) => issue.rule === "FE-LOADER-PARSER-001" && /substring or prefix/.test(issue.message)), true);
  assert.equal(fuzzyIssues.some((issue) => /legacy loader aliases/.test(issue.message)), true);

  const includesParser = parserSource.replace(
    "  return LOADER_KINDS.find((kind) => kind === normalized) ?? null;",
    [
      "  const exact = LOADER_KINDS.find((kind) => kind === normalized);",
      '  if (normalized.includes("fabric")) return "fabric" as LoaderKind;',
      "  return exact ?? null;",
    ].join("\n"),
  );
  const includesIssues = evaluateExactLoaderKindParser(includesParser);
  assert.equal(includesIssues.some((issue) => issue.rule === "FE-LOADER-PARSER-001" && /includes/.test(issue.message)), true);
  assert.equal(includesIssues.some((issue) => issue.message.includes("exact canonical comparison")), false);

  const extraAliasParser = parserSource.replace(
    'export const LOADER_KINDS = ["fabric", "forge", "neoforge", "quilt", "liteloader", "optifine"];',
    'export const LOADER_KINDS = ["fabric", "forge", "neoforge", "quilt", "liteloader", "optifine", "fabric-loader"];',
  );
  assert.equal(
    evaluateExactLoaderKindParser(extraAliasParser)
      .some((issue) => issue.rule === "FE-LOADER-PARSER-001" && /extra or alias literals/.test(issue.message)),
    true,
  );

  const reorderedParser = parserSource.replace(
    'export const LOADER_KINDS = ["fabric", "forge", "neoforge", "quilt", "liteloader", "optifine"];',
    'export const LOADER_KINDS = ["forge", "fabric", "neoforge", "quilt", "liteloader", "optifine"];',
  );
  assert.equal(
    evaluateExactLoaderKindParser(reorderedParser)
      .some((issue) => issue.rule === "FE-LOADER-PARSER-001" && /reviewed order/.test(issue.message)),
    true,
  );
});
test("phase-5 frontend contract gate requires HostServices, overlays, exact parsing, and stale-request guards", () => {
  const contributions = [
    "export interface OverlayContribution {",
    "  readonly id: string;",
    "  readonly featureId: string;",
    "  readonly order: number;",
    "  readonly component: PageComponent;",
    "  readonly visible?: (context: AppPageContext) => boolean;",
    "}",
    "export interface FeatureContributions { readonly overlays: readonly OverlayContribution[]; }",
    "export interface FrontendContributionRegistry { readonly overlays: readonly OverlayContribution[]; }",
    "if (!Array.isArray(feature.contributions.overlays)) throw new Error('bad');",
    "assertFunction(overlay.component, 'component');",
    "assertFunction(overlay.visible, 'visible');",
    "freezeOrderedContributions(feature.contributions.overlays);",
    "freezeOrderedContributions(orderedFeatures.flatMap((feature) => feature.contributions.overlays));",
  ].join("\n");
  const parser = [
    'export const LOADER_KINDS = ["fabric", "forge", "neoforge", "quilt", "liteloader", "optifine"];',
    "export function isLoaderKind(value: string): value is LoaderKind {",
    "  return (LOADER_KINDS as string[]).includes(value.trim().toLowerCase());",
    "}",
    "export function parseLoaderKind(value: string): LoaderKind | null {",
    "  const normalized = value.trim().toLowerCase();",
    '  if (!normalized || normalized === "vanilla") return null;',
    "  return LOADER_KINDS.find((kind) => kind === normalized) ?? null;",
    "}",
  ].join("\n");
  const sources = [
    ["src/core/contributions.ts", contributions],
    ["src/utils.ts", parser],
    ["src/core/app-context.ts", "import type { HostServices } from './platform/hostServices';\nreadonly services: HostServices;\nreadonly accountDialogOpen: boolean;"],
    ["src/core/availability.ts", "const overlays = freezeOrderedContributions(registry.overlays.filter((overlay) => activeFeatureIds.has(overlay.featureId)));"],
    ["src/app/App.tsx", "availableRegistry.overlays.map((overlay) => { if (overlay.visible && !overlay.visible(pageContext)) return null; return (<ContributionBoundary><Overlay context={pageContext} /></ContributionBoundary>); });\nuseRequestGeneration();\nrequests.isCurrent(requestId);"],
    ["src/main.tsx", 'import { createTauriHostServices } from "./core/platform/tauriServices";\nconst hostServices = createTauriHostServices();'],
    ["src/api.ts", "import { tauriInvoke } from './core/api/tauriInvoke';"],
    ["src/modules/accounts/index.tsx", 'const ACCOUNT_OVERLAY_ID = "flint.accounts.overlay.account-dialog";\noverlays: [];\nvisible: (context: AppPageContext) => context.accountDialogOpen'],
    ...["about", "launch", "saves", "settings", "versions"].map((name) => [`src/modules/${name}/index.tsx`, "overlays: []"]),
    ["src/modules/versions/downloads/LoaderInstallPanel.tsx", "parseLoaderKind(); useRequestGeneration(); requests.isCurrent(id);"],
    ["src/modules/versions/downloads/ModDiscoveryPanel.tsx", "parseLoaderKind(); useRequestGeneration(); requests.isCurrent(id);"],
    ["src/modules/saves/SaveManagerView.tsx", "useRequestGeneration(); requests.isCurrent(id);"],
    ...reviewedTauriAdapterAllowlist().map((adapter) => [adapter.file, "export {};"]),
  ].map(([relativePath, sourceText]) => ({ relativePath, sourceText }));
  assert.deepEqual(evaluatePhase5FrontendContractSources(sources), []);

  const missingOverlay = sources.map((source) =>
    source.relativePath === "src/modules/accounts/index.tsx"
      ? { ...source, sourceText: "overlays: []; visible: (context: AppPageContext) => context.accountDialogOpen" }
      : source,
  );
  assert.equal(
    evaluatePhase5FrontendContractSources(missingOverlay)
      .some((issue) => issue.rule === "FE-PHASE5-OVERLAY-001" && /exactly one overlay/.test(issue.message)),
    true,
  );
  const missingAdapter = sources.filter((source) => source.relativePath !== "src/core/platform/tauriWindow.ts");
  assert.equal(
    evaluatePhase5FrontendContractSources(missingAdapter)
      .some((issue) => issue.rule === "FE-TAURI-ADAPTER-IMPORT-001" && /tauriWindow/.test(issue.message)),
    true,
  );
  const missingLoaderParser = sources.map((source) =>
    source.relativePath === "src/modules/versions/downloads/LoaderInstallPanel.tsx"
      ? { ...source, sourceText: "useRequestGeneration(); requests.isCurrent(id);" }
      : source,
  );
  assert.equal(
    evaluatePhase5FrontendContractSources(missingLoaderParser)
      .some((issue) => issue.rule === "FE-LOADER-PARSER-001" && /parseLoaderKind/.test(issue.message)),
    true,
  );
  const missingStaleGuard = sources.map((source) =>
    source.relativePath === "src/modules/saves/SaveManagerView.tsx"
      ? { ...source, sourceText: "requests.isCurrent(id);" }
      : source,
  );
  assert.equal(
    evaluatePhase5FrontendContractSources(missingStaleGuard)
      .some((issue) => issue.rule === "FE-STALE-REQUEST-001" && /useRequestGeneration/.test(issue.message)),
    true,
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
