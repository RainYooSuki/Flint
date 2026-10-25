# Flint Architecture

## Document status

This document is the canonical detailed source for architecture boundaries, provider/registry semantics, lifecycle, frontend contributions, and migration phases. It defines the implemented baseline, the accepted target design, and the migration path between them.

| Label | Meaning |
| --- | --- |
| **Implemented** | Present in the repository at the recorded baseline. |
| **Planned** | Approved target design that has not been implemented yet. |
| **Deferred** | Explicitly outside the current design and requires a separate decision. |

Current baseline: the working tree contains the Phase 5 strict-hardening implementation, subject to GitHub Actions verification. The architecture has nineteen Cargo packages, typed mod/loader provider contracts and eight extracted provider leaves, seven extracted feature crates, a framework-neutral feature lifecycle/availability coordinator, one shared actual-registry `ProviderRuntimeState`, snapshot-driven command gating and frontend filtering, a typed five-adapter frontend Tauri boundary, and a five-slot frontend contribution registry including overlays. The release `prepare` gate runs the checker and frontend contract tests and invokes `node scripts/check-architecture.mjs --phase 5`. GitHub Actions is the final verification authority; this document does not claim a CI-green result.

### Canonical migration phase status

| Area | Status |
| --- | --- |
| **Rust structure** | **Implemented (CI verification pending).** `src-tauri/Cargo.toml` is both the host package manifest and the workspace root with resolver 2 and exactly nineteen current members: `.`, `crates/flint-core`, `crates/flint-feature-api`, `crates/flint-provider-api`, `modules/flint-feature-accounts`, `modules/flint-feature-launch`, `modules/flint-feature-loaders`, `modules/flint-feature-mods`, `modules/flint-feature-vanilla`, `modules/flint-feature-settings`, `modules/flint-feature-saves`, `providers/flint-mod-provider-curseforge`, `providers/flint-mod-provider-modrinth`, and the six `providers/flint-loader-provider-*` crates. The host package remains `flint` with the `flint_lib` library target. All nineteen packages share the generated date-based release version through the matching `[workspace.package] version`; non-root members use `version.workspace = true`. `flint-core` owns shared path/network/error policy plus framework-neutral typed ports for bounded network access, safe workspace/archive handling, process/Java execution, progress, logging, deadlines, and cancellation. `flint-provider-api` owns the generic provider registry plus typed mod and loader contracts. The seven feature crates implement accounts, vanilla, settings, saves, launch, mods, and loaders plus their DTOs; the two mod provider crates and six loader provider crates implement the typed provider contracts through core ports. The host re-exports `flint_core` policy modules as `crate::{paths, error}`; `flint_core::net` remains a core module. |
| **Phase 1: ModProvider** | **Implemented (CI verification pending).** The generic provider registry and typed mod contracts live in `flint-provider-api`; CurseForge/Modrinth implementations live in their extracted crates. The host creates one shared actual-registry `ProviderRuntimeState`, composes and injects one immutable mod registry, and uses the same provider lifecycle/catalog truth for feature availability and commands. Strict Phase 5 source, dependency, typed-port, composition, and shim checks are implemented. |
| **Phase 2: LoaderProvider** | **Implemented (CI verification pending).** The six `providers/flint-loader-provider-*` crates implement the typed `LoaderProvider` contract through core ports. `ProviderRuntimeState` owns the single immutable loader registry; `modules/flint-feature-loaders/src/lib.rs` holds the sole `LoaderKind` binding and typed orchestrator, and preserves the 75-second metadata / 600-second installer HTTP / 600-second Java split. The host loader module is a thin progress-event adapter. |
| **Phase 3: Frontend contribution registry** | **Implemented.** Typed contracts, validation, the frozen registry, and the explicit static composition root are in `src/core/contributions.ts`, `src/app/composition.ts`, and `src/app/`. Pages, navigation, five settings sections, and a launch-navigation instance action are mounted. `AppPageContext` exposes injected host services and availability-filtered slot lists. Phase 5 adds a typed overlays slot used by the accounts feature and registers every slot through the same registry/boundary model. |
| **Architecture checker and CI** | **Implemented Phase 5 strict gate; CI verification pending.** `scripts/check-architecture.mjs`, `scripts/test-architecture-checker.mjs`, and `scripts/test-frontend-contracts.mjs` run in the `flint-release.yml` `prepare` job, followed by `node scripts/check-architecture.mjs --phase 5`. The checker enforces exact Tauri adapter/import rules with no phase-expiring exceptions, exact nineteen-package Cargo metadata/internal/external dependency allowlists and cycles, all workspace production sources, typed provider ports, one shared actual-registry `ProviderRuntimeState`/`ProviderCatalog`, the sole `ModSource`/`LoaderKind` bindings, thin host event adapters, overlay and host-service contracts, stale-request guards, shim/alias bans, and the 30-command/28-feature-command gate map. GitHub Actions is the final authority; a green run is still pending. |
| **Phase 4: Workspace/module extraction and Rust feature lifecycle** | **Implemented.** The workspace contains nineteen crates sharing one version source. `ProviderRuntimeState` supplies actual registries to both provider-backed commands and the feature lifecycle catalog; `feature_host.rs` is the explicit feature composition root. Startup, managed runtime state, availability IPC/filtering, all 28 feature-command gates, failure propagation, and reverse/idempotent shutdown are implemented. The Phase 4 metadata/source/typed-port checks remain part of the strict Phase 5 gate. |
| **Phase 5: Hardening and documentation closeout** | **Implemented in the working tree; CI verification pending.** Exact frontend Tauri adapters, overlay contributions, shared request/event cleanup hooks, strict loader parsing, shared actual-registry provider lifecycle state, early archive entry limits and preflight validation, bounded/sanitized progress messages, redacted launch-command construction, runtime-failure/shutdown coverage, and the strict `--phase 5` checker gate are implemented. |
| **Public RPC contract** | Existing command names, arguments, success results, event names, and DTOs are preserved. The additive `get_host_availability` command, unavailable-feature gating, bounded/sanitized progress-event messages, and strict mod-loader validation are documented in the authoritative contracts. |

Important current boundary: the application remains a **compile-time modular monolith** that ships as one executable. The workspace root, three foundation crates, seven feature crates, two mod provider crates, and six loader provider crates exist. The feature lifecycle contract/registry/coordinator is composed and started through `feature_host.rs`, with managed availability state, IPC publication, frontend filtering, and command gates. The loader feature crate owns typed orchestration over the injected immutable registry; the host loader module is a thin progress-event adapter, and source-specific loader support lives in the six provider crates. No runtime plugin discovery or arbitrary native loading is part of this baseline.

The mandatory rules in this document apply to the current implementation. [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) defines the completed phase matrix, mandatory implementation rules, and review/delivery process. Genuinely future extensions remain explicitly deferred.

The frontend/backend RPC contracts remain authoritative:

- [`API_CONTRACT.md`](API_CONTRACT.md) defines the base commands, DTOs, events, and behavior.
- [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md) defines the extended account, loader, mod, and save contracts and their events.

Architecture, module boundaries, and internal registries must not rewrite those contracts by implication. If this document and a contract disagree about an RPC name, argument, result, event, or user-visible rule, the contract is the source of truth until all documents and code are intentionally updated together. A public behavior change updates the affected authoritative contract; both RPC contract files are updated only when the shared model, command, or event surface is affected.

Related documents:

- [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) contains mandatory implementation and review rules.
- [`architecture-decision-records/0001-modular-core-and-feature-modules.md`](architecture-decision-records/0001-modular-core-and-feature-modules.md) records the accepted decision.
- [`README.md`](README.md) is the documentation index.

## Purpose

The launcher spans several independent capabilities: local version discovery, installation, launch, accounts, loader installation, mod discovery and management, saves, and settings. The host anchors a workspace with extracted `flint-core`, `flint-feature-api`, `flint-provider-api`, seven extracted feature crates under `src-tauri/modules`, two mod provider crates, and six loader provider crates under `src-tauri/providers`. `flint-core` owns shared policy and typed port contracts. The frontend has a static five-slot contribution registry with snapshot-driven availability filtering and a typed Tauri host-service boundary. Typed mod and loader provider leaves, the generic provider registry, feature lifecycle, host lifecycle/availability integration, and the frontend registry are implemented through Phase 5.

The target architecture introduces explicit boundaries while preserving the current application behavior and release outputs. It is a **compile-time modular monolith**, not an arbitrary runtime plugin system. Features and providers are selected and registered by the application composition root before the app starts.

## Goals

1. Separate stable core policy from feature-specific behavior and from host/framework adapters.
2. Make dependencies directional and reviewable.
3. Replace source- and loader-specific call-site branches with provider contracts and registries.
4. Make feature registration, initialization order, failures, and shutdown deterministic.
5. Replace hard-coded frontend navigation and page composition with typed, compile-time contributions.
6. Preserve the current Tauri command surface, DTO shapes, event names, user behavior, and release/build paths during migration.
7. Keep the implementation original. External references may inform expected behavior only; no names, branding, identifiers, text, or code may be copied from another project.

## Non-goals

- Rewriting user-visible behavior as part of the boundary refactor.
- Loading arbitrary native libraries, downloading executable UI, or executing untrusted plugin code.
- Introducing remote UI modules, dynamic command registration, or a general service locator.
- Changing release tooling paths or binary names as part of the first workspace extraction.
- Moving every helper into a crate. Extraction should follow stable responsibility boundaries, not line counts.
- Adding dynamic plugin machinery before a separate accepted decision and threat model.

## Terminology

| Term | Definition |
| --- | --- |
| **Core** | Shared domain policy and infrastructure used by multiple features, with no knowledge of individual features or the host framework. |
| **Feature** | A cohesive, independently owned capability with a stable ID, versioned contract, initialization entry point, and declared contributions. |
| **Feature descriptor** | Declarative metadata and registration data for a feature. |
| **Feature registry** | The immutable, validated registry that resolves feature dependencies, orders initialization, and tracks lifecycle state. |
| **Provider** | A source-specific or format-specific implementation behind a stable contract, such as a mod catalog provider or a loader provider. |
| **Provider registry** | The validated mapping from stable provider identifiers to provider implementations. |
| **Contribution** | A declarative item owned by a feature and added to a registry slot, such as a navigation item, page, settings section, instance action, service, or provider capability. |
| **Composition root** | The single explicit location where concrete features, providers, and adapters are assembled. Rust and React each have one. |
| **Host** | The Tauri-facing application layer that starts the app, registers commands, adapts framework types, and owns the Rust composition root. |
| **Adapter** | A translation layer at an external boundary. It maps transport or framework types to internal types and delegates behavior. |
| **Runtime plugin** | Code discovered and loaded after the application is built. This is deferred and is not part of the accepted initial target. |

## Current baseline (Implemented in working tree; CI verification pending)

### Backend shape

The Rust backend is a Tauri v2 workspace rooted at `src-tauri`, with the host package plus eighteen extracted crates: three foundation crates, seven extracted feature crates, two mod provider crates, and six loader provider crates. `flint-core` is the shared policy and typed-port layer, `flint-feature-api` owns feature lifecycle contracts, `flint-provider-api` owns generic and typed mod/loader contracts, and the provider crates implement CurseForge, Modrinth, Fabric, Quilt, Forge, NeoForge, LiteLoader, and OptiFine through core ports. `provider_composition.rs` owns one shared actual-registry `ProviderRuntimeState`; the host keeps thin command orchestration and Tauri event adapters, while the loader feature crate owns the sole `LoaderKind` binding. The host feature composition and lifecycle roots are `feature_host.rs` and `lib.rs`.

- `src-tauri/Cargo.toml` is both the host package manifest and the workspace root and is the single Cargo version source: the host `[package] version` equals `[workspace.package] version`, and each non-root member manifest declares `version.workspace = true`. `src-tauri/Cargo.lock` is the single workspace lockfile and carries one version entry for every local workspace package.
- `src-tauri/src/main.rs` is a thin desktop entry point that calls `flint_lib::run()`.
- `src-tauri/src/lib.rs` declares every remaining host Rust module, re-exports `flint_core` policy modules as `crate::{paths, error}`, contains the 30 `#[tauri::command]` wrappers (`launcher_info` and `get_host_availability` ungated, 28 feature commands gated), and registers each exactly once with the compile-time `tauri::generate_handler!` macro.
- `src-tauri/src/models.rs` contains the serializable Rust DTOs. Existing public command/DTO/event contracts are preserved; the host-availability DTO/command and feature-command gating error behavior are documented in `API_CONTRACT.md`.
- Blocking network, filesystem, process, and archive work is generally moved off Tauri async commands with `tokio::task::spawn_blocking`.
- Public command failures are currently returned as display-ready `String` errors. `flint-core::error` maps background-task join failures; the host re-exports it as `crate::error`.

The current Rust module responsibilities are:

| Module | Implemented responsibility |
| --- | --- |
| `crates/flint-core/src/paths.rs` | Application and game directory resolution, settings path, path normalization, safe joining, deduplication keys, and SHA-1 helpers. Public core policy used by the host through `crate::paths`. |
| `crates/flint-core/src/net.rs` | Shared blocking HTTP client creation, HTTPS validation, bounded response reads, atomic downloads, temporary paths, and directory creation. Public core policy consumed directly as `flint_core::net` by core/types that need it; the host does not re-export `net`. |
| `crates/flint-core/src/error.rs` | Conversion of background-task failures to public error strings, re-exported by the host as `crate::error`. |
| `crates/flint-core/src/ports/*` | Object-safe, framework-neutral bounded network, safe workspace/archive, process/Java, progress, logging, deadline, and cancellation contracts, plus the `ReqwestNetwork`, `SafeWorkspace`, `CommandProcessPort`, and `ProcessJavaRunner` adapters. |
| `crates/flint-core/src/minecraft.rs` | Capability-neutral Minecraft workspace, profile-library resolution/download, installer, ZIP check, profile discovery, version comparison, HTTPS normalization, and progress/log/deadline helpers. |
| `crates/flint-feature-api/src/lib.rs` | Framework-neutral feature IDs, data-only descriptors, registration/initializer and runtime contracts, validated deterministic registry, provider-catalog abstraction, lifecycle coordinator/state machine, command gate, and read-only availability snapshot. Host lifecycle composition and IPC/gate integration are implemented in `src-tauri/src/feature_host.rs` and `src-tauri/src/lib.rs`. |
| `src-tauri/src/feature_host.rs` | Host feature composition root: seven explicit feature descriptors/initializers, consumption of the shared actual-registry `ProviderRuntimeState` as the provider catalog, deterministic coordinator startup/snapshot state, poisoned-lock-safe managed runtime state, command gate helper, and idempotent shutdown. |
| `crates/flint-provider-api/src/lib.rs` | Generic versioned provider descriptors, capability bindings, provider requirements, compatibility checks, immutable deterministic provider registry, and unavailable/ambiguous selection errors. Shared `ApiVersion` and `ProviderRequirement` values come from `flint_core::contract` and are re-exported by `flint_provider_api`. The crate also exposes typed mod and loader provider contracts; its non-default `test-ports` feature supplies in-memory fake network, workspace, and Java ports for tests. |
| `provider_composition.rs` | Explicit host composition of one immutable `ProviderRegistry<dyn ModProvider>` and one immutable `ProviderRegistry<dyn LoaderProvider>`, plus one shared managed `ProviderRuntimeState` that owns both actual registries and implements the feature catalog, and exact requirement helpers consumed by feature availability. Concrete provider construction stays only here. |
| `providers/flint-loader-provider-*` | Six typed loader provider crates implementing Fabric, Quilt, Forge, NeoForge, LiteLoader, and OptiFine through `LoaderProvider` and shared core ports; each exposes only concrete provider types/constructors and descriptors while mapping payloads privately. |
| `modules/flint-feature-accounts/src/lib.rs` | Extracted accounts feature: account DTOs, local account store, Microsoft device flow/refresh, account selection/removal, launch identity resolution, and sensitive-value redaction. |
| `modules/flint-feature-vanilla/src/{install,java,version}.rs` | Extracted vanilla feature: remote version listing, verified manifest/client/asset/library downloads, ZIP/JAR handling, Java discovery/validation, and version resolution. `InstallProgressSink` is the framework-neutral progress boundary. |
| `install.rs` | Thin host installer adapter implementing `InstallProgressSink` with `AppHandle` and the unchanged `flint://install-progress` event, delegating to the vanilla feature crate. |
| `modules/flint-feature-launch/src/lib.rs` | Extracted launch feature: LaunchRequest/LaunchResult DTOs, local version resolution, argument/placeholder construction, native extraction, detached process start, command display, log paths, and token redaction. |
| `modules/flint-feature-loaders/src/lib.rs` | Extracted loader feature: Loader DTOs, sole `LoaderKind` provider binding, injected immutable registry selection, typed contexts, Java resolution, vanilla base preparation, 75/600/600 per-operation timeouts, and framework-neutral loader/vanilla progress sink traits. |
| `modules/flint-feature-mods/src/lib.rs` | Extracted mods feature: Mod* DTOs, seven public operations, injected mod-provider registry selection, dependency planning, archive/JAR/sidecar handling, installed-mod management, and framework-neutral install progress sink. |
| `modules/flint-feature-saves/src/lib.rs` | Extracted saves feature crate: SaveInfo/SaveTransferResult DTOs, save discovery, bounded NBT parsing, ZIP import/export/backup, rename, and delete behavior. |
| `modules/flint-feature-settings/src/lib.rs` | Extracted settings feature crate: LauncherSettings DTO/default, settings validation, atomic JSON persistence, and Unix owner-only permissions. |
| `modules/flint-feature-vanilla/src/lib.rs` | Vanilla feature DTOs (`InstanceInfo`, `JavaInfo`, `RemoteVersion`, `InstallProgress`, `InstallResult`) and public module facade. |
| `mods.rs` | Thin host mods adapter implementing `ModInstallProgressSink` and preserving the `flint://mod-install-progress` event while delegating to the feature crate. |
| `loaders.rs` | Thin host loader adapter implementing loader and vanilla progress sinks, preserving `flint://loader-install-progress` and `flint://install-progress`, while delegating to the feature crate. |
| `models.rs` | Host-only serializable request/response/enum DTOs plus re-exports of all extracted feature DTOs. |

The Phase 1 and Phase 2 provider integrations, typed mod and loader provider leaves, host registry injection/adoption, and strict Phase 5 checker enforcement are implemented in the working tree:

- `src-tauri/crates/flint-provider-api/src/lib.rs` provides the shared `Provider` contract, `ProviderCapability`, `ProviderDescriptor`, and `ProviderRegistry`, and re-exports `ApiVersion` and `ProviderRequirement` from `flint_core::contract`.
- The registry is immutable after construction, validates provider/capability/version/API/selection-key constraints, rejects duplicate provider/capability IDs and duplicate capability/selection-key bindings, sorts providers by stable ID, and returns explicit unavailable or ambiguous errors from selection.
- `src-tauri/src/provider_composition.rs` builds and owns one immutable mod-provider registry and one immutable loader-provider registry inside one shared `ProviderRuntimeState`. That same state supplies typed registry access to provider-backed commands and the actual-registry catalog to the feature lifecycle; source-specific mod and loader payload mapping lives in the extracted provider crates. The loader composition registers exactly the six concrete constructors and six keyed requirements.
- `src-tauri/modules/flint-feature-loaders/src/lib.rs` owns the sole `LoaderKind` binding and typed orchestrator. It injects the composed registry, builds typed list/install contexts with separate 75-second metadata and 600-second installer HTTP clients plus the 600-second Java timeout, maps neutral data into unchanged DTOs, and preserves command/event/progress behavior; the host adapter keeps only Tauri sink/event adaptation. The old `loaders/providers.rs` adapter and legacy loader API alternatives have been removed.
- Provider implementations do not depend on one another. Mod and loader providers are validated once in their immutable composition registries and reused through managed state; the feature availability catalog consumes the same real descriptors, requirements, and unavailable-provider state. Provider initialization and shutdown update that shared lifecycle truth.
- All six loader provider crates use the shared core ports and have no parent-module helper imports. The strict checker enforces the exact provider members, manifests, source leaves, dependency allowlists, registry construction location, the sole loader-feature `LoaderKind` binding, and the thin host loader event adapter.

The workspace foundation, `flint-core`/`flint-feature-api`/`flint-provider-api`, the seven extracted settings/saves/accounts/vanilla/launch/mods/loaders feature crates, the two extracted mod provider crates, and the six extracted loader provider crates are implemented. `flint-feature-api` contains the feature descriptor, validated registry, lifecycle coordinator, and availability snapshot contract. The Tauri host composes and starts it through `feature_host.rs`, exposes `get_host_availability`, filters frontend contributions, and gates all 28 existing feature commands before I/O. The shared core policy and typed ports are implemented, including generic Minecraft profile/installer support. Mod and loader providers use the shared ports behind immutable registries held in the shared `ProviderRuntimeState`; that same state drives provider availability, loss, and shutdown. Existing command names, arguments, success results, and event names are preserved apart from the additive availability command, gating behavior, strict mod-loader validation, and bounded/sanitized progress text.

### Frontend shape

The frontend is a React 18 and TypeScript application rooted at `src`. The contribution registry and Phase 5 typed-host boundary are implemented without changing the public RPC contract:

- `src/main.tsx` creates one typed `HostServices` instance with `createTauriHostServices()` and passes it to `src/app/App.tsx`. It is the only frontend composition entry allowed to import the concrete Tauri service factory.
- `src/core/api/tauriInvoke.ts`, `src/core/events/tauriEvents.ts`, `src/core/platform/tauriDialogs.ts`, `src/core/platform/tauriOpener.ts`, and `src/core/platform/tauriWindow.ts` are the only five files permitted to import `@tauri-apps/*` packages, each with exactly one reviewed package specifier. No phase-expiring Tauri import exception remains.
- `src/core/platform/hostServices.ts` defines the Tauri-free `HostServices` contracts for typed event subscription, directory/file/save dialogs, path/URL opening, and window controls. Feature code receives these services through `AppPageContext`; it does not import Tauri or the concrete adapters.
- `src/core/events/useHostEvent.ts` owns subscription cleanup for late listener registration and unmount. `src/core/requests/requestGeneration.ts` and `useRequestGeneration.ts` provide the shared stale-request generation guard used by request-bearing frontend components.
- `src/api.ts` remains the typed command client. It reaches IPC only through the reviewed `tauriInvoke` adapter; feature modules continue to call the shared typed client.
- `src/core/contributions.ts` defines typed `FeatureDescriptor` contracts for pages, navigation, settings sections, instance actions, and overlays. It validates stable IDs, feature requirements/cycles, page targets, duplicate contribution IDs across slots, ordering, and component/handler references, then builds and deeply freezes a deterministic registry.
- `src/app/composition.ts` is the explicit static frontend composition root. It imports the approved feature modules and exports the frozen `frontendRegistry`; there is no glob auto-discovery or remote contribution manifest.
- `src/app/AppSidebar.tsx` renders navigation contributions and resolves the active page by canonical `targetPageId`/page contribution ID.
- `src/core/ContributionBoundary.tsx` provides the typed per-contribution error boundary used by the shell, settings sections, instance actions, and overlays. `src/app/App.tsx` owns shared app-level state, populates `AppPageContext` from `src/core/app-context.ts`, injects host services, filters the registry against the availability snapshot, and renders the active page and visible overlays through contribution boundaries.
- `src/modules/` contains the current feature modules: launch, versions/downloads, saves, settings, about, and accounts. The page modules expose pages/navigation; settings contributes five settings sections, launch contributes an instance action, and accounts contributes the account-dialog overlay instead of a shell-specific export.
- The settings feature owns five section components under `src/modules/settings/sections/` and registers them in descriptor order. `SettingsView` renders the registered sections, contribution validators, the outer fieldset, status/error panel, and action bar. Section-local touched state preserves field-level validation displays.
- The launch feature contributes a launch-navigation instance action. `src/core/InstanceActionRenderer.tsx` builds the typed `InstanceActionContext` for the versions page header and renders visible/enabled registered actions.
- `src/types.ts`, `src/utils.ts`, and the remaining generic components remain shared modules at `src`; `src/utils.ts::parseLoaderKind` accepts only the exact canonical loader IDs and rejects aliases, substrings, and unsupported values.
- The frontend has no router. The host-availability client validates the read-only snapshot and filters navigation, pages, settings sections, instance actions, and overlays by host/feature readiness; malformed snapshots fail closed.
- The strict Phase 5 checker combines Rust and TypeScript source analysis with Cargo metadata. It enforces the exact five Tauri adapters and importer set, no Tauri dynamic/aliased imports, no production shim/alias tokens, overlay contracts, stale-request generation use, provider-runtime integration, and the full workspace/provider port rules. GitHub Actions is the final verification authority and is not yet recorded as green.

The registry, host lifecycle, availability filtering, provider lifecycle, typed frontend adapters, and Phase 5 hardening are implemented in the working tree. No temporary migration exception or duplicate old/new dispatch path remains in the reviewed production source.

### Build and release constraints (Implemented)

The current build and release automation assumes these paths and identities:

- The Cargo package root and Tauri application root are `src-tauri`.
- The Cargo lockfile is `src-tauri/Cargo.lock`.
- The Tauri configuration is `src-tauri/tauri.conf.json`.
- The frontend build output is repository-root `dist`, referenced by `frontendDist: "../dist"` in the Tauri configuration.
- The default Cargo target directory is `src-tauri/target`; the `flint-release.yml` release workflow reads executables from `src-tauri/target/<rust-target>/release/flint` or `flint.exe`.
- Release/version tooling inspects the host and `[workspace.package]` versions in `src-tauri/Cargo.toml`, the version in `src-tauri/tauri.conf.json`, every local workspace package entry in `src-tauri/Cargo.lock`, and the root package metadata. It requires non-root member manifests to use `version.workspace = true` and exactly one lock block for each of the nineteen local workspace packages, all equal to the generated metadata. `apply-version` updates both Cargo manifest version fields and every local lock entry while the release file list remains the documented five paths (`VERSION`, `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`).
- `build.rs` performs Tauri code generation and adds the Windows GNU cdylib link argument.
- The library crate is named `flint_lib` and keeps staticlib, cdylib, and rlib crate types.
Current architecture-checker and CI facts:

- `scripts/check-architecture.mjs`, `scripts/test-architecture-checker.mjs`, and `scripts/test-frontend-contracts.mjs` are implemented.
- The `prepare` job in the `flint-release.yml` release workflow installs frontend dependencies, runs both contract test files, and then runs `node scripts/check-architecture.mjs --phase 5`.
- All build-matrix jobs depend on `prepare`, so the contract tests and Phase 5 strict gate must pass before any build starts.
- The checker invokes Cargo only for `cargo metadata` workspace/package/dependency truth and fails closed when it is unavailable or invalid. Its Rust scope covers all nineteen production source roots, exact package/internal/external dependency rules and cycles, one root lockfile and version/resolver coherence, module/provider import and selection boundaries, typed provider context ports, forbidden raw provider APIs and feature/host imports, the single shared actual-registry `ProviderRuntimeState` in `provider_composition.rs`, exact six loader requirements/constructors, and the sole `LoaderKind` binding in `src-tauri/modules/flint-feature-loaders/src/lib.rs`.
- The frontend scope enforces the exact five Tauri adapters/importers, host-service injection, overlay contributions, shared stale-request guards, exact loader parsing, and bans migration shims/aliases. GitHub Actions is the final verification authority; no green hosted run is claimed here.

Consequences for migration:

1. Keep `src-tauri/Cargo.toml` as the implemented workspace root and single Cargo package-version source: keep the host `[package] version` equal to `[workspace.package] version` unless the release tooling and documented paths are changed in the same scoped change. Non-root members must continue to declare `version.workspace = true`.
2. Keep the host package `flint` and its `src-tauri/src` location. A workspace root can also be a package root; member crates can live under `src-tauri/crates`, `src-tauri/modules`, and `src-tauri/providers`.
3. Keep one lockfile at `src-tauri/Cargo.lock` and avoid member-local lockfiles.
4. Keep the default target directory and executable names stable until the release workflow and validation scripts have been deliberately updated.
5. Keep `main.rs` as the executable entry and `flint_lib::run()` as the library entry unless a release-path migration is explicitly approved.
6. Do not move or redefine the Tauri configuration or frontend output path during the architecture migration.

## Architecture boundaries (Implemented through Phase 5; future extensions deferred)
The layer model is normative and the migration boundaries are present in the working tree: the nineteen-crate workspace, generic provider API/registry, seven extracted feature crates, two extracted mod providers, six extracted loader providers, shared actual-registry provider lifecycle state, host feature lifecycle/availability integration, the five-slot frontend registry, and the typed frontend host-service boundary. Phase 5 strict checker enforcement is implemented; GitHub Actions remains the final verification authority. The table below describes the mandatory ownership rules, not an optional future layout.

### Layer model

```text
React entry (`src/main.tsx`)
  -> frontend composition root
  -> frontend core registry/API/event clients + shared app shell
  -> frontend feature modules and their contribution declarations

Tauri host (`src-tauri/src`)
  -> thin command adapters
  -> Rust composition root
  -> feature registry and provider registry
  -> feature module services
  -> provider implementations behind API traits
  -> flint-core
```

Arrows mean "may depend on". The dependency rules below are stricter than the diagram and are mandatory.

| Layer | Responsibility | May depend on | Must not depend on |
| --- | --- | --- | --- |
| `flint-core` | Shared domain policy, path/network/process/error primitives, object-safe transport/workspace/process/progress/logging/deadline/cancellation port contracts, and stable utilities that are independent of any one feature. | Standard library and vetted general-purpose crates. | Tauri host, frontend code, feature modules, provider implementations, feature descriptors, or product-specific branches. |
| Feature API crate (`flint-feature-api`) | Feature IDs, versions, descriptors, registry/lifecycle contracts, provider-catalog abstraction, command gating, and availability DTOs. | `flint-core` where a shared primitive is necessary, plus stable general-purpose libraries such as `semver` and `serde`. | Host, concrete modules, concrete providers, Tauri. |
| Provider API crate(s) | `ModProvider`, `LoaderProvider`, provider descriptors, request/context/result/error contracts, and progress/capability ports. | `flint-core` only where a shared primitive is necessary. | Host, concrete modules, concrete provider implementations, Tauri. |
| Feature modules | User-facing domain orchestration, feature-owned state and services, contribution declarations, and feature initialization. | Core, feature API, provider API, stable external libraries, and explicitly published cross-feature APIs. | Host, Tauri, another module's private files, provider implementations, React, or dynamic global state. |
| Provider implementations | Source- or loader-specific external I/O, payload mapping, and provider-specific planning/execution behind a provider API. | Provider API, core network/path/process policy, and provider-specific leaf libraries. | Host, Tauri, feature module internals, any other provider, provider-to-provider dependencies, or UI code. |
| Tauri host/adapters | App startup, command registration, DTO conversion, Tauri event/process/dialog adaptation, dependency injection, and the Rust composition root. | All approved core, API, module, and provider surfaces, plus Tauri. | Nothing is allowed to depend back on the host. |
| Frontend `core`/`shared` | API/event transports, registry primitives, generic UI, hooks, error presentation, and stable cross-feature types. | Framework/library code and its own public contracts. | Concrete feature modules or `app` code. |
| Frontend feature modules | UI for one capability, its local state, typed contribution registrations, and calls through the shared API client. | Frontend `core`, `shared`, and its own subtree/public surface. | Any other frontend feature module (including its public entry), remote code, direct `invoke` calls that bypass the API layer, or framework adapters from a feature's domain logic. |
| Frontend `app` | The single composition root, root providers, shell wiring, and explicit module list. | Every approved frontend module and core/shared surface. | Business logic that belongs to a feature. |

### Workspace layout (Implemented)

The workspace root and extracted crates are implemented at the following paths. Names are stable architecture contracts; new crates require the explicit composition and allowlist updates described below.

```text
src-tauri/
  Cargo.toml                      # host package + workspace root; [workspace.package] version source
  Cargo.lock                      # one workspace lockfile; every local package version
  tauri.conf.json                 # existing path
  build.rs                        # host build script
  src/                            # host location
    main.rs
    lib.rs                        # command registration, setup, RunEvent::Exit shutdown
    feature_host.rs               # explicit seven-feature lifecycle composition
    provider_composition.rs       # actual mod/loader registries + shared ProviderRuntimeState
    install.rs
    loaders.rs
    mods.rs
    models.rs
  crates/
    flint-core/
    flint-feature-api/
    flint-provider-api/
  modules/
    flint-feature-accounts/
    flint-feature-launch/
    flint-feature-vanilla/
    flint-feature-loaders/
    flint-feature-mods/
    flint-feature-saves/
    flint-feature-settings/
  providers/
    flint-mod-provider-curseforge/
    flint-mod-provider-modrinth/
    flint-loader-provider-fabric/
    flint-loader-provider-forge/
    flint-loader-provider-liteloader/
    flint-loader-provider-neoforge/
    flint-loader-provider-optifine/
    flint-loader-provider-quilt/
```

The seven feature crates are fixed for this migration. DTO ownership follows the feature/provider contract: feature-specific DTOs stay with their feature, provider payloads stay private to providers, and `flint-core` does not become a dumping ground for serializable types.

The host package keeps the `flint` package name, `flint_lib` library name, `src-tauri` root, single lockfile, Tauri configuration, target directory, and executable names. The composition root explicitly lists features and providers; there is no runtime directory scan for crates or provider libraries.

### Dependency direction rules (Mandatory)

Rust:

1. `flint-core` is the lowest layer. It cannot import the host, feature modules, provider implementations, frontend code, or feature-specific identifiers.
2. Feature and provider API crates may depend only on stable lower-level contracts. They cannot import the host, concrete modules, concrete providers, Tauri, or frontend code.
3. A feature module may depend on `flint-core`, feature/provider API crates, and its own public modules. It cannot import the Tauri host or a private module of another feature.
4. Cross-feature use is allowed only through a deliberately exported public contract that is declared as a dependency and cannot create a feature cycle. Direct paths into another feature's implementation are forbidden.
5. A provider implementation may depend on provider/core APIs and its own private submodules. It cannot import feature module internals, the host, Tauri, or another provider's private code.
6. The host may depend on features, providers, APIs, and core. The reverse direction is forbidden: no `core -> host`, no `module -> host`, no `provider -> host`, and no `module -> private module` import.
7. Tauri types, `AppHandle`, event emission, window APIs, and command macros stay in the host/adapters layer. They must not leak into core, API, feature, or provider crates.
8. Only the host composition root may instantiate concrete feature and provider implementations and inject them into registries.
9. A module's public API is the minimum needed by the composition root or an approved contract. Private implementation modules remain private.
10. Cargo manifests and TypeScript imports define actual edges, but neither Cargo nor TypeScript inherently enforces an arbitrary layer allowlist. The strict Phase 5 checker in the release workflow `prepare` gate enforces frontend manifest/composition/import and exact Tauri-adapter boundaries, module/provider import and selection boundaries, forbidden std/process/fs/net/reqwest/Tauri patterns in extracted provider leaves, exact feature and provider dependency/source/composition boundaries, all nineteen workspace production sources, exact Cargo internal/external dependency rules and cycles, typed provider context ports, one shared actual-registry `ProviderRuntimeState`, the sole loader-feature binding and thin host loader event adapter, exact six loader requirement/constructor checks, forbidden feature Tauri/host-private imports, and shim/alias bans. An allowlist change requires explicit review.

Frontend:

1. `src/app` is the only layer allowed to import concrete feature modules for assembly. `src/core` and `src/shared` cannot import `src/app` or concrete feature modules.
2. A frontend feature module may import only frontend core/shared APIs and its own files. It cannot import another feature module at all, including that feature public entry, and cannot reach into another feature store.
3. Cross-feature UI composition, state, actions, and navigation are performed through typed core contracts, shell services/contexts supplied by the app composition root, or contribution slots. No feature imports another feature module as a cross-feature API.
4. Domain/API behavior belongs in feature services or the shared API client, not in generic shared components.
5. The composition root creates and freezes the frontend registry before rendering. Components read the registry and typed shell services; they do not mutate global registration state at render time.

### Architecture boundary checker (Phase 5 strict gate active)

The checker is implemented at `scripts/check-architecture.mjs`, with automated coverage in `scripts/test-architecture-checker.mjs` and frontend contract tests in `scripts/test-frontend-contracts.mjs`. It is phase-aware, fail-closed, and reads checked-in reviewed allowlists. Broad directory/package wildcards and phase-expiring exceptions are not accepted in the Phase 5 production policy.

Current integration status:

- The release workflow `prepare` job installs frontend dependencies, runs `node --test scripts/test-architecture-checker.mjs scripts/test-frontend-contracts.mjs`, then runs `node scripts/check-architecture.mjs --phase 5`.
- All build-matrix jobs depend on `prepare`; the architecture/frontend contract tests and Phase 5 checker are the release-prepare gate.
- GitHub Actions is the final authority. These documents describe the working-tree implementation and pending verification, not a claimed CI-green result.

#### Rust checks

- The Phase 5 gate combines static Rust source analysis with frontend AST checks and invokes Cargo only for workspace/package/dependency metadata. It masks comments/strings, scopes test-only modules correctly, scans every production Rust source under all nineteen crates, enforces reviewed module/provider import and selection boundaries, and rejects forbidden std/process/fs/net/reqwest/Tauri patterns and direct `File`/socket/process APIs in provider leaves.
- Cargo checks require the exact nineteen-package set, one root lockfile, version/resolver coherence, exact internal edges, exact normal/build/dev external dependency allowlists, and no dependency cycles. Cargo-unavailable or invalid-metadata failures are fail-closed.
- Provider checks require typed `LoaderProvider`/`ModProvider` contracts and typed context-port use. `ProviderRuntimeState` must own both actual immutable registries, implement the sole `ProviderCatalog`, be created once in `lib.rs`, and be passed into `feature_host.rs`; a metadata-only duplicate catalog or independent provider state is forbidden.
- The checker requires the sole `ModSource` binding in the mods feature, the sole `LoaderKind` binding in the loader feature, exact six loader descriptors/requirements/constructors, and thin Tauri event adapters.
- Phase 5 source checks reject legacy phase-expiry policy tables/flags, metadata-only provider catalogs, special frontend surfaces, loader substring/alias helpers, provider-context aliases, and dead loader display metadata.
- Manifest checks reject dependencies that would bypass approved ports; source checks catch standard-library or macro paths Cargo cannot see. The checker is not a security sandbox: review still rejects obfuscation, FFI, or unsafe escape hatches.

#### TypeScript checks

- The checker uses the installed TypeScript compiler API to parse every `.ts`/`.tsx` file and collect static imports, re-exports, side-effect imports, `import type`, `require` forms, and dynamic `import()` calls.
- Tauri package imports are allowed only at these exact file/specifier pairs: `src/core/api/tauriInvoke.ts` -> `@tauri-apps/api/core`; `src/core/events/tauriEvents.ts` -> `@tauri-apps/api/event`; `src/core/platform/tauriDialogs.ts` -> `@tauri-apps/plugin-dialog`; `src/core/platform/tauriOpener.ts` -> `@tauri-apps/plugin-opener`; and `src/core/platform/tauriWindow.ts` -> `@tauri-apps/api/window`. Every other file, extra specifier, alias, dynamic/require/re-export form, or unauthorized concrete-adapter import fails.
- `src/main.tsx` is the only composition entry allowed to import/call `createTauriHostServices`; `src/core/platform/tauriServices.ts` is the sole importer of the event/dialog/opener/window adapters; `src/api.ts` may import only the reviewed `tauriInvoke` adapter. Features consume `HostServices` through `AppPageContext`.
- Frontend checks require the overlay contribution contract on every feature descriptor, registry ordering/freeze/duplicate validation, account-dialog overlay registration and visibility, availability filtering of overlays, shared stale-request generation guards, and exact `parseLoaderKind` behavior.
- Frontend boundaries remain strict: `src/app` is the only assembly layer allowed to import all concrete feature modules; `src/core`/`src/shared` cannot import `src/app` or `src/modules`; a feature module may import only its own subtree plus core/shared APIs. No feature may import another feature module, including its public entry.
- Dynamic imports are allowed only with one string-literal, relative, bundler-resolvable specifier inside the importing feature subtree or an explicitly reviewed local chunk. Variable/template specifiers, absolute or remote paths, package-specifier contribution loaders, `eval`, and `new Function` are rejected.
- External packages remain allowlisted. Adding a package to a feature requires dependency review, not merely an import that happens to compile.

#### CI gate

- CI failure blocks completion; there is no warn-only mode.
- The current gate is the release workflow `prepare` job at Phase 5. It covers architecture/checker tests, frontend contract tests, exact Tauri adapter/importer rules, exact nineteen-package Cargo metadata and all-workspace source rules, dependency allowlists/cycles, typed provider ports, shared provider runtime truth, strict frontend contribution/stale-request/shim checks, and the host 30-command/28-feature-command gate map.
- Any allowlist change is an architecture change and must be reviewed as such.
- A green hosted run has not yet been recorded in this repository documentation; do not describe the working tree as CI-verified.

## Rust feature contract and lifecycle registry (Implemented)
Status: **Implemented in the working tree; CI verification pending.** The crate provides validated feature/contribution IDs, data-only descriptors with separate registration/initializer and runtime-handle contracts, a deterministic feature registry, a provider-catalog abstraction, the lifecycle state machine/failure propagation/gating/shutdown coordinator, and the read-only availability snapshot DTO. The host composes seven explicit features in `feature_host.rs`, consumes the shared actual-registry `ProviderRuntimeState`, starts through Tauri setup, owns poisoned-lock-safe managed runtime state, exposes the ungated `get_host_availability` command, gates all 28 feature commands, filters frontend contributions from the snapshot, and invokes idempotent shutdown on `RunEvent::Exit`. Phase 5 lifecycle/provider-runtime, failure, reverse-shutdown, and gating coverage is implemented subject to CI verification.

The implemented `FeatureAvailabilitySnapshot` serializes `revision`, `hostState`, `lastFailure`, and lexicographically sorted `features` entries with `featureId`, `state`, and `reason`. Lifecycle states serialize as snake_case; provider identities and provider error details are excluded.

A feature is registered by declaration and has one initialization entry point. The implemented feature contract uses the following simplified shape; descriptors remain data-only, while the initializer is registered separately:

```rust
pub struct FeatureDescriptor {
    pub id: FeatureId,
    pub feature_version: Version,
    pub api_version: ApiVersion,
    pub startup_policy: StartupPolicy,
    pub requires: Vec<FeatureRequirement>,
    pub provider_requirements: Vec<ProviderRequirement>,
    pub contributions: Vec<ContributionDescriptor>,
}

pub struct FeatureRequirement {
    pub id: FeatureId,
    pub version_req: VersionReq,
}

pub enum StartupPolicy {
    Required,
    Optional,
}
```

### Descriptor fields

| Field | Rule |
| --- | --- |
| **Stable feature ID** | Lowercase, dot-separated semantic namespace such as `flint.mods` or `flint.loaders`. Each segment matches `[a-z][a-z0-9-]*`; at least two segments are required. IDs are permanent contract identifiers, never display labels, and are never reused after removal. Duplicate registration is forbidden. |
| **`feature_version`** | A semantic version for the feature's own implementation and public behavior. Breaking behavior requires a major version change according to the project's contract policy; compatible additions use minor; fixes use patch. This is distinct from both `api_version` and `provider_version`. |
| **`api_version`** | The version of the host feature API against which the module was built. The host exposes one feature API version at startup. A feature is compatible when its API major equals the host major and its minor is not newer than the host minor. An incompatible feature is rejected before initialization. |
| **`startup_policy`** | `Required` or `Optional`. Failure propagates to the failed feature plus all transitive dependents. If that affected set contains any `Required` feature, the host takes the fatal startup/runtime path. An `Optional` failure may leave the host `Degraded` only when the affected set contains no `Required` feature and every `Required` feature remains `Ready`. A `Required` feature must never remain `Ready` if any required dependency/provider is not `Ready`. This is declarative lifecycle metadata, not a feature switch. Core service failures are always fatal. |
| **Dependencies (`requires`)** | Declared feature IDs with semantic-version requirements. All dependencies are required in the first implementation; optional feature dependencies are not part of the initial contract. Unknown dependencies, unmet requirements, duplicate declarations, and cycles are registration failures. |
| **Provider requirements** | Stable capability ID, required capability API version, a semantic-version constraint on the provider implementation, and an optional selection key bound to that capability. Requirements are validated against active providers before feature initialization. A feature must not discover providers by scanning directories or selecting the first map entry. |
| **Initialization entry point** | An explicit function or trait object entry point invoked exactly once after all descriptors validate and dependencies are ordered. It receives a typed feature context for services, registries, progress/event ports, cancellation/deadline, and path/network/process policy. It must not use Tauri globals, frontend globals, or a service locator. |
| **Contributions** | Declarative, namespaced service/provider/event/capability contributions consumed by the backend registry. Contributions contain stable kinds, requirements, and feature-prefixed backend registration IDs. Backend registration IDs are globally unique across the backend registry. Registration itself performs no I/O and starts no threads. Tauri commands are not dynamically added by this field because command registration is compile-time. |

Initialization is registered separately from the descriptor as a `FeatureInitializer`; a successful initializer returns a runtime handle with lifecycle methods such as `health` and `shutdown`. Initialization must be atomic from the registry's perspective: if it returns an error after acquiring resources or starting workers, the feature itself must release those partial resources before returning the error. The registry cannot infer a failed feature's partial state. Keep the lifecycle small and explicit; do not invent a general-purpose dependency-injection framework.

### Feature ID and version rules

- IDs are compared exactly after validation; no case-insensitive aliases, display-name matching, or auto-generated IDs.
- A renamed capability keeps the old ID as a documented alias only during a migration and must update every contribution and contract reference in the same change. Prefer adding a new versioned capability over silently changing semantics.
- `feature_version` describes the feature implementation; `api_version` describes the feature contract boundary; `provider_version` describes a provider implementation. Do not overload one field for another.
- Provider IDs follow the same stable-ID discipline and are distinct from feature IDs.
- Backend registration IDs are globally unique across the backend registry and prefixed with the owning feature ID. Frontend contribution IDs are a separate namespace and use the single global-across-all-slots rule in the frontend section.

## Rust feature registration and lifecycle (Implemented)
Status: **Implemented in the working tree; CI verification pending.** `flint-feature-api` validates feature metadata before side effects, builds a deterministic dependency graph and topological order, coordinates provider/feature startup and failure propagation, exposes gated runtime handles, shuts down in reverse order, and produces the availability DTO. The Tauri host owns/starts the coordinator, publishes the snapshot through the ungated availability command, gates feature commands, filters frontend contributions, and receives exact mod/loader availability from the shared `ProviderRuntimeState`. Provider registry construction failure, provider loss, Required/optional propagation, command gating, and idempotent shutdown are integrated with the same provider lifecycle truth used by commands.

The feature registry is built from explicit composition-root declarations. It validates all metadata before feature side effects, initializes in deterministic order, exposes only active contributions, and is immutable after startup.

### Construction and validation

1. Collect feature descriptors, provider descriptors, and contribution manifests from the explicit composition root.
2. Validate ID syntax, semantic versions, API versions, startup policies, backend registration IDs, provider capability bindings, and contribution declarations.
3. Reject duplicate feature IDs, provider IDs, backend registration IDs, and service IDs. Reject duplicate capability IDs within one provider and malformed duplicate selection-key bindings within one capability. Never allow last-registration-wins.
4. Resolve each feature dependency and each provider capability requirement structurally. Record unmet requirements against the declaring feature; effective criticality is then determined from the failure-propagated affected set, not from the failed feature in isolation.
5. Construct the **feature dependency graph only** and detect cycles before invoking feature initialization. Providers are independent leaves and are not part of a provider-to-provider dependency graph.
6. Topologically order features. Among currently ready feature nodes, break ties by lexicographically ascending stable feature ID.
7. Validate provider descriptors independently and determine provider initialization order by lexicographically ascending `ProviderId`.
8. Do not rely on hash-map iteration, object insertion order, filesystem enumeration, or registration order for either feature or provider ordering.

No feature or provider initialization entry point runs if construction/validation fails. A malformed composition is a startup/configuration failure, not a partially valid app.

### Startup stages and readiness

Window/process creation is a transport concern; **application readiness** is a separate state. A Tauri window may exist during startup or in a diagnostic startup-failure mode, but it must not publish a `Ready` app state or active feature contributions before the stages below complete.

| Stage | Behavior | Failure semantics |
| --- | --- | --- |
| **0. Composition validation** | Load explicit manifests; validate IDs, feature dependency cycles, provider descriptors/capability bindings, contribution declarations, and structural provider requirements. | Fatal startup/configuration failure. No provider or feature initialization occurs. The host enters `StartupFailed` or exits after reporting diagnostics. |
| **1. Core/bootstrap initialization** | Initialize shared path, network, task, logging, and other core services required by provider/feature infrastructure. | Fatal. No feature commands may report ready; successfully constructed host resources are cleaned up and the host enters `StartupFailed` or exits. |
| **2. Provider initialization** | Initialize independent provider leaves in lexicographic `ProviderId` order after descriptor/capability validation. Each provider becomes `Available` only after successful initialization. | A failed provider becomes `Failed`/unavailable for selection; it does not create a provider dependency cycle or a separate provider graph. Features requiring it follow their own `Required`/`Optional` policy in Stage 3. |
| **3. Feature initialization** | Visit features in deterministic topological order. Provider selection uses the full requirement predicate and active provider state. A feature whose dependency or provider requirement failed/was skipped is marked `BlockedDependency`, and its init is not called. Successful init produces a runtime handle and activates that feature's contributions. | A failure first marks the feature `Failed` and all transitive dependents `BlockedDependency`, activates none of their contributions, and prevents their init. If the affected set contains any `Required` feature, enter the fatal `StartupFailed` path with cleanup. If it contains only `Optional` features and every `Required` feature remains `Ready`, continue in `Degraded`. |
| **4. Freeze and availability publication** | If no fatal failure occurred, freeze registries and publish host state and the read-only active-feature snapshot to the shell. | `Ready` when every `Required` feature and all optional features are ready. `Degraded` only when every `Required` feature remains `Ready` and one or more affected `Optional` features are `Failed`/`BlockedDependency`; any affected `Required` feature makes this a `StartupFailed` path instead. |
| **5. Running** | Commands and UI operate against ready feature runtimes, a validated active provider set, and active contributions. | A command for a non-ready feature or unavailable provider returns the documented unavailable error. No silent fallback. |

Rules for Stage 3 and later:

- A feature's contributions are staged during descriptor validation but become visible/active only after that feature reaches `Ready`. Contributions from `Failed`, `BlockedDependency`, `Starting`, or `Stopped` features are not visible.
- Failure propagation always includes the failed feature plus all transitive dependents. Mark the failed feature `Failed`, its transitive dependents `BlockedDependency`, deactivate all affected contributions, and skip initialization of affected dependents.
- Effective criticality is determined from that entire affected set, never from the originally failed feature alone. If the affected set contains any `Required` feature, the host takes the fatal `StartupFailed` path: stop starting further features, do not initialize affected dependents, and clean up already initialized state.
- If the affected set contains only `Optional` features and every `Required` feature remains `Ready`, the host may enter `Degraded`. Independent ready features stay active; required behavior is not silently routed to another feature.
- A `Required` feature must never remain `Ready` when any required dependency or provider is not `Ready`. Such a state is a required failure and follows the fatal path.
- A failed initialization is atomic from the registry's perspective. The feature must clean up all partial resources before returning the error; the registry records the failure and does not hold a runtime handle for it.
- The host exposes `get_host_availability` as an ungated, contract-documented command. The frontend receives the read-only availability snapshot and filters navigation, pages, settings sections, instance actions, and overlays by the owning feature active state.

### Runtime failure transitions

Runtime failures use the same propagated affected-set rule as startup failures. Feature states never include a `Degraded` state.

- **Affected-set propagation:** mark the failed feature `Failed`; mark every transitive dependent `BlockedDependency`; hide all affected contributions and return unavailable errors for affected commands.
- **Optional-only affected set:** if the affected set contains no `Required` feature and every `Required` feature remains `Ready`, transition the host to `Degraded`. Independent ready features stay `Ready`. If any affected feature is `Required`, this branch is invalid.
- **Any affected `Required` feature:** transition the host to `RuntimeFailed`. No `Required` feature may remain `Ready` when any required dependency/provider is not `Ready`. New work is rejected; affected contributions are hidden; the shutdown coordinator moves the host through `Stopping` to `Stopped`, or the process exits after diagnostics. No fallback or partial-recovery state is presented.
- **Provider runtime loss:** surface the loss as feature failures for all features whose selected capability requirement no longer has an active provider. Propagate each failure through its transitive dependents, then apply the affected-set criticality rule above.
- **Normal exit:** transition `Ready` or `Degraded` through `Stopping` to `Stopped` after reverse-order feature/provider/core shutdown.

Invariant: `Degraded` is valid only when every `Required` feature remains `Ready` after failure propagation. Any failure whose transitive affected set contains a `Required` feature is fatal: at startup it enters `StartupFailed`; at runtime it enters `RuntimeFailed`, then `Stopping`, then `Stopped` or process diagnostics/exit. If the process remains open in diagnostic mode, every executable feature command is gated and unavailable.

### Unavailable feature commands

Tauri command registration is compile-time, so adapters for an inactive feature may still exist. Every adapter that depends on a feature or provider must:

- look up the current runtime state;
- execute only when the feature is `Ready` and its required dependencies/providers are ready;
- otherwise return a concise unavailable-feature error with the stable feature ID and a non-secret reason;
- avoid direct filesystem/network access, fallback provider selection, or partial work in the unavailable path.

Current public errors are strings. Preserve the display-ready string shape unless the API contract intentionally adds a structured error. The error text is an adapter mapping; internal lifecycle states remain typed.

### Duplicate and dependency failures

- Duplicate feature, provider, backend registration, service, or provider-capability IDs are startup/configuration failures in their defined scopes.
- Duplicate selection-key bindings within one capability are malformed descriptor/configuration failures. Multiple providers satisfying the same full requirement produce an explicit ambiguous-selection result.
- A dependency on an unregistered feature or unavailable provider capability is fatal for a `Required` feature and yields `Failed`/`BlockedDependency` for an `Optional` feature as defined above.
- An unsupported feature API major or a feature API minor newer than the host is a startup/configuration failure.
- A provider selection with zero matching providers is unavailable; more than one full-predicate match is ambiguous; neither case may choose by iteration order or fallback.
- A feature dependency cycle is reported with the participating IDs and an edge path. Providers do not form dependency graphs or provider cycles.
- No partial registration is accepted as success.

### Health, states, and shutdown

Host states are at least `Starting`, `Ready`, `Degraded`, `StartupFailed`, `RuntimeFailed`, `Stopping`, and `Stopped`. Feature states are at least `Starting`, `Ready`, `Failed`, `BlockedDependency`, and `Stopped`; there is no feature-level `Degraded`. Provider states are at least `Initializing`, `Available`, `Failed`, and `Stopped`. Health is a read-only diagnostic snapshot, not a command that mutates arbitrary global state.

- A feature reports `Ready` only after successful initialization and while its required dependencies/providers remain available.
- Startup and runtime failures follow the affected-set propagation and effective-criticality rules above. `Degraded` is valid only while every `Required` feature remains `Ready`; any affected `Required` feature is fatal and must not be reported as degraded.
- Shutdown is idempotent. Feature runtimes shut down in reverse successful-init order; provider leaves stop after dependent features are no longer using them, and core/bootstrap services shut down last. A component stops owned workers, joins or detaches only what it owns, flushes only its own durable state, and reports errors without panicking.
- Shutdown errors are logged and retained as diagnostics. They never make a partially stopped app appear ready.
- Startup-fatal cleanup uses the same coordinator: shut down features that reached `Ready` in reverse successful-init order, stop successfully initialized provider leaves, then stop core/bootstrap services. If the process remains open in diagnostic `StartupFailed` mode, the coordinator still performs provider/core cleanup; the diagnostic surface must not use stopped feature/provider/core services.
- A feature or provider that failed initialization must clean its own partial state before returning the error and has no runtime handle to shut down later.

### Tauri run-event integration (Implemented)

The Tauri host adapter owns the lifecycle bridge; no feature or provider imports `RunEvent` or an `AppHandle`.

- A single host shutdown coordinator is registered with the Tauri run loop. `RunEvent::Exit` invokes the managed host runtime shutdown; `RunEvent::ExitRequested` is not given final resource ownership.
- On normal final exit, shutdown publishes `Stopping`, invokes feature shutdown in reverse successful-init order, stops provider leaves once unused, shuts down core services, and finally marks `Stopped`. The operation is idempotent and retains shutdown diagnostics.
- On startup-fatal failure, the same coordinator cleans up successfully initialized features, then provider leaves, then core services before the host settles in diagnostic `StartupFailed` mode or exits.
- On runtime-fatal failure, the adapter enters `RuntimeFailed`, starts the same coordinator without waiting for normal exit, and transitions through `Stopping` to `Stopped` or process exit after diagnostics.
- Shutdown must be safe to invoke more than once and must not hold the UI/window thread indefinitely. The adapter may bound or asynchronously observe shutdown tasks while still preventing a false `Ready` state.

### Determinism rules

- Feature initialization order is topological by declared dependencies, with lexicographic feature-ID tie-breaking.
- Provider leaves initialize in lexicographic provider-ID order after descriptor/capability validation.
- Lists exposed to UI have deterministic order: explicit numeric `order`, then stable contribution ID.
- Provider selection uses the full compatibility/selection predicate in the provider section, never `HashMap` iteration or "first match".
- Retry/backoff, if later added, must not make initialization order depend on wall-clock timing.
- All registry mutations are forbidden after freeze; dynamic plugin loading is deferred and cannot use the stable registry as an unreviewed escape hatch.

## Provider contracts (Implemented)

The provider layer separates reusable feature behavior from provider-specific external I/O. The typed mod/loader provider contracts and registries live in `src-tauri/crates/flint-provider-api`, with CurseForge/Modrinth and the six loader implementations extracted under `src-tauri/providers`. `provider_composition.rs` owns both immutable registries inside one shared `ProviderRuntimeState`; the loader feature crate owns the sole `LoaderKind` binding, while the host `src-tauri/src/loaders.rs` adapter only adapts progress sinks/events. Existing serialized `ModSource`, `LoaderKind`, and DTO contracts remain unchanged.

### Versioned provider and capability model

The implemented provider API uses these shapes (simplified; current fields use borrowed static identifiers):

```rust
pub struct ProviderDescriptor {
    pub id: &'static str,
    pub provider_version: &'static str,
    pub provider_api: ApiVersion,
    pub capabilities: &'static [ProviderCapability],
}

pub struct ProviderCapability {
    pub id: &'static str,
    pub api_version: ApiVersion,
    pub selection_keys: &'static [&'static str],
}

pub struct ProviderRequirement {
    pub capability: &'static str,
    pub api_version: ApiVersion,
    pub provider_version_req: &'static str,
    pub selection_key: Option<&'static str>,
}

pub trait Provider {
    fn descriptor(&self) -> ProviderDescriptor;
}
```

`ApiVersion` and `ProviderRequirement` are defined in `flint_core::contract`; `flint-provider-api` re-exports them so provider code can import both from the stable provider API surface.

Definitions:

- `ProviderId`, `CapabilityId`, and `ProviderSelectionKey` are stable lowercase namespaced identifiers. IDs are never display labels or filesystem paths.
- `provider_version` is the semantic version of the implementation.
- `provider_api` is the provider API contract version the implementation was built against.
- Each capability binds its own ID, capability API version, and selection keys. Selection keys are not an independent provider-level list.
- The target Rust `FeatureDescriptor` carries `ProviderRequirement` values. They say which capability a feature needs, which capability API it was compiled against, which provider implementation versions are acceptable, and optionally which selection key is required. The current provider registry already consumes this requirement shape.
- Providers are independent leaves. A provider must not declare or resolve a dependency on another provider; cross-provider coordination belongs in core services or feature orchestration. Provider-to-provider dependency graphs and cycles are therefore not built.
- Registry construction sorts providers by stable `ProviderId`, validates all descriptors/capability bindings, and never depends on `HashMap` iteration, filesystem discovery, or registration order. `ProviderRuntimeState` is the single actual-registry lifecycle source: it implements the feature provider catalog, reports unavailable providers, and updates lifecycle truth on initialization, shutdown, and runtime loss.

### Compatibility and selection predicate

For a feature requirement `R`, selection filters all registered providers by the complete predicate below:

1. **Host provider API:** let `H = (major, minor)` be the host provider API version and `P = (major, minor)` be the provider's `provider_api`. Compatible iff `P.major == H.major && P.minor <= H.minor`.
2. **Capability identity and API:** the provider must expose a capability with `capability.id == R.capability`. For that capability `C`, compatible iff `R.api_version.major == C.api_version.major && R.api_version.minor <= C.api_version.minor`.
3. **Provider implementation version:** the provider's `provider_version` must satisfy `R.provider_version_req` using semantic-version precedence, not string comparison.
4. **Optional selection key:** if `R.selection_key` is `Some(key)`, that exact key must be present in the matched capability's `selection_keys`. If `R.selection_key` is `None`, the key does not participate in filtering.
5. **Active provider state:** the provider must have completed descriptor/capability validation and provider initialization successfully, and must not be stopped or failed.

Selection then requires exactly one provider satisfying the full requirement:

- zero matches: return an actionable unavailable-provider error;
- more than one match: return an ambiguous-selection error;
- never use `HashMap` iteration, filesystem discovery, thread completion order, a first match, or an implicit fallback.

Selection keys are unique within one capability binding. The same key under different capability IDs is qualified by capability ID and does not create cross-capability ambiguity. Duplicate capability IDs within a provider and malformed duplicate key bindings in one capability are descriptor-validation errors. Multiple providers satisfying the same full requirement produce the explicit ambiguous-selection result above. The implemented registry additionally rejects duplicate `(capability, selection_key)` claims across providers during construction; ambiguous selection remains possible for keyless or differently keyed requirements and is returned explicitly.

These checks occur before feature initialization. A missing, incompatible, ambiguous, or failed provider requirement makes a `Required` feature fail startup and makes an `Optional` feature fail or block according to the architecture lifecycle rules.

A provider that fails initialization is unavailable for selection. Features requiring it handle the failure through their `Required`/`Optional` startup policy; provider initialization itself does not create a separate provider dependency graph.
Current implementation status: the host composes one immutable mod-provider registry and one immutable loader-provider registry inside one shared `ProviderRuntimeState`, and provider-backed commands reuse those registries through managed state. `flint-feature-api` defines the framework-neutral provider catalog/lifecycle abstraction; `src-tauri/src/feature_host.rs` receives that same shared state for availability and lifecycle checks. A registry build failure therefore makes the affected feature unavailable/blocked instead of allowing a Required feature to appear ready while its command registry is missing.

### `ModProvider` (Implemented)

`ModProvider` owns source-specific catalog and version metadata behavior while the shared mod feature owns common dependency planning, downloads, archive inspection, and filesystem management.

Implemented method shape:

```text
ModProvider
  search(&ctx, &ModSearchRequest) -> ModSearchResultData
  list_versions(&ctx, &ModListVersionsRequest) -> [ModVersionData]
  version_by_id(&ctx, &ModVersionRequest) -> ModVersionData
  dependency_version(&ctx, &ModDependencyVersionRequest) -> Option<ModVersionData>
```

Current implementation:

- `flint-provider-api` defines the object-safe `ModProvider` trait, provider-neutral project/version/dependency/search records, typed requests, descriptors, stable capability/API/provider/selection-key constants, and the operation-scoped `ModProviderContext`. The two extracted implementation crates define only their concrete provider types, constructors, and descriptors.
- `ModProviderContext` is operation-scoped and exposes a `NetworkPort`, an optional redacted scoped credential, `ProgressSink`, `LoggerPort`, `Deadline`, and `Cancellation`. It contains no Tauri handle or settings-file path. Shared mod code controls validation, dependency graph limits, downloads, archive checks, sidecar persistence, installed-mod management, and progress publication.
- The host builds one registry in `provider_composition.rs` and injects it into provider-backed mod commands. Provider methods perform source-specific HTTP queries and payload mapping and return provider-neutral data; the shared mod feature injects `ModSource` while mapping to host DTOs and owns final game-directory writes.

Implemented context boundaries:

- provider HTTP/credential details use approved core network, safe path, scoped settings/credential, logging, and cancellation/deadline ports;
- final mod placement and sidecar writes remain in the shared mod feature;
- direct Tauri handles, raw windows, unrestricted filesystem roots, unrestricted process runners, and direct settings-file paths are forbidden.

### `LoaderProvider` (Implemented)

`LoaderProvider` owns loader-specific discovery and installation implementation while the shared loader feature owns validation, vanilla preparation, shared clients/safety policy, progress adaptation, and result assembly.

Implemented method shape:

```text
LoaderProvider
  list_versions(ctx: LoaderListContext, request: LoaderListRequest) -> [LoaderVersionData]
  install(ctx: LoaderInstallContext, request: LoaderInstallRequest) -> LoaderInstallOutputData
```

Current implementation:

- `flint-provider-api/src/loaders.rs` defines the typed `LoaderProvider` contract, `LoaderVersionData`, `LoaderInstallOutputData`, list/install requests and contexts, `LoaderTimeouts` (75/600/600 seconds), stable six-provider IDs/capabilities/selection keys, and descriptor helpers. The six `providers/flint-loader-provider-*` crates implement the leaves; `src-tauri/modules/flint-feature-loaders/src/lib.rs` owns the sole `LoaderKind` binding and typed orchestrator while the host `src-tauri/src/loaders.rs` adapter adapts progress sinks/events over the registry created in `provider_composition.rs`.
- The six leaves are the implemented loader providers. They do not depend on one another; registry construction is deterministic and shared loader code performs one registry selection.
- The typed API list context carries network/workspace/progress/logger/deadline/cancellation plus `LoaderTimeouts`. The install context carries separate metadata and installer `NetworkPort`s, safe workspace/`PathPort`, `JavaRunner`, progress/logger, root, deadline/cancellation, and the same timeout bundle. The loader feature crate constructs these typed contexts from reviewed core adapters and the injected registry; legacy client/request helper contexts are removed.
- `src-tauri/modules/flint-feature-loaders/src/lib.rs` owns request validation, vanilla baseline preparation, typed context construction, one injected-registry selection through the sole `LoaderKind` binding, profile verification, and final `LoaderInstallResult` assembly. `src-tauri/src/loaders.rs` retains only Tauri progress-sink/event adaptation and delegation. The six implementation leaves live in extracted provider crates and are instantiated only by `provider_composition.rs`.
- The six provider implementations, typed host adoption, injected immutable registry, progress/event mapping, 75/600/600 timeout split, and shared provider runtime lifecycle are implemented. Loader progress messages are sanitized and bounded before event publication.

Typed contexts are implemented in `flint-provider-api`: approved core network/download, safe workspace/path, Java-runner, progress/logging, timeout, deadline, and cancellation ports. The six provider crates and injected-registry host adoption are implemented, and the same provider truth drives lifecycle availability and commands.

Loader providers may run only trusted installer flows, with bounded processes, captured/redacted output, timeouts, and safe archive policy. A mod provider must never execute a downloaded mod archive.

### Enforcing shared ports

Phase 5 checker status:

- The active strict checker enforces frontend composition/import boundaries, the exact five Tauri adapters and importer map, module/provider import and selection boundaries, typed provider contracts/context ports, all-workspace source roots, exact Cargo internal/external dependency rules and cycles, exact provider crate manifests/targets and six loader requirement/constructor checks, one shared actual-registry `ProviderRuntimeState`, the sole `LoaderKind` binding in `src-tauri/modules/flint-feature-loaders/src/lib.rs`, and exact allowlists.
- Extracted provider and feature-module Cargo manifests may depend only on reviewed allowlist entries. Direct Tauri, HTTP, process, and filesystem implementation dependencies are rejected where the layer must use ports.
- The source checker rejects restricted standard-library paths and calls such as `std::fs`, `std::process`, `std::net`, `Command::new`, and direct process spawning in module/provider implementation code where the corresponding ports are mandatory.
- Only core implementations and host adapters may implement the network, path, Java-runner, settings, logging, and progress ports.
- Policy crates expose typed context/port traits, not a handle to a global adapter, so a provider cannot obtain unrestricted file/process/network access through the contract.
- The checker is not a sandbox. Review rejects obfuscation, unsafe FFI, macro tricks, or new manifest paths intended to evade the allowlist; no temporary production exception remains.

The gate is active at `--phase 5`. See **Architecture boundary checker (Phase 5 strict gate active)** above for the full source, Cargo, frontend, and CI rules.

### Enum and DTO compatibility
The typed mod provider integration, six typed loader provider leaves, and host loader adoption preserve public commands, DTOs, enum serialization, and event names. Mod selection uses the sole `ModSource` binding in `flint-feature-mods`; loader selection uses the sole `LoaderKind` binding in `flint-feature-loaders`. Provider-internal IDs/versions remain internal. Strict mod-search loader validation accepts only `fabric`, `forge`, `neoforge`, and `quilt`; unsupported or legacy separator aliases fail before provider I/O.

- `ModSource` and `LoaderKind` remain serialized enum/DTO values while the frontend contract depends on them.
- The host translates deserialized `ModSource` to a complete provider requirement plus credential kind in the feature `source_binding` function, and translates `LoaderKind` only inside the feature `loader_binding` function. Shared feature services use registry lookup, not provider-specific branching.
- Unknown enum values fail during deserialization as they do today.
- Adding or removing an enum variant is a public contract change: update Rust and TypeScript DTOs and the affected authoritative contract together.
- Provider IDs, provider versions, capability versions, and provider payloads are internal. They must not replace or appear inside public DTOs unless a contract change explicitly introduces them.
- Provider-internal response types are normalized and validated into public DTOs before crossing the adapter boundary.
- Provider API/capability versions are separate from the public RPC contract version and from `feature_version`.

### Provider migration order and status

1. **Implemented:** introduced the provider API, capability model, operation contexts, and provider registry in `flint-provider-api`.
2. **Implemented:** extracted the typed ModProvider API and CurseForge/Modrinth implementation crates while preserving command wrappers, DTOs, serde shapes, and events.
3. **Implemented:** moved mod-source dispatch to the provider registry and kept shared dependency/install logic in the mod feature.
4. **Implemented:** repeated the migration for LoaderProvider, including typed list/install contexts, six independent leaves, and the preserved metadata/installer timeout split.
5. **Implemented in the working tree; CI verification pending:** completed provider runtime lifecycle integration, shared actual-registry `ProviderRuntimeState`, strict allowlists, shim removal, and Phase 5 checker enforcement.

## Tauri host and adapter boundary (Implemented)
Status: **Implemented in the working tree; CI verification pending.** Commands are compile-time registered in `src-tauri/src/lib.rs` and gate before delegating to extracted feature services. Mod/loader provider selection is inside the owning feature crates, not command bodies. `feature_host.rs` and `provider_composition.rs` provide the explicit Rust composition roots, and the host owns the Tauri event/progress adapters and the `RunEvent::Exit` shutdown bridge. The rules below are mandatory for every host change.

The host is an adapter layer, not a place for domain logic.

### Command adapters

A command adapter must:

1. Accept only Tauri/framework types and contract DTOs.
2. Deserialize and validate transport-level shape.
3. Convert DTOs into internal requests using an explicit mapper.
4. Call one feature service or provider-facing application service.
5. Convert internal results/errors back into contract DTOs or the agreed error shape.
6. Return without performing filesystem scans, network calls, provider branching, dependency traversal, archive parsing, or business decisions.

Current public errors are display strings. Preserve that shape unless the API contract changes. Internal typed errors may be used below the boundary, but their mapping must be deliberate, stable, and free of secrets.

### Event adapters

- Events are published through an internal `ProgressSink` or event port, not by feature modules importing Tauri.
- The host adapter owns `AppHandle`, event name strings, payload serialization, and delivery failures.
- Existing event names and payloads remain defined by the contract documents. A new event is added to the relevant contract, emitted through an adapter, and consumed with typed frontend handling.
- By default, progress events are best-effort observational notifications. The command result and committed durable state are authoritative. A failed progress delivery is logged as a diagnostic and does not fail an otherwise successful command or roll back committed state. If a future contract explicitly makes an event a required protocol step, delivery failure must return an error or typed partial-success result and must not be reported as success; already committed durable state is not rolled back.
- Listener cleanup, duplicate emissions, and failed listener registration are handled by the typed frontend event adapter/subscription hook. Event payloads must not contain credentials, tokens, paths, control characters, or unredacted commands. Provider-influenced loader/mod progress messages are redacted, control-normalized, and bounded before publication. Delivery failure of an explicitly required protocol event must not be silently suppressed.

### Compile-time command registration

Tauri command registration uses `tauri::generate_handler!`, whose arguments are compile-time paths. A feature module cannot add a runtime command dynamically. Every new or changed command therefore requires a coordinated change:

- a thin adapter in the host;
- an explicit entry in `generate_handler!`;
- a typed client wrapper;
- matching Rust and TypeScript DTOs;
- contract documentation;
- the applicable CI verification.

The composition root may assemble feature services and provider registries, but the command list remains a static, reviewable list. No macro or build script should scan source files to generate commands implicitly.

### Async and blocking boundary

- Tauri async commands must not perform blocking filesystem, network, archive, or process work on the async runtime thread.
- The host/adapter layer is responsible for scheduling blocking module operations with the approved task runner and mapping join failures.
- Modules expose sync/blocking operations only when their contract says so, or async operations that do not hide blocking calls without a scheduling boundary.
- Do not hold locks, references into registry state, or UI-bound resources across an await point.
- Timeouts and bounded operations belong in the core/feature policy, not only in the UI.
- Cross-command cancellation is not promised by the current contract. Provider operations check their scoped cancellation/deadline controls before I/O, and the frontend treats superseded results as stale and cleans up listeners/timers on unmount or context change.

## Frontend architecture and contribution registry (Implemented)

Status: **Implemented in the working tree; CI verification pending.** The typed registry/contracts, frozen validation, explicit static composition root, registry-driven rendering, and host-availability filtering are implemented. The registry supports pages, navigation, settings sections, instance actions, and overlays. The settings feature contributes five mounted settings sections, the launch feature contributes a mounted instance action, and the accounts feature contributes the account-dialog overlay. `AppPageContext` injects typed host services and availability-filtered contribution lists. The exact five-file Tauri adapter boundary and stale-event/request guards are part of the strict Phase 5 gate.

### Current frontend layout

```text
src/
  main.tsx                       # creates HostServices and mounts App
  api.ts                         # typed command client over tauriInvoke
  types.ts
  utils.ts                       # exact parseLoaderKind and UI helpers
  app/
    App.tsx
    AppSidebar.tsx
    composition.ts               # explicit feature composition root
  components/
  core/
    api/tauriInvoke.ts
    events/hostEvents.ts
    events/tauriEvents.ts
    events/useHostEvent.ts
    platform/hostServices.ts
    platform/tauriDialogs.ts
    platform/tauriOpener.ts
    platform/tauriServices.ts
    platform/tauriWindow.ts
    requests/requestGeneration.ts
    requests/useRequestGeneration.ts
    app-context.ts
    availability.ts
    contributions.ts
    ContributionBoundary.tsx
    InstanceActionRenderer.tsx
  modules/
    launch/
    versions/downloads/
    accounts/
    saves/
    settings/
    about/
```

Only the five files `core/api/tauriInvoke.ts`, `core/events/tauriEvents.ts`, `core/platform/tauriDialogs.ts`, `core/platform/tauriOpener.ts`, and `core/platform/tauriWindow.ts` may import `@tauri-apps/*`. `core/platform/tauriServices.ts` composes those adapters, and only `src/main.tsx` imports the concrete service factory. Features consume the Tauri-free contracts in `core/platform/hostServices.ts` through `AppPageContext`.

### Contribution slots

| Slot | Required semantics | Current implementation |
| --- | --- | --- |
| **Navigation** | `id`, owning `featureId`, label, icon, `targetPageId`, numeric order, optional typed visibility/capability selector. `targetPageId` must equal a registered page contribution ID. | Implemented and rendered from the registry. |
| **Page** | Canonical page `id`, owning `featureId`, title, component or local lazy-loader, numeric order, declared shell inputs. | Implemented through `ContributionBoundary`; current modules use static component references. |
| **Settings section** | `id`, owning `featureId`, title, component, order, settings namespace, optional validation hook. | Five settings-section contributions are mounted from `AppPageContext.settingsSections`. |
| **Instance action** | `id`, owning `featureId`, label, icon, order, typed visibility/enabled selectors, handler with `InstanceActionContext`. | Launch-navigation action is registered and rendered by `InstanceActionRenderer`. |
| **Overlay** | `id`, owning `featureId`, order, component, optional visible predicate taking `AppPageContext`. Overlay IDs participate in the same global contribution-ID namespace. | Accounts registers the account-dialog overlay; `App` renders visible availability-filtered overlays through `ContributionBoundary`. No shell-specific account export or conditional remains. |

A contribution declaration is data plus a locally bundled component/function reference only. Registration performs no I/O, network calls, timers, event listeners, or store mutation during module evaluation. A contribution loader is never a runtime string path.

### Registry rules

The runtime registry validates feature descriptors and all five slot shapes, enforces globally unique IDs/page targets/ordering/feature requirement cycles, and freezes the registry before render.

- Every contribution has a globally unique stable ID across all slots and is prefixed by the owning feature ID.
- Duplicate IDs, unknown navigation targets, self/cyclic/duplicate feature requirements, invalid owners, and invalid component/function references are rejected before rendering.
- Ordering is numeric `order` ascending, then stable contribution ID lexicographically.
- The composition root explicitly imports approved feature modules and passes their descriptors to the registry builder. Glob discovery, remote manifests, and runtime module URLs are forbidden.
- The shell owns the selected page ID and rendering boundaries. Unknown or unavailable pages show a diagnostic surface rather than rendering another feature.
- The host availability snapshot is required. The shell filters navigation, pages, settings sections, instance actions, and overlays by host/feature readiness; unavailable contributions are not mounted. Diagnostics may list safe reasons but never fetch remote code or substitute another feature.
- The registry is frozen before React renders and cannot be mutated while running.

### Composition and state

```text
src/main.tsx
  -> createTauriHostServices()
  -> src/app/App.tsx (services prop)
  -> src/app/composition.ts imports approved feature descriptors
  -> buildContributionRegistry validates/freezes frontendRegistry
  -> availability snapshot filters the registry
  -> AppSidebar renders navigation; App renders page + visible overlays
```

`src/app/composition.ts` is the only frontend file that imports the complete feature-module list. `AppPageContext` is the explicit typed boundary for shell state, availability state, frozen contribution lists, host services, navigation, notices, and shared actions.

| State | Owner |
| --- | --- |
| Selected instance and effective game directory | App/shell-level typed context backed by instance/settings contracts. |
| Saved and draft settings | Settings feature/service; other sections use the shared draft interface. |
| Active account and account summary | Accounts feature with shell-provided read-only context. |
| Per-feature query results, progress, modal/overlay state, and form state | Owning feature module. |
| Cross-feature notices | App-level notice service with explicit typed API. |
| Registry and contribution lists | App composition root; immutable after freeze. |
| Backend authoritative data | Owning backend feature through the typed API client and contract DTOs. |

No feature may import another feature module, its store, or its public entry. Cross-feature actions, state, and navigation pass through typed core contracts, injected host services, or contribution slots.

### Imports, async work, and lazy loading

- Tauri framework imports are restricted to the exact five adapter files. Concrete adapter modules may be imported only by their reviewed composition callers; features use `HostServices`.
- The typed API client reaches IPC only through `src/core/api/tauriInvoke.ts`. Event subscription uses `createHostEventSubscriber` behavior through the reviewed adapter and `useHostEvent` cleanup.
- Async components use `useRequestGeneration` (or equivalent reviewed generation guard) and check `isCurrent` before committing success, error, or loading state after an await.
- Contribution components use static references or a local `lazy(() => import("./Page"))` with one literal, relative, bundler-resolvable specifier.
- Variable/template import specifiers, absolute or remote URLs, package-specifier contribution loaders, `eval`, `new Function`, remote UI, and untrusted HTML are forbidden.

## Cross-cutting rules

### Errors and recovery

- Treat all frontend input, provider payloads, local metadata, archives, and process output as untrusted.
- Validate at the boundary and again where a lower layer relies on invariants. Use bounded sizes, counts, depths, and timeouts.
- Return actionable errors with operation context; do not panic on malformed local files, network failures, or provider responses.
- Public command errors retain the contract's current display-ready string shape unless a deliberate contract change is approved. Internal error variants should remain typed enough to map consistently.
- A failed operation must not leave a false success state. Partial filesystem changes must be described in the result or cleaned up according to the operation's contract.
- Command results and committed durable state are authoritative. Default progress-event delivery is best-effort: a delivery failure is logged and does not fail an otherwise successful command or roll back committed state. If a contract explicitly marks an event as a required protocol step, failure to deliver it must return an error or typed partial-success result and must not report success; committed durable state is still not rolled back.
- Never include passwords, access tokens, refresh tokens, API keys, authorization headers, provider URLs, or unredacted launch commands in logs, events, errors, or UI notices. `LaunchResult.command` is constructed from redacted display arguments; the executable process still receives the internal arguments.

### Events

- Event names and payloads are public contract. Existing `flint://...` names and DTO payloads must not be renamed as an internal refactor. Command results and committed durable state are authoritative; default progress events are best-effort and never roll back committed state. Explicitly required protocol events must fail the operation result when delivery fails, while still preserving already committed state.
- Event publication goes through a host adapter/port. Core, feature, and provider code must not import Tauri solely to emit events.
- Emit progress from the owning operation, not from unrelated retries or stale requests. Provider-influenced loader/mod progress messages are passed through `sanitize_progress_message`, which redacts credential/URL patterns, normalizes control/whitespace characters, and bounds the result to 512 Unicode scalar values before publication. Consumers ignore stale results and unlisten on unmount.
- A future event addition must update the relevant authoritative contract, Rust payload, TypeScript payload, API/event client, and tests in the same change. Both RPC contract files are updated together only when the event affects a shared model or command surface.

### Paths and settings

- Path resolution, default game directory selection, safe component checks, symlink boundaries, and app-data locations belong in shared core/path policy.
- A feature may receive approved paths but may not construct its own app-data directory or duplicate path validation in a provider.
- `src-tauri/crates/flint-core/src/paths.rs` owns shared path policy, including safe joins and metadata path validation; `src-tauri/modules/flint-feature-settings` owns settings validation/defaults and atomic persistence through that policy.
- Settings remain one versioned persisted document with backward-compatible defaults. Multiple UI contributions may edit a view model, but one settings owner validates and atomically persists it.
- Settings changes that introduce fields or sections must preserve older-file loading, update both Rust and TypeScript models, and update the affected authoritative contract. Update both RPC contract files only when the shared settings/model surface is documented in both.
- Any caller-supplied path is untrusted, including export destinations. Check that destructive operations remain within their declared roots and never follow symlinks outside those roots.

### Async and blocking work

- Keep blocking I/O off the Tauri async command runtime.
- Use a shared task/runner policy instead of ad hoc threads in features or providers.
- Bound retries and include timeouts. Do not retry a destructive operation without an explicit idempotency rule.
- Do not hold a lock across blocking I/O or await. Registries are immutable after startup; runtime mutable state should be feature-owned and narrow.
- UI request-generation guards must prevent stale completions from overwriting newer state or clearing a newer loading state. Listener registration uses a cleanup-safe adapter so a listener that resolves after unmount is immediately removed.

### Security boundaries and limits

- The Tauri IPC/frontend boundary is trusted only for shape, never for safety. Commands revalidate all paths, identifiers, sizes, and operation context.
- External requests require HTTPS, bounded response bodies, timeouts, and redirect/protocol review. Providers may not bypass the shared network policy.
- Archive handling must enforce the entry-count limit before per-entry path parsing, then reject traversal, absolute paths, duplicate/case-alias/unsafe names, symlink entries, oversized paths, and decompression/size bombs. Save and native-JAR extractors preflight all entries through the shared `SafeWorkspace::validate_archive_entries` policy before creating or writing extraction output; actual-copy limits remain in place for archives whose declared sizes do not match their data.
- Downloaded mod archives are inspected but never executed by the launcher. Loader installer execution is limited to the trusted flows defined by the feature contract and a bounded Java process runner.
- Provider and feature IDs are not filesystem paths. Never derive a write location from a display name or unvalidated provider string.
- Do not broaden Tauri capabilities, window permissions, or the current content security configuration as an incidental part of feature work. Any security-surface change needs explicit review.
- Runtime native plugins are deferred because they introduce ABI stability, signing, isolation, crash containment, resource limits, and arbitrary-code trust problems that this design does not solve.

### Logging and observability

- Log operation, feature/provider ID, and a safe path or object identifier where useful.
- Redact secrets and sensitive headers. Prefer concise English diagnostics over dumping raw external responses. Installer/process output, provider progress text, availability reasons, and launch-command display are redacted and bounded through reviewed helpers.
- Do not log full filesystem contents, user tokens, or entire provider payloads.
- Errors surfaced to the user should be concise and actionable; detailed technical context may go to a safe log.
- Health and registry snapshots should expose stable IDs, versions, dependency state, and failure reasons without leaking credentials.

## Migration phases (Implementation status)

The migration sequence is complete through Phase 5 in the working tree. Each phase preserves the external RPC contract and is backed by the strict checker, tests, and the release workflow gate. GitHub Actions is the final authority; this repository does not yet record a green Phase 5 run.

### Phase 1: ModProvider (Implemented; CI verification pending)

Status: **Implemented.** The provider-neutral mod contract/registry lives in `flint-provider-api`; CurseForge and Modrinth implementations live in extracted provider crates. Mod operations use one immutable registry held by `ProviderRuntimeState`, while the mods feature owns source-to-requirement binding, dependency planning, downloads, archive inspection, sidecar state, installed-mod operations, DTO mapping, and progress publication.

Acceptance criteria implemented:

- No command body or shared mod service branches on individual mod sources after lookup.
- Provider descriptors, provider/capability API compatibility, semantic provider versions, capability-scoped selection keys, duplicate registrations, and zero/multiple selection results are validated explicitly.
- Providers initialize in lexicographic provider-ID order and have no provider-to-provider dependency graph. Selection uses the full predicate and requires exactly one active match with no fallback.
- Typed network, path/archive, process, progress/logging, scoped credential/settings, deadline, and cancellation ports are the provider boundary; direct Tauri/host handles and unrestricted filesystem/process/network access are forbidden by the strict checker.
- Unknown, incompatible, ambiguous, or unavailable providers return actionable errors and never fall back silently.
- Existing mod commands, inputs, results, event names, and DTO serialization remain compatible. The only intentional validation change is that unsupported mod-search loader values now fail explicitly for `fabric`, `forge`, `neoforge`, and `quilt` contexts.
- The strict Phase 5 checker backs the extracted sources, manifests, composition, typed ports, sole source binding, and shim bans.

### Phase 2: LoaderProvider (Implemented; CI verification pending)

Status: **Implemented.** Six independent loader providers implement typed `LoaderProvider` contexts. `ProviderRuntimeState` owns the immutable loader registry; `modules/flint-feature-loaders/src/lib.rs` owns the sole `LoaderKind` binding and typed orchestrator; `src-tauri/src/loaders.rs` only adapts Tauri progress events.

Acceptance criteria implemented:

- Loader list/install commands contain no provider-specific branches after lookup.
- Providers use typed network, safe workspace/archive, Java-runner, progress/logging, timeout, deadline, cancellation, and vanilla-preparation boundaries. Direct Tauri/HTTP/filesystem/process implementation dependencies are forbidden in provider leaves.
- Duplicate provider/capability IDs and malformed capability/key bindings fail explicitly; provider selection requires exactly one full-predicate match and never uses map order or first-match fallback.
- The 75-second metadata / 600-second installer HTTP / 600-second Java timeout split is preserved.
- Loader installation preserves vanilla preparation, profile verification, result/event shapes, and redacted logs.
- Provider registry construction failure and runtime loss flow through the shared `ProviderRuntimeState`; affected Required features follow fatal startup/runtime semantics and optional-only failures may degrade only while all Required features stay Ready.
- Runtime lifecycle, reverse/idempotent shutdown, cancellation-before-I/O, and gating tests are implemented subject to hosted CI verification.

### Phase 3: Frontend contribution registry (Implemented)

Status: **Implemented.** The typed registry validates and freezes page, navigation, settings-section, instance-action, and overlay contributions. The settings feature contributes five sections, launch contributes an instance action, and accounts contributes the account-dialog overlay.

Acceptance criteria implemented:

- Duplicate contribution IDs are rejected globally; navigation targets resolve to exactly one page ID; invalid ordering, owners, components, and feature cycles fail before render.
- Shell rendering is registry-driven. Adding a page does not require a hard-coded navigation array or page conditional chain.
- Contribution order is deterministic by `order` then stable ID, and the registry is frozen before render.
- A feature imports only its own subtree plus frontend core/shared contracts; no cross-feature module import is allowed.
- Remote UI, arbitrary dynamic imports, `eval`, and untrusted HTML are forbidden. Literal local lazy imports are the only dynamic contribution-loading form.
- The availability client filters navigation, pages, settings sections, instance actions, and overlays by host/feature readiness and fails closed.

### Phase 4: Workspace and module extraction (Implemented)

Status: **Implemented.** `src-tauri/Cargo.toml` is the workspace root with exactly nineteen packages: the host, `flint-core`, `flint-feature-api`, `flint-provider-api`, seven feature crates, two mod provider crates, and six loader provider crates. One `src-tauri/Cargo.lock` is the workspace lockfile.

Acceptance criteria implemented:

- Cargo metadata and source checks enforce layer direction, exact workspace membership, one lockfile, coherent versions/resolver, exact internal/external dependency rules, and no dependency cycles.
- The host package remains `flint` with the `flint_lib` library target; release/version tooling preserves the same executable paths and updates all local package versions together.
- `src-tauri/src/lib.rs` contains exactly 30 compile-time command handlers and no domain/provider logic. `launcher_info` and `get_host_availability` are ungated; the other 28 commands gate through the reviewed owning-feature map before I/O.
- The feature registry/coordinator validates before side effects, detects dependency cycles, initializes deterministically, propagates failure through transitive dependents, enforces the Required-Ready invariant, exposes read-only availability, gates unavailable features, and shuts down in reverse successful-init order.
- Frontend snapshot/filtering and unavailable-feature diagnostics are implemented without fallback.
- The Phase 4 Cargo/source/typed-port checks are retained inside the stricter Phase 5 gate.

### Phase 5: Hardening and documentation closeout (Implemented; CI verification pending)

Status: **Implemented in the working tree; GitHub Actions verification pending.** No temporary checker exception or old/new dispatch path remains in reviewed production source.

Scope completed:

- Replaced every direct Tauri frontend import with the exact five typed adapters and injected `HostServices` through `AppPageContext`; removed phase-expiring Tauri exceptions.
- Added the typed overlay contribution slot and replaced the special account-dialog shell export with the accounts overlay.
- Removed legacy frontend loader parsing/substring matching, Rust loader/provider aliases, dead `LoaderBinding` metadata, phase-expiring provider-target machinery, and the metadata-only duplicate provider catalog.
- Consolidated provider lifecycle truth in one shared actual-registry `ProviderRuntimeState` implementing `ProviderCatalog` and used by both commands and feature lifecycle.
- Added early archive entry-count rejection and full preflight validation for save-import and native-JAR extraction; retained actual-copy size/symlink/path defenses and cleanup tests.
- Added bounded/sanitized provider progress text, redacted `LaunchResult.command` construction, availability-reason redaction tests, and cancellation-before-I/O tests.
- Added shared frontend request-generation and event-cleanup hooks with stale-response, unmount, late-registration, and timer tests.
- Activated the strict Phase 5 checker for exact adapters/importers, shim bans, overlays, provider runtime integration, all-workspace Cargo/source rules, and the command gate map.

Acceptance criteria:

- No target contract exists only in a migration comment.
- No duplicate old/new dispatch path or temporary architecture/checker exception remains.
- Minimal public surfaces and cross-feature isolation are enforced by source checks and review.
- Public behavior changes are reflected in the authoritative contracts: availability gating, bounded/sanitized progress messages, redacted launch command display, and strict mod-loader validation. Command/event names and DTO shapes are unchanged.
- Architecture, ADR, standards, index, release documentation, and tests are synchronized.
- GitHub Actions is the recorded verification authority. Local reviewer commands are not evidence, and no green CI result is claimed until the hosted workflow records one.

### Deferred: optional runtime plugins

Runtime plugins are not part of this architecture migration and must not be added as an incidental extension of the registry work.

A future proposal must first define:

- a stable binary or sandboxed plugin ABI and compatibility policy;
- trusted installation, signature/provenance, update, and removal rules;
- capability grants and least-privilege access to paths, network, processes, settings, and UI slots;
- crash containment, health supervision, resource limits, timeouts, and version negotiation;
- isolation strategy for native code or a deliberate restricted interpreter/VM choice;
- a prohibition on arbitrary remote UI and an explicit rule for how any plugin-provided UI is reviewed and bundled;
- migration and uninstall behavior for persisted plugin data.

Until a separate accepted Architecture Decision Record exists, all registrations are compile-time and the application must remain a modular monolith.

## Architecture acceptance gates

These are the end-state acceptance gates for the completed Phase 5 implementation. GitHub Actions is the final authority; this document records implementation and expected checks, not a claimed green run.

- The current implementation is described accurately, and unimplemented future extensions are labeled planned/deferred rather than presented as current behavior.
- Cargo manifests and TypeScript imports satisfy the Phase 5 gate in the release workflow `prepare` job. The gate runs architecture/checker tests, frontend contract tests, and `node scripts/check-architecture.mjs --phase 5`.
- `flint-core`, `flint-feature-api`, and `flint-provider-api` obey inward-only dependencies. Feature/provider crates cannot depend on the host, Tauri, another feature’s private modules, or another provider.
- The host package remains `flint` with the `flint_lib` target. The workspace has exactly nineteen reviewed packages, one root lockfile, coherent shared versions, exact internal/external dependency allowlists, and no dependency cycles.
- Provider leaves use only reviewed typed context/port boundaries. Raw Tauri, HTTP client, filesystem, process, network, settings-file, and provider-to-provider access is rejected outside approved core/host adapter implementations.
- Exactly one shared actual-registry `ProviderRuntimeState` implements the feature provider catalog, services provider-backed commands, records initialization/loss/shutdown state, and is created once in host startup.
- Feature descriptors use stable IDs, `feature_version`, `api_version`, declared dependencies/provider requirements, startup policy, and separately registered initialization entry points.
- Startup/runtime failure propagation includes the failed feature and all transitive dependents. Any affected `Required` feature is fatal: `StartupFailed` with feature/provider/core cleanup at startup, or `RuntimeFailed` -> `Stopping` -> `Stopped`/exit at runtime. `Degraded` is valid only while every `Required` feature remains `Ready`; no Required feature remains Ready with a non-ready dependency/provider.
- Feature order is deterministic topological order with feature-ID tie-breaking; provider leaves initialize in provider-ID order. Reverse/idempotent shutdown, unavailable-command gating, and shutdown diagnostics are explicit and tested.
- Provider selection uses active providers, per-capability selection-key bindings, and the full capability/API/provider-version requirement. Exactly one match is required; zero/multiple matches fail without fallback.
- Tauri commands are thin and compile-time registered: 30 handlers, with `launcher_info`/`get_host_availability` ungated and 28 feature commands gated before I/O.
- Frontend Tauri imports occur only in the exact five reviewed adapters with their exact package specifiers. Only `src/main.tsx` creates the concrete host services; features consume `HostServices` through `AppPageContext`.
- Frontend contribution IDs are globally unique across pages, navigation, settings sections, instance actions, and overlays. Navigation uses canonical `targetPageId` page IDs. Feature modules import only their own subtree plus core/shared contracts.
- Progress event names and payload shapes remain stable. Provider-influenced loader/mod progress messages are redacted, control-normalized, and bounded; launch-command display is redacted; availability reasons exclude credentials, provider IDs, URLs, and unsafe paths.
- Archive handling enforces entry-count limits before per-entry path parsing, preflights traversal/symlink/duplicate/case-alias/path-size/size limits through shared policy, and retains actual-copy/cleanup defenses.
- Public behavior changes update the affected authoritative contract; both RPC contract files change only for a shared model/command/event surface. No command, event, or DTO was renamed by the internal migration.
- No copied names, branding, identifiers, text, or code from another project is present. References only inform expected behavior.
