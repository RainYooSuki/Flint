# Flint Architecture

## Document status

This document is the canonical detailed source for architecture boundaries, provider/registry semantics, lifecycle, frontend contributions, and migration phases. It defines the implemented baseline, the accepted target design, and the migration path between them.

| Label | Meaning |
| --- | --- |
| **Implemented** | Present in the repository at the recorded baseline. |
| **Partially implemented** | A named milestone is present, but the full target or full phase is not complete. |
| **Planned** | Approved target design that has not been implemented yet. |
| **Deferred** | Explicitly outside the current design and requires a separate decision. |

Current baseline: the repository tree contains all four frontend contribution slots, the active Phase 4 architecture gate, the Phase 4 workspace foundation with `flint-core`, `flint-feature-api`, `flint-provider-api`, seven extracted feature crates, two extracted mod provider crates, and six extracted loader provider crates, shared typed ports, typed mod provider adoption, typed loader host adoption at milestone level, and the host feature lifecycle/availability IPC milestone. All seven feature crates are implemented; the Phase 4 checker gate is implemented, while Phase 5 remains pending. This document owns the canonical phase-status table and distinguishes implemented paths from accepted target-only requirements.

### Canonical migration phase status

| Area | Status |
| --- | --- |
| **Rust structure** | `src-tauri/Cargo.toml` is both the host package manifest and the workspace root with resolver 2 and exactly nineteen current members: `.`, `crates/flint-core`, `crates/flint-feature-api`, `crates/flint-provider-api`, `modules/flint-feature-accounts`, `modules/flint-feature-launch`, `modules/flint-feature-loaders`, `modules/flint-feature-mods`, `modules/flint-feature-vanilla`, `modules/flint-feature-settings`, `modules/flint-feature-saves`, `providers/flint-mod-provider-curseforge`, `providers/flint-mod-provider-modrinth`, and the six `providers/flint-loader-provider-*` crates. The host package remains `flint` with the `flint_lib` library target. All nineteen packages share the generated date-based release version through the host-matching `[workspace.package] version`; non-root members use `version.workspace = true`. `flint-core` owns the public path/network/error policy plus framework-neutral typed ports for bounded network access, safe workspace/archive handling, process/Java execution, progress, logging, deadlines, and cancellation. It also owns generic Minecraft profile/library/installer helpers. `flint-provider-api` owns the generic provider registry plus typed mod and loader contracts. The seven feature crates implement accounts, vanilla, settings, saves, launch, mods, and loaders plus their DTOs, the two mod provider crates implement the mod contract, and six loader provider crates implement the loader contract through core ports. The host re-exports policy modules as `crate::{paths, net, error}`. The Phase 4 checker gate is implemented; Phase 5 remains pending.
| **Phase 1: ModProvider** | **Partial: typed mod provider milestone implemented; full phase pending.** The generic provider registry and typed mod contracts live in `flint-provider-api`; CurseForge/Modrinth implementations live in their extracted crates. The host composes and injects one immutable mod registry. Full provider runtime lifecycle acceptance and Phase 5 remain open; the Phase 4 checker gate is implemented.
| **Phase 2: LoaderProvider** | **Milestone implemented: six loader leaves and typed host adoption; full runtime phase pending.** The six `providers/flint-loader-provider-*` crates implement the typed `LoaderProvider` contract through core ports. `provider_composition.rs` builds one immutable loader registry from the six concrete constructors, and `modules/flint-feature-loaders/src/lib.rs` holds the sole `LoaderKind` binding and typed contexts, and the preserved 75-second metadata / 600-second installer HTTP / 600-second Java split. Provider runtime lifecycle acceptance and Phase 5 remain pending; the Phase 4 checker gate is implemented.
| **Phase 3: Frontend contribution registry** | **Implemented.** Typed contracts, validation, the frozen registry, and the explicit static composition root are in `src/core/contributions.ts`, `src/app/composition.ts`, and `src/app/`. Pages and navigation are registry-driven; the settings feature registers five mounted settings sections, and the launch feature registers a mounted launch-navigation instance action. Settings sections render through a per-contribution boundary from `SettingsView`, and instance actions render through the shared `src/core/InstanceActionRenderer.tsx`. `AppPageContext` exposes the frozen `settingsSections` and `instanceActions` arrays. The Phase 4 host availability snapshot/filtering milestone now consumes this static registry. |
| **Architecture checker and CI** | **Implemented Phase 4 checker gate; Phase 5 pending.** `scripts/check-architecture.mjs` and its tests run in the `flint-release.yml` `prepare` job: tests run, then `node scripts/check-architecture.mjs --phase 4`. The active static checker enforces frontend boundaries, exact nineteen-member workspace membership, host feature/loader path dependencies, exact feature and loader crate manifests/dependency allowlists, all extracted feature/loader production sources, both immutable composition registries, the sole loader-feature `LoaderKind` binding and thin host loader event adapter, denied provider std/reqwest/fs/process/Tauri patterns, forbidden feature Tauri/host-private imports, the vanilla InstallProgressSink/event boundary, exact descriptor/key requirements, and the 30-command/28-feature-command gate map. At `--phase 4` it also invokes `cargo metadata` for package/dependency truth, requires the exact 19-package/edge/external-dependency allowlists and cycles, scans every production source under all 19 crates, and enforces typed provider ports. Phase 5 checks remain inactive.
| **Phase 4: Workspace/module extraction and Rust feature lifecycle** | **Phase 4 checker gate implemented; provider runtime lifecycle hardening and Phase 5 remain pending.** The workspace contains nineteen crates sharing one version source: the host, three foundation crates, seven feature crates, two mod providers, and six loader providers. `provider_composition.rs` injects one mod registry and one loader registry; `feature_host.rs` is the explicit feature composition root. Host startup, managed runtime state, availability IPC/filtering, and gates on all 28 feature commands are implemented. All seven feature crates are extracted; the Phase 4 Cargo-metadata/source/typed-port gate is active. Full provider runtime lifecycle hardening and Phase 5 remain pending.
| **Phase 5: Hardening and documentation closeout** | **Pending.** Temporary transitional waivers and remaining migration work continue. |
| **Public RPC contract** | Existing command names, arguments, success results, events, and DTOs are preserved. Phase 4 adds the ungated `get_host_availability` command and gates all 28 existing feature commands; the availability DTO and unavailable-error semantics are documented in `API_CONTRACT.md`. |

Important current boundary: the application is still a **modular monolith**. The host workspace root, three foundation crates, seven extracted feature crates, two mod provider crates, and six loader provider crates exist. The feature lifecycle contract/registry/coordinator is composed and started through `feature_host.rs`, with managed availability state, IPC publication, frontend filtering, and command gates. The loader feature crate now owns the typed orchestrator over an injected immutable registry; the host loader module is a thin progress-event adapter and source-specific loader support lives in the six provider crates. All seven feature crates are extracted; the Phase 4 checker gate is implemented. Full provider runtime lifecycle hardening and Phase 5 remain planned.

Target-only requirements are phase-gated. [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) defines the transition matrix and the compliant path for work performed before the relevant target structure exists.

The frontend/backend RPC contracts remain authoritative:

- [`API_CONTRACT.md`](API_CONTRACT.md) defines the base commands, DTOs, events, and behavior.
- [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md) defines the extended account, loader, mod, and save contracts and their events.

Architecture, module boundaries, and internal registries must not rewrite those contracts by implication. If this document and a contract disagree about an RPC name, argument, result, event, or user-visible rule, the contract is the source of truth until all documents and code are intentionally updated together. A public behavior change updates the affected authoritative contract; both RPC contract files are updated only when the shared model, command, or event surface is affected.

Related documents:

- [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) contains mandatory implementation and review rules.
- [`architecture-decision-records/0001-modular-core-and-feature-modules.md`](architecture-decision-records/0001-modular-core-and-feature-modules.md) records the accepted decision.
- [`README.md`](README.md) is the documentation index.

## Purpose

The launcher spans several independent capabilities: local version discovery, installation, launch, accounts, loader installation, mod discovery and management, saves, and settings. The host anchors a workspace with extracted `flint-core`, `flint-feature-api`, `flint-provider-api`, seven extracted feature crates under `src-tauri/modules`, two mod provider crates, and six loader provider crates under `src-tauri/providers`. `flint-core` owns shared policy and typed port contracts. The frontend has a static contribution registry with snapshot-driven availability filtering. Typed mod and loader provider contracts/leaves, generic provider registry, feature lifecycle, host lifecycle/availability integration, and frontend registry are implemented at milestone level; all seven feature crates are extracted; the Phase 4 checker gate is implemented, while provider runtime lifecycle hardening and Phase 5 remain target work.

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

## Current baseline (Implemented)

### Backend shape

The Rust backend is a Tauri v2 workspace rooted at `src-tauri`, with the host package plus eighteen extracted crates: three foundation crates, seven extracted feature crates, two mod provider crates, and six loader provider crates. `flint-core` is the shared policy and typed-port layer, `flint-feature-api` owns feature lifecycle contracts, `flint-provider-api` owns generic and typed mod/loader contracts, and the provider crates implement CurseForge, Modrinth, Fabric, Quilt, Forge, NeoForge, LiteLoader, and OptiFine through core ports. The host keeps thin command orchestration and Tauri progress-event adapters; the loader feature crate owns the sole `LoaderKind` binding. `provider_composition.rs` and `feature_host.rs` remain host roots.

- `src-tauri/Cargo.toml` is both the host package manifest and the workspace root and is the single Cargo version source: the host `[package] version` equals `[workspace.package] version`, and each non-root member manifest declares `version.workspace = true`. `src-tauri/Cargo.lock` is the single workspace lockfile and carries one version entry for every local workspace package.
- `src-tauri/src/main.rs` is a thin desktop entry point that calls `flint_lib::run()`.
- `src-tauri/src/lib.rs` declares every remaining host Rust module, re-exports `flint_core` policy modules as `crate::{paths, net, error}`, contains the 30 `#[tauri::command]` wrappers (`launcher_info` and `get_host_availability` ungated, 28 feature commands gated), and registers each exactly once with the compile-time `tauri::generate_handler!` macro.
- `src-tauri/src/models.rs` contains the serializable Rust DTOs. Existing public command/DTO/event contracts are preserved; Phase 4 adds the host availability DTO/command and feature-command gating error behavior documented in `API_CONTRACT.md`.
- Blocking network, filesystem, process, and archive work is generally moved off Tauri async commands with `tokio::task::spawn_blocking`.
- Public command failures are currently returned as display-ready `String` errors. `flint-core::error` maps background-task join failures; the host re-exports it as `crate::error`.

The current Rust module responsibilities are:

| Module | Implemented responsibility |
| --- | --- |
| `crates/flint-core/src/paths.rs` | Application and game directory resolution, settings path, path normalization, safe joining, deduplication keys, and SHA-1 helpers. Public core policy used by the host through `crate::paths`. |
| `crates/flint-core/src/net.rs` | Shared blocking HTTP client creation, HTTPS validation, bounded response reads, atomic downloads, temporary paths, and directory creation. Public core policy used by the host through `crate::net`. |
| `crates/flint-core/src/error.rs` | Conversion of background-task failures to public error strings, re-exported by the host as `crate::error`. |
| `crates/flint-core/src/ports/*` | Object-safe, framework-neutral bounded network, safe workspace/archive, process/Java, progress, logging, deadline, and cancellation contracts, plus the `ReqwestNetwork`, `SafeWorkspace`, `CommandProcessPort`, and `ProcessJavaRunner` adapters. |
| `crates/flint-core/src/minecraft.rs` | Capability-neutral Minecraft workspace, profile-library resolution/download, installer, ZIP check, profile discovery, version comparison, HTTPS normalization, and progress/log/deadline helpers. |
| `crates/flint-feature-api/src/lib.rs` | Framework-neutral feature IDs, data-only descriptors, registration/initializer and runtime contracts, validated deterministic registry, provider-catalog abstraction, lifecycle coordinator/state machine, command gate, and read-only availability snapshot. Host lifecycle composition and IPC/gate integration are implemented in `src-tauri/src/feature_host.rs` and `src-tauri/src/lib.rs`. |
| `src-tauri/src/feature_host.rs` | Host feature composition root: seven explicit feature descriptors/initializers, a catalog built from real provider metadata, deterministic coordinator startup/snapshot state, poisoned-lock-safe managed runtime state, command gate helper, and idempotent shutdown. |
| `crates/flint-provider-api/src/lib.rs` | Generic versioned provider descriptors, capability bindings, provider requirements, compatibility checks, immutable deterministic provider registry, and unavailable/ambiguous selection errors. Shared `ApiVersion` and `ProviderRequirement` values come from `flint_core::contract` and are re-exported by `flint_provider_api`. The crate also exposes typed mod and loader provider contracts; its non-default `test-ports` feature supplies in-memory fake network, workspace, and Java ports for tests. |
| `provider_composition.rs` | Explicit host composition of one immutable `ProviderRegistry<dyn ModProvider>` and one immutable `ProviderRegistry<dyn LoaderProvider>`, plus managed `ModProviderState`/`LoaderProviderState`, leaf descriptor helpers, and exact requirement helpers consumed by feature availability. Concrete provider construction stays only here.
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

The Phase 1 and Phase 2 registry/selection milestones, typed mod provider leaves, six typed loader provider leaves, host loader registry injection/adoption, and active Phase 4 checker enforcement are implemented at milestone level. All seven feature crates are extracted; the Phase 4 checker gate is implemented; Phase 5 remains pending:

- `src-tauri/crates/flint-provider-api/src/lib.rs` provides the shared `Provider` contract, `ProviderCapability`, `ProviderDescriptor`, and `ProviderRegistry`, and re-exports `ApiVersion` and `ProviderRequirement` from `flint_core::contract`.
- The registry is immutable after construction, validates provider/capability/version/API/selection-key constraints, rejects duplicate provider/capability IDs and duplicate capability/selection-key bindings, sorts providers by stable ID, and returns explicit unavailable or ambiguous errors from selection.
- `src-tauri/src/provider_composition.rs` builds and owns one immutable mod-provider registry and one immutable loader-provider registry. Both managed states are injected into provider-backed commands; source-specific mod and loader payload mapping live in the extracted provider crates. The loader composition registers exactly the six concrete constructors and exposes exact leaf descriptors and six keyed requirements.
- `src-tauri/modules/flint-feature-loaders/src/lib.rs` owns the sole `LoaderKind` binding and typed orchestrator. It injects the composed registry, builds typed list/install contexts with separate 75-second metadata and 600-second installer HTTP clients plus the 600-second Java timeout, maps neutral data into unchanged DTOs, and preserves command/event/progress behavior; the host adapter keeps only Tauri sink/event adaptation. The old `loaders/providers.rs` adapter and legacy loader API alternatives have been removed.
- Provider implementations do not depend on one another. Mod and loader providers are validated once in their immutable composition registries and reused through managed state; the feature availability catalog consumes the same real descriptors and requirements. Feature runtime lifecycle hardening and Phase 5 remain pending; the Phase 4 checker enforcement is active.
- All six loader provider crates use the shared core ports and no transitional parent-helper imports. The host loader checker enforces the exact provider members, manifests, source leaves, dependency allowlists, registry construction location, the sole loader-feature `LoaderKind` binding, and the thin host loader event adapter. The Phase 4 Cargo-metadata/source/typed-port gate is active; Phase 5 enforcement remains deferred.

The workspace foundation, `flint-core`/`flint-feature-api`/`flint-provider-api`, the seven extracted settings/saves/accounts/vanilla/launch/mods/loaders feature crates, the two extracted mod provider crates, and the six extracted loader provider crates are implemented. `flint-feature-api` contains the feature descriptor, validated registry, lifecycle coordinator, and availability snapshot contract. The Tauri host composes and starts it through `feature_host.rs`, exposes `get_host_availability`, filters frontend contributions, and gates all 28 existing feature commands before I/O. The shared core policy and typed ports are implemented, including generic Minecraft profile/installer support. Mod and loader providers use the shared ports behind immutable injected registries; all seven feature crates are extracted; full provider runtime lifecycle hardening and Phase 5 remain pending; the Phase 4 Cargo-metadata/source/typed-port gate is active. Existing command names/arguments/success results/events are preserved apart from the additive availability command and gating error behavior.

### Frontend shape

The frontend is a React 18 and TypeScript application rooted at `src`. Phase 3 contribution modularization is implemented without changing the public RPC contract:

- `src/main.tsx` mounts `src/app/App.tsx`.
- `src/core/contributions.ts` defines typed `FeatureDescriptor`, page, navigation, settings-section, and instance-action contracts; validates stable IDs, feature requirements/cycles, page targets, duplicate contribution IDs across slots, ordering, and component/handler references; then builds and deeply freezes a deterministic contribution registry.
- `src/app/composition.ts` is the explicit static frontend composition root. It imports the approved feature modules and exports the frozen `frontendRegistry`; there is no glob auto-discovery or remote contribution manifest.
- `src/app/AppSidebar.tsx` renders navigation contributions and resolves the active page by canonical `targetPageId`/page contribution ID.
- `src/core/ContributionBoundary.tsx` provides the typed per-contribution error boundary used by the shell and the settings-section slot. `src/app/App.tsx` owns shared app-level state, populates `AppPageContext` from `src/core/app-context.ts`, and renders the active page contribution through that boundary.
- `src/modules/` contains the current feature modules: launch, versions/downloads, saves, settings, about, and accounts. The page modules expose pages/navigation; settings also contributes five settings sections, launch contributes an instance action, and accounts exposes the account-dialog surface required by the shell.
- The settings feature owns five section components under `src/modules/settings/sections/` and registers them in descriptor order. `SettingsView` is a shell that renders the registered sections, contribution validators, the outer fieldset, status/error panel, and action bar. Section-local touched state preserves field-level validation displays.
- The launch feature contributes a launch-navigation instance action. `src/core/InstanceActionRenderer.tsx` builds the typed `InstanceActionContext` for the versions page header and renders visible/enabled registered actions; the launch handler uses `context.navigateToPage`.
- `src/api.ts`, `src/types.ts`, `src/utils.ts`, and the remaining generic components still live at `src` and have not completed the target `core`/`shared` extraction.
- Some current modules and the shell still use reviewed transitional direct Tauri event/dialog/opener imports. Those are checker waivers expiring in Phase 5, so full typed adapter extraction is not complete.
- The frontend has no router. The Phase 4 host-availability client validates the read-only snapshot and filters navigation, pages, settings sections, and instance actions by host/feature readiness; malformed snapshots fail closed.
- The implemented Phase 4 checker logic combines static Rust/TypeScript analysis with Cargo metadata. The `flint-release.yml` release workflow `prepare` job installs frontend dependencies, runs the architecture checker tests, then runs the checker at `--phase 4`; all build-matrix jobs depend on `prepare`. The Phase 4 workspace, Cargo-metadata, all-workspace source, and typed-port checks are active in that gate; Phase 5 checks remain inactive.

Phase 3 therefore has the four contribution slots populated and mounted, the shared per-contribution boundary and instance-action renderer in core, and the architecture checker gate active at `--phase 4`. Host active-feature availability filtering, typed mod provider adoption, typed loader host adoption, and exact mod/loader provider crate boundaries are implemented at milestone level; all seven feature crates are extracted; the Phase 4 checker gate is implemented, while provider runtime lifecycle hardening and Phase 5 remain pending.

### Build and release constraints (Implemented)

The current build and release automation assumes these paths and identities:

- The Cargo package root and Tauri application root are `src-tauri`.
- The Cargo lockfile is `src-tauri/Cargo.lock`.
- The Tauri configuration is `src-tauri/tauri.conf.json`.
- The frontend build output is repository-root `dist`, referenced by `frontendDist: "../dist"` in the Tauri configuration.
- The default Cargo target directory is `src-tauri/target`; the `flint-release.yml` release workflow reads executables from `src-tauri/target/<rust-target>/release/flint` or `flint.exe`.
- Release/version tooling inspects the host and `[workspace.package]` versions in `src-tauri/Cargo.toml`, the version in `src-tauri/tauri.conf.json`, every local workspace package entry in `src-tauri/Cargo.lock`, and the root package metadata. It requires member manifests to use `version.workspace = true` and exactly one lock block each for `flint`, `flint-core`, `flint-feature-api`, `flint-provider-api`, `flint-mod-provider-curseforge`, and `flint-mod-provider-modrinth`, all equal to the generated metadata. `apply-version` updates both Cargo manifest version fields and every local lock entry while the release file list remains the documented five paths (`VERSION`, `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`).
- `build.rs` performs Tauri code generation and adds the Windows GNU cdylib link argument.
- The library crate is named `flint_lib` and keeps staticlib, cdylib, and rlib crate types.
Current architecture-checker and CI facts:

- `scripts/check-architecture.mjs` and `scripts/test-architecture-checker.mjs` are implemented.
- The `prepare` job in the `flint-release.yml` release workflow installs frontend dependencies, runs `node --test scripts/test-architecture-checker.mjs`, and then runs `node scripts/check-architecture.mjs --phase 4`.
- All build-matrix jobs depend on `prepare`, so the checker tests and phase-4 gate must pass before any build starts.
- The active checker invokes Cargo only for `cargo metadata` workspace/package/dependency truth and fails closed when it is unavailable or invalid. At `--phase 4` its Rust scope covers all nineteen production source roots, exact package/internal/external dependency rules and cycles, one root lockfile and version/resolver coherence, module/provider import and selection boundaries, typed `LoaderProvider`/`ModProvider` context ports, forbidden raw provider APIs and feature/host imports, both immutable provider registries only in `provider_composition.rs`, exact six loader descriptors/requirements, and the sole `LoaderKind` binding in `src-tauri/modules/flint-feature-loaders/src/lib.rs`. Legacy loader adapter/API alternatives and waivers are rejected fail-closed. Phase 5 checks remain inactive.

Consequences for migration:

1. Keep `src-tauri/Cargo.toml` as the implemented workspace root and single Cargo package-version source: keep the host `[package] version` equal to `[workspace.package] version` unless the release tooling and documented paths are changed in the same scoped change. Non-root members must continue to declare `version.workspace = true`.
2. Keep the host package `flint` and its `src-tauri/src` location. A workspace root can also be a package root; member crates can live under `src-tauri/crates`, `src-tauri/modules`, and `src-tauri/providers`.
3. Keep one lockfile at `src-tauri/Cargo.lock` and avoid member-local lockfiles.
4. Keep the default target directory and executable names stable until the release workflow and validation scripts have been deliberately updated.
5. Keep `main.rs` as the executable entry and `flint_lib::run()` as the library entry unless a release-path migration is explicitly approved.
6. Do not move or redefine the Tauri configuration or frontend output path during the architecture migration.

## Target architecture (Partially implemented; full extraction and typed provider ports pending)
The target layer model remains normative. The current tree has the implemented nineteen-crate workspace foundation, generic provider API/registry, seven extracted settings/saves/accounts/vanilla/launch/mods/loaders feature crates, typed mod provider contract with two extracted mod leaves, six typed loader provider leaves with host adoption, host feature lifecycle/availability integration, and a frontend contribution registry with all four slots mounted and snapshot filtering. The nineteen-crate layout and all seven feature crates are present; full provider runtime lifecycle and Phase 5 remain target-only, while the Phase 4 Cargo-metadata/source/typed-port gate is active. The table below describes required end-state boundaries, not the full current crate layout.

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

### Recommended workspace layout (Planned; foundation crates exist)

The workspace root and the `flint-core`/`flint-feature-api`/`flint-provider-api` foundation crates are implemented. The remaining paths are a target layout, not a statement that every listed crate exists. Names may be adjusted during extraction, but the ownership and dependency rules must remain.

```text
src-tauri/
  Cargo.toml                      # host package + workspace root; [workspace.package] version source
  Cargo.lock                      # one workspace lockfile; every local package version
  tauri.conf.json                 # existing path; unchanged during extraction
  build.rs                        # existing path; host build script
  src/                            # existing host location; remains thin
    main.rs
    lib.rs
    composition/                  # explicit feature/provider assembly
    commands/                     # thin Tauri command adapters by feature
    adapters/                     # DTO, event, dialog, process, and framework adapters
  crates/
    flint-core/                   # implemented shared policy, typed port contracts, and contract primitives
flint-feature-api/            # implemented feature descriptor, validated registry, lifecycle coordinator, and host-consumed availability DTO
    flint-provider-api/           # implemented generic Provider contract and registry
  modules/
    flint-feature-accounts/
    flint-feature-launch/
    flint-feature-vanilla/
    flint-feature-loaders/
    flint-feature-mods/
    flint-feature-saves/
    flint-feature-settings/
  providers/
    flint-mod-provider-<provider-id>/
    flint-loader-provider-<loader-id>/
```

The exact number of feature crates is not fixed in advance. The table is a candidate responsibility map, and a module should be extracted only when its public contract and dependencies are stable enough to justify the crate boundary. `models.rs` and other shared DTOs should be split by ownership: feature-specific DTOs belong with the feature contract, provider payloads stay private to providers, and `flint-core` must not become a dumping ground for every serializable type.

The host package keeps the existing `flint` package name, `flint_lib` library name, `src-tauri` root, lockfile, Tauri configuration, target directory, and executable names. The composition root explicitly lists modules and providers; there is no runtime directory scan for crates or provider libraries.

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
10. Cargo manifests and TypeScript imports define actual edges, but neither Cargo nor TypeScript inherently enforces an arbitrary layer allowlist. The active Phase 4 checker in the release workflow `prepare` gate enforces frontend manifest/composition/import boundaries plus module/provider import and selection boundaries, forbidden std/process/fs/net/reqwest/Tauri patterns in all extracted provider leaves, exact settings/saves/accounts/vanilla/launch/mods/loaders feature and mod/loader provider dependency/source/composition boundaries, all nineteen workspace production sources, exact Cargo internal/external dependency rules and cycles, typed provider context ports, both immutable composition registries, the sole loader-feature binding and thin host loader event adapter, exact six loader descriptor/key requirements, forbidden feature Tauri/host-private imports, and exact allowlists. Full provider runtime lifecycle hardening and Phase 5 remain pending.

Frontend:

1. `src/app` is the only layer allowed to import concrete feature modules for assembly. `src/core` and `src/shared` cannot import `src/app` or concrete feature modules.
2. A frontend feature module may import only frontend core/shared APIs and its own files. It cannot import another feature module at all, including that feature public entry, and cannot reach into another feature store.
3. Cross-feature UI composition, state, actions, and navigation are performed through typed core contracts, shell services/contexts supplied by the app composition root, or contribution slots. No feature imports another feature module as a cross-feature API.
4. Domain/API behavior belongs in feature services or the shared API client, not in generic shared components.
5. The composition root creates and freezes the frontend registry before rendering. Components read the registry and typed shell services; they do not mutate global registration state at render time.

### Architecture boundary checker (Phase 4 gate active; Phase 5 pending)

The checker is implemented at `scripts/check-architecture.mjs`, with automated coverage in `scripts/test-architecture-checker.mjs`. It is phase-aware, fail-closed, and reads checked-in reviewed allowlists. A broad directory or package wildcard is not an allowlist. Temporary waivers name the exact rule, file/package, phase, owner, and removal condition and fail closed when the phase ends.

Current integration status:

- The release workflow `prepare` job installs frontend dependencies, runs the checker test suite, then runs `node scripts/check-architecture.mjs --phase 4`.
- All build-matrix jobs depend on `prepare`; the architecture tests and Phase 4 checker are the single release-prepare gate.
- The release workflow `prepare` gate checks the Phase 1/2/3 milestone boundaries, the exact Phase 4 nineteen-package Cargo graph and source roots, internal/external dependency allowlists and cycles, loader registry injection, typed provider contracts, and the host 30-command/28-feature-command gate map. Full provider runtime lifecycle hardening and Phase 5 are not claimed as complete.

#### Rust checks

- The active Phase 4 gate combines static Rust source analysis with frontend AST checks and invokes Cargo only for workspace/package/dependency metadata. It masks comments/strings, scopes test-only modules correctly, scans every production Rust source under all nineteen crates, enforces reviewed module/provider import and selection boundaries, rejects forbidden std/process/fs/net/reqwest/Tauri patterns and direct `File`/socket/process APIs in provider leaves, checks exact Cargo internal/external dependency allowlists and cycles, requires typed `LoaderProvider`/`ModProvider` contracts and typed context-port use, requires both provider registries to be constructed only in `provider_composition.rs`, enforces the sole `LoaderKind` binding in `src-tauri/modules/flint-feature-loaders/src/lib.rs`, and uses exact allowlists. Phase 5 remains future work.
- The active Phase 4 gate runs `cargo metadata --manifest-path src-tauri/Cargo.toml --no-deps --format-version 1` and uses that output only for workspace/package/dependency truth. It requires the exact nineteen-package set, one root lockfile, version/resolver coherence, exact internal edges, exact normal/build/dev external dependency allowlists, and no dependency cycles. Cargo-unavailable or invalid-metadata failures are fail-closed.
- Phase 4 crate rules are active: reject host/Tauri dependencies from core, API, feature, and provider crates; reject cycles; reject provider-to-module, provider-to-provider, and module-to-private-module edges; allow only explicit cross-feature public Rust API edges. Frontend feature imports are stricter and never cross feature boundaries.
- Phase 4 source rules reject forbidden direct I/O, network, process, and framework imports in feature/provider implementation code. Core port implementations and host adapters are the only approved homes for underlying I/O/process/framework calls.
- Manifest checks reject dependencies that would bypass approved ports; source checks catch standard-library or macro paths that Cargo metadata cannot see. The checker is not a security sandbox: review must reject obfuscation, FFI, or unsafe escape hatches intended to avoid the allowlist, and unsafe FFI is prohibited absent a separate accepted decision.

#### TypeScript checks

- The checker uses the installed TypeScript compiler API to parse every `.ts`/`.tsx` file and collect static imports, re-exports, side-effect imports, `import type`, `require` forms if any, and dynamic `import()` calls.
- The implemented target frontend rules are strict: `src/app` is the only assembly layer allowed to import all concrete feature modules; `src/core` and `src/shared` cannot import `src/app` or `src/modules`; a feature module may import only its own subtree plus frontend core/shared APIs. No feature may import another feature module at all, including that feature public entry. Cross-feature composition/state/actions use typed core contracts, shell services/contexts, or contribution slots.
- Framework imports are restricted to approved transport/adapter layers. Feature components should call typed API/event clients rather than importing Tauri packages directly; reviewed temporary Phase 5 waivers currently remain in the checker.
- External packages are part of the allowlist. Adding a new package to a feature requires dependency review, not merely an import that happens to compile.
- Dynamic imports are allowed only with exactly one string-literal, relative, bundler-resolvable specifier inside the importing feature's own subtree or an explicitly allowlisted local chunk. Reject variable/template-computed specifiers, absolute paths, file/HTTP URLs, package specifiers for contribution loading, `eval`, and `new Function`.
- A contribution loader is a component reference or a locally declared lazy function. It must never be a string path that the registry evaluates at runtime. The checker and bundler can prove a literal import target; the registry cannot runtime-prove an arbitrary filesystem or URL string.
- The active release `prepare` gate runs the Phase 4 static manifest, composition-root, import, Cargo-metadata, all-workspace source, and typed-port checks. It does not re-implement runtime duplicate-ID/navigation/ordering checks or claim full provider runtime lifecycle acceptance; Phase 5 remains pending.

#### CI gate

- CI failure blocks completion; there is no warn-only mode.
- The current gate is the release workflow `prepare` job at Phase 4. The implemented checker milestone covers frontend boundaries, module/provider import and selection boundaries, forbidden std/process/fs/net/reqwest/Tauri patterns in all provider leaves, exact settings/saves/accounts/vanilla/launch/mods/loaders feature and mod/loader provider dependency/source/composition checks, exact six loader descriptors/requirements, both immutable composition registries, the sole `LoaderKind` binding in the loader feature crate, forbidden feature Tauri/host-private imports, and exact allowlists. It establishes the Phase 4 Cargo-metadata/source/typed-port gate; full provider runtime lifecycle and Phase 5 remain pending.
- The workspace root and Phase 4 Cargo-metadata/source/typed-port gate are active. The six loader provider leaves and host typed loader adoption are implemented at milestone level; full provider runtime lifecycle acceptance and Phase 5 remain pending.
- Any allowlist change is an architecture change and must be reviewed as such.

## Rust feature contract and lifecycle registry (Feature API and host lifecycle/availability milestone implemented; Phase 5 pending)
Status: **Feature API and Tauri host lifecycle/availability milestone implemented; Phase 5 pending.** The crate provides validated feature/contribution IDs, data-only descriptors with separate registration/initializer and runtime-handle contracts, a deterministic feature registry, a provider-catalog abstraction, the lifecycle state machine/failure propagation/gating/shutdown coordinator, and the read-only availability snapshot DTO. The host composes seven explicit features and real mod/loader provider metadata in `feature_host.rs`, starts through Tauri setup, owns poisoned-lock-safe managed runtime state, exposes the ungated `get_host_availability` command, gates all 28 feature commands, filters frontend contributions from the snapshot, and invokes idempotent shutdown on `RunEvent::Exit`. All seven feature crates are extracted; full provider runtime lifecycle hardening and Phase 5 remain pending; the Phase 4 Cargo-metadata/source/typed-port gate is active.

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

Initialization is registered separately from the descriptor as a `FeatureInitializer`; a successful initializer returns a runtime handle with lifecycle methods such as `health` and `shutdown`. Initialization must be atomic from the registry's perspective: if it returns an error after acquiring resources or starting workers, the feature itself must release those partial resources before returning the error. The registry cannot infer a failed feature's partial state. The initial implementation should keep the lifecycle small and explicit; do not invent a general-purpose dependency-injection framework.

### Feature ID and version rules

- IDs are compared exactly after validation; no case-insensitive aliases, display-name matching, or auto-generated IDs.
- A renamed capability keeps the old ID as a documented alias only during a migration and must update every contribution and contract reference in the same change. Prefer adding a new versioned capability over silently changing semantics.
- `feature_version` describes the feature implementation; `api_version` describes the feature contract boundary; `provider_version` describes a provider implementation. Do not overload one field for another.
- Provider IDs follow the same stable-ID discipline and are distinct from feature IDs.
- Backend registration IDs are globally unique across the backend registry and prefixed with the owning feature ID. Frontend contribution IDs are a separate namespace and use the single global-across-all-slots rule in the frontend section.

## Rust feature registration and lifecycle (Feature API registry/coordinator and host integration milestone implemented; Phase 5 pending)
Status: **Feature API and host integration milestone implemented; Phase 5 pending.** `flint-feature-api` validates feature metadata before side effects, builds a deterministic dependency graph and topological order, coordinates provider/feature startup and failure propagation, exposes gated runtime handles, shuts down in reverse order, and produces the availability DTO. The Tauri host owns/starts the coordinator, publishes the snapshot through the ungated availability command, gates feature commands, filters frontend contributions, and consumes exact mod/loader descriptors and requirements from `provider_composition.rs`. All seven feature crates are extracted; full provider runtime lifecycle hardening and Phase 5 remain pending; the Phase 4 Cargo-metadata/source/typed-port gate is active.

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
| **4. Freeze and availability publication** | If no fatal failure occurred, freeze registries and publish host state. In Phase 4, publish the read-only active-feature snapshot to the shell. | `Ready` when every `Required` feature and all optional features are ready. `Degraded` only when every `Required` feature remains `Ready` and one or more affected `Optional` features are `Failed`/`BlockedDependency`; any affected `Required` feature makes this a `StartupFailed` path instead. |
| **5. Running** | Commands and UI operate against ready feature runtimes, a validated active provider set, and active contributions. | A command for a non-ready feature or unavailable provider returns the documented unavailable error. No silent fallback. |

Rules for Stage 3 and later:

- A feature's contributions are staged during descriptor validation but become visible/active only after that feature reaches `Ready`. Contributions from `Failed`, `BlockedDependency`, `Starting`, or `Stopped` features are not visible.
- Failure propagation always includes the failed feature plus all transitive dependents. Mark the failed feature `Failed`, its transitive dependents `BlockedDependency`, deactivate all affected contributions, and skip initialization of affected dependents.
- Effective criticality is determined from that entire affected set, never from the originally failed feature alone. If the affected set contains any `Required` feature, the host takes the fatal `StartupFailed` path: stop starting further features, do not initialize affected dependents, and clean up already initialized state.
- If the affected set contains only `Optional` features and every `Required` feature remains `Ready`, the host may enter `Degraded`. Independent ready features stay active; required behavior is not silently routed to another feature.
- A `Required` feature must never remain `Ready` when any required dependency or provider is not `Ready`. Such a state is a required failure and follows the fatal path.
- A failed initialization is atomic from the registry's perspective. The feature must clean up all partial resources before returning the error; the registry records the failure and does not hold a runtime handle for it.
- The Phase 4 host lifecycle/availability milestone exposes `get_host_availability` as an ungated, contract-documented command. The frontend receives the read-only availability snapshot and filters navigation, pages, settings sections, and instance actions by the owning feature active state. Phase 3 itself remains independent of Rust lifecycle state.

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

### Tauri run-event integration (Planned)

The Tauri host adapter owns the lifecycle bridge; no feature or provider imports `RunEvent` or an `AppHandle`.

- A single host shutdown coordinator is registered with the Tauri run loop. The planned final integration point is `RunEvent::Exit`, with `RunEvent::ExitRequested` reserved for exit policy or cancellation intent rather than final resource ownership.
- On normal final exit, the coordinator transitions the host to `Stopping`, invokes feature shutdown in reverse successful-init order, stops provider leaves once unused, shuts down core services, and finally marks `Stopped`.
- On startup-fatal failure (the propagated affected set contains a `Required` feature), the same coordinator runs immediately for successfully initialized features, then provider leaves, then core services before the host settles in diagnostic `StartupFailed` mode or exits.
- On runtime-fatal failure (the propagated affected set contains a `Required` feature), the adapter enters `RuntimeFailed`, starts the same coordinator without waiting for normal exit, and transitions through `Stopping` to `Stopped` or process exit after diagnostics.
- Shutdown must be safe to invoke more than once and must not hold the UI/window thread indefinitely. The adapter may bound or asynchronously observe shutdown tasks while still preventing a false `Ready` state.

### Determinism rules

- Feature initialization order is topological by declared dependencies, with lexicographic feature-ID tie-breaking.
- Provider leaves initialize in lexicographic provider-ID order after descriptor/capability validation.
- Lists exposed to UI have deterministic order: explicit numeric `order`, then stable contribution ID.
- Provider selection uses the full compatibility/selection predicate in the provider section, never `HashMap` iteration or "first match".
- Retry/backoff, if later added, must not make initialization order depend on wall-clock timing.
- All registry mutations are forbidden after freeze; dynamic plugin loading is deferred and cannot use the stable registry as an unreviewed escape hatch.

## Provider contracts (Phase 1/2 milestones implemented; runtime lifecycle hardening pending)

The provider layer separates reusable feature behavior from provider-specific external I/O. The typed mod/loader provider contracts and registries live in `src-tauri/crates/flint-provider-api`, with CurseForge/Modrinth and the six loader implementations extracted under `src-tauri/providers`. `provider_composition.rs` owns both immutable registries; the loader feature crate owns the sole `LoaderKind` binding, while the host `src-tauri/src/loaders.rs` adapter only adapts progress sinks/events. Existing serialized `ModSource`, `LoaderKind`, and DTO contracts remain unchanged. All seven feature crates are extracted; full provider runtime lifecycle remains pending.

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
- Current registry construction sorts providers by stable `ProviderId`, validates all descriptors/capability bindings, and never depends on `HashMap` iteration, filesystem discovery, or registration order. Full provider initialization/lifecycle coordination remains Phase 4 work.

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
Current implementation status: the host composes one immutable mod-provider registry and one immutable loader-provider registry at startup, and provider-backed commands reuse them through managed state. `flint-feature-api` defines the framework-neutral provider catalog/lifecycle abstraction; `src-tauri/src/feature_host.rs` builds a real-metadata catalog from both composition helpers. The six loader leaves, typed host loader adoption, and settings/saves/accounts/vanilla/launch/mods/loaders feature crates are implemented at milestone level; full mutable provider runtime health/lifecycle integration remains pending; the Phase 4 checker gate is implemented and Phase 5 remains pending.

### `ModProvider` (typed API and implementation crates implemented; full provider phase pending)

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

Mod and loader providers now follow these context expectations. The six extracted loader crates receive typed list/install contexts built by the host from the injected registry and core ports.

- replace feature-local HTTP/credential details with approved core network, safe instance/path, scoped settings/credential, logging, and cancellation/deadline ports;
- keep final mod placement and sidecar writes in the shared mod feature;
- forbid direct Tauri handles, raw windows, unrestricted filesystem roots, unrestricted process runners, and direct settings-file paths.

### `LoaderProvider` (typed API/core support and six leaves/host adoption implemented; full runtime pending)

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
- The typed API list context carries network/workspace/progress/logger/deadline/cancellation plus `LoaderTimeouts`. The install context carries separate metadata and installer `NetworkPort`s, safe workspace/`PathPort`, `JavaRunner`, progress/logger, root, deadline/cancellation, and the same timeout bundle. The host constructs these typed contexts directly; transitional client/request helper contexts are removed.
- `src-tauri/src/loaders.rs` retains only host orchestration: request validation, vanilla baseline preparation, typed context construction, one injected-registry selection through the `LoaderKind` binding, progress/event adaptation, profile verification, and final `LoaderInstallResult` assembly. The six implementation leaves live in extracted provider crates and are instantiated only by `provider_composition.rs`.
- The six provider implementations, typed host adoption, injected immutable registry, progress/event mapping, and 75/600/600 timeout split are implemented at milestone level. Full provider runtime lifecycle hardening and Phase 5 remain pending; the Phase 4 checker gate is active.

Typed context expectations are implemented in `flint-provider-api`: approved core network/download, safe workspace/path, Java-runner, progress/logging, timeout, deadline, and cancellation ports. The six provider crates and injected-loader-registry host adoption are implemented at milestone level; full provider runtime lifecycle remains pending.

Loader providers may run only trusted installer flows, with bounded processes, captured/redacted output, timeouts, and safe archive policy. A mod provider must never execute a downloaded mod archive.

### Enforcing shared ports

Phase 4 checker status:

- The active Phase 4 checker enforces frontend manifest/composition/import boundaries plus module/provider import and selection boundaries, typed provider contracts and typed context-port use, all-workspace source roots, exact Cargo internal/external dependency rules and cycles, exact six loader leaf manifests/targets and descriptor/key requirements, one mod and one loader composition registry, the sole `LoaderKind` binding in `src-tauri/modules/flint-feature-loaders/src/lib.rs`, and exact allowlists. Legacy loader adapter/API alternatives and waivers are rejected. Cargo is invoked only for metadata truth; Phase 5 remains pending.
- The shared core port contracts exist. Extracted mod and loader providers use the typed network/path/process/progress/logging/deadline/cancellation contexts. Full provider runtime lifecycle remains incomplete.
- The current checker contains transitional Tauri import waivers that expire in Phase 5; the current active gate is Phase 4.

Active Phase 4 rules and Phase 5 expectations:

- Extracted provider and feature-module Cargo manifests may depend only on reviewed phase allowlist entries. Direct Tauri, HTTP, process, and filesystem implementation dependencies are rejected where the layer must use ports.
- The source checker rejects restricted standard-library paths and calls such as `std::fs`, `std::process`, `std::net`, `Command::new`, and direct process spawning in module/provider implementation code once the corresponding ports exist.
- Only core implementations and host adapters may implement the network, path, Java-runner, settings, logging, and progress ports.
- Provider API crates expose context/port traits, not a handle to a global adapter, so a provider cannot obtain unrestricted file/process/network access through the contract.
- The checker is not a sandbox. Review rejects obfuscation, unsafe FFI, macro tricks, or new manifest paths intended to evade the allowlist; any temporary exception is explicit, scoped, and phase-expiring.

The checker gate is active at Phase 4. It enforces typed provider context ports, all-workspace source boundaries, and the Cargo metadata gate. Phase 5 hardening remains pending. See **Architecture boundary checker (Phase 4 gate active; Phase 5 pending)** above for current and target responsibilities.

### Enum and DTO compatibility during migration
The Phase 1/2 registry/selection milestones, the typed mod provider extraction, the six typed loader provider leaves, and typed host loader adoption are implemented without changing public commands, DTOs, enum serialization, or events. Mod selection uses the sole `ModSource` binding in `flint-feature-mods`; loader selection uses the sole `LoaderKind` binding in `flint-feature-loaders`. Provider-internal IDs/versions remain internal. Full provider runtime lifecycle acceptance and Phase 5 remain future work.

- `ModSource` and `LoaderKind` remain serialized enum/DTO values while the frontend contract depends on them.
- The host translates deserialized `ModSource` to a complete provider requirement plus credential kind in the feature `source_binding` function, and translates `LoaderKind` only inside the feature `loader_binding` function. Shared feature services use registry lookup, not provider-specific branching.
- Unknown enum values fail during deserialization as they do today.
- Adding or removing an enum variant is a public contract change: update Rust and TypeScript DTOs and the affected authoritative contract together.
- Provider IDs, provider versions, capability versions, and provider payloads are internal. They must not replace or appear inside public DTOs unless a contract change explicitly introduces them.
- Provider-internal response types are normalized and validated into public DTOs before crossing the adapter boundary.
- Provider API/capability versions are separate from the public RPC contract version and from `feature_version`.

### Provider migration order and status

1. **Milestone implemented; full phase pending:** introduced the provider API, capability model, operation contexts, and provider registry inside the current crate.
2. **Milestone implemented; full phase pending:** implemented the typed ModProvider API and two extracted provider implementation crates while keeping command wrappers, DTOs, serde shapes, and events stable.
3. **Milestone implemented; full phase pending:** moved mod-source dispatch to the provider registry and kept shared dependency/install logic in the mod feature.
4. **Milestone implemented; full phase pending:** repeated the migration for LoaderProvider, including typed list/install contexts, six independent leaves, and the preserved metadata/installer timeout split.
5. **Pending Phase 5:** complete provider runtime lifecycle integration/hardening and Phase 5 enforcement. The Phase 4 Cargo-metadata/source/typed-port gate is active. The generic provider API, typed mod/loader provider APIs, two mod implementation crates, six loader implementation crates, feature lifecycle API, and host composition are implemented at milestone level.

## Tauri host and adapter boundary (Planned)
Status: **Partially realized.** Commands remain compile-time registered in `src-tauri/src/lib.rs` and call the current modules. Mod/loader provider selection is now inside shared feature modules, not command bodies. Separate command-adapter modules, a Rust composition root, and the full Tauri run-event lifecycle remain Phase 4 work. The target adapter rules below remain mandatory for future extraction.

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
- Listener cleanup, duplicate emissions, and failed listener registration must be handled on the frontend. Event payloads must not contain credentials, tokens, or unredacted commands. Delivery failure of an explicitly required protocol event must not be silently suppressed.

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
- Cancellation is not promised unless a contract defines it; if a request becomes stale, the frontend must still ignore/clean up its own result safely.

## Frontend architecture and contribution registry (Phase 3 registry and Phase 4 availability filtering implemented; full extraction pending)
Status: **Phase 3 implemented; Phase 4 availability filtering milestone implemented.** The typed registry/contracts, frozen validation, explicit static composition root, and registry-driven rendering for pages, navigation, settings sections, and instance actions are implemented. The settings feature contributes five mounted settings sections, and the launch feature contributes a mounted instance action rendered through a shared core renderer. The Phase 4 availability client validates `get_host_availability` and filters slots by host/feature readiness; complete core/shared adapter extraction remains later-phase work.

### Target frontend layout

```text
src/
  main.tsx                    # existing entry; mounts the composed application
  app/
    bootstrap.tsx             # composition root and root providers
    AppShell.tsx              # generic shell; no hard-coded feature list
    registry.ts               # create/validate/freeze frontend registry
  core/
    api/                      # typed command client built on the contract
    events/                   # typed event subscriptions and cleanup
    errors/                   # shared error presentation helpers
    types/                    # cross-feature shell and contribution types
  shared/
    components/               # generic presentational components
    hooks/                    # generic reusable hooks
    utils/                    # pure generic helpers
  modules/
    launch/
    downloads/
    accounts/
    saves/
    settings/
    about/
    <future-feature>/
      index.ts                # public module export and contribution list
      components/             # private implementation
      state/                  # feature-owned state
```

This tree is the target shape, not a complete current tree. The implemented subset is `src/app/` (`App.tsx`, `AppSidebar.tsx`, `composition.ts`), `src/core/` (`app-context.ts`, `availability.ts`, `contributions.ts`, `ContributionBoundary.tsx`, `InstanceActionRenderer.tsx`), and `src/modules/` feature modules with mounted page, navigation, settings-section, and instance-action contributions plus snapshot-driven availability filtering. `src/shared/`, the remaining `src/core` transport/event/error/type extraction, and a separate generic `AppShell`/bootstrap/provider layer are still target-only. `src/main.tsx` currently mounts `src/app/App.tsx` directly.

### Contribution slots

The frontend registry accepts typed contributions for at least these slots:

| Slot | Required target semantics | Current implementation status |
| --- | --- | --- |
| **Navigation** | `id`, owning `featureId`, label, icon, `targetPageId`, numeric order, and optional typed visibility/capability selector. `targetPageId` must equal the canonical `id` of a registered page contribution. The shell renders the ordered list; it must not maintain a second hard-coded nav array. | Implemented and rendered. Current modules provide ordered nav contributions and optional indicators. |
| **Page** | `id` (the canonical page contribution ID and navigation target), owning `featureId`, title, component or local lazy-loader, numeric order, and any declared shell inputs. An optional URL path may be presentation metadata, but it is not a second identity or target. A page is mounted only through the shell contribution boundary. | Implemented and rendered through `ContributionBoundary`; current modules use static component references. |
| **Settings section** | `id`, owning `featureId`, title, component or local lazy-loader, order, and a declared settings namespace/validation hook. Sections edit through the shared settings owner; they do not write their own copy of the settings file. | Implemented and mounted. The settings feature registers five section components with stable IDs, namespaces, order, and validation hooks backed by the existing central validators where applicable. `SettingsView` renders them from `AppPageContext.settingsSections` through per-contribution boundaries. |
| **Instance action** | `id`, owning `featureId`, label, icon, order, typed enabled/visible selector, and an action handler receiving an `InstanceActionContext` with the selected instance, effective game directory, and approved navigation/API services. Examples include launch, open directory, manage content, and open a launch log. | Implemented and mounted. The launch feature registers a launch-navigation action; `src/core/InstanceActionRenderer.tsx` builds the action context and the versions page header renders the registered actions. |

A contribution declaration must be data plus a locally bundled component/function reference only. It must not perform registration side effects such as I/O, network calls, timers, event listeners, or stores during module evaluation. A contribution loader is never a runtime string path: the registry can validate metadata, but only the TypeScript AST checker and bundler can prove that a literal module specifier resolves to bundled code.

### Registry rules
The runtime Phase 3 registry validates feature descriptors and all four contribution-slot shapes, enforces globally unique IDs/page targets/ordering/feature requirement cycles, and freezes the registry; the frontend mounts every slot from `frontendRegistry`. The active static checker at `--phase 4` verifies feature-manifest exports, explicit composition-root coverage, and import/dynamic-import boundaries; it does not duplicate the runtime duplicate-ID, navigation-target, or ordering validation. The rules below remain the full target contract.

- Every frontend contribution has a globally unique stable ID across every frontend slot, not merely within its slot, and is prefixed by the owning feature ID (for example `flint.mods.page.discovery`).
- Any duplicate contribution ID across any slot is rejected before render; last-registration-wins is forbidden. Navigation targets must resolve to exactly one registered page contribution by its canonical page ID.
- The composition root explicitly imports approved feature modules and passes their contribution lists to the registry builder. Glob-based auto-discovery, remote manifests, and runtime module URLs are forbidden in the initial architecture.
- The registry validates required fields, numeric finite ordering, global ID uniqueness, known `targetPageId` references, owning feature IDs, and local lazy-loader shape. A URL path is never used to resolve a navigation target.
- Ordering is `order` ascending, then stable contribution ID lexicographically. The shell must not depend on insertion order from an object or module loader.
- The registry is frozen before React renders. No feature can add, replace, or remove a contribution while the app is running. A later dynamic plugin proposal must not bypass this rule.
- Unknown page IDs produce a deliberate fallback or diagnostic page. They must not accidentally render another feature.
- The shell owns the selected page contribution ID and rendering boundary. Conditional feature logic belongs in typed selectors or feature-owned components, not in a growing chain of shell checks.
- The Phase 4 host availability snapshot is implemented. The shell filters contributions against it; contributions owned by a feature that is not `Ready` are not mounted in normal navigation, page, settings, or instance-action slots. A diagnostics surface may list unavailable features and reasons; it must not fetch remote code or substitute another feature. Phase 3 historically rendered the statically registered compiled set.

### Composition root

Current implemented sequence:

```text
src/main.tsx
  -> src/app/App.tsx
  -> src/app/composition.ts imports approved feature modules
  -> buildContributionRegistry validates/freezes frontendRegistry
  -> AppSidebar renders navigation and App renders the active page contribution
```

`src/app/composition.ts` is the only current frontend file that imports the complete feature-module list. The shell keeps shared state and passes the frozen `settingsSections` and `instanceActions` through `AppPageContext` into page components. All four contribution slots are live.

Target evolution: install the remaining shared core/event/error/type providers and a fully generic shell. The Phase 4 host active-feature availability snapshot/filter is implemented; typed mod/loader provider adoption and the six loader leaves are implemented at milestone level. Remaining work is full provider runtime lifecycle acceptance and Phase 5 hardening; the Phase 4 checker gate is active. The frontend composition root remains the only place that imports concrete feature modules for assembly.

### State ownership
Current Phase 3 state: `src/app/App.tsx` owns shared shell state and exposes `AppPageContext`, including the frozen contribution lists. Feature modules own their section/page presentation state; registered settings sections use the shared draft and action callbacks, and registered instance actions use the typed action context. Accounts contributes a dialog surface while the shell still owns shared account state. The table below is the target ownership model.

| State | Owner |
| --- | --- |
| Selected instance and effective game directory | App/shell-level typed context, backed by the instance/settings contracts. |
| Saved and draft settings | Settings feature/service owning the single settings workflow; other settings sections use the shared draft interface. |
| Active account and account list summary | Accounts feature/public context; the shell may display a read-only summary. |
| Per-feature query results, progress, modal state, and form state | The owning feature module. |
| Cross-feature notices | App-level notice service with explicit typed API. |
| Registry and contribution list | App composition root; immutable after freeze. |
| Backend authoritative data | The owning backend feature, exposed through the typed API client and contract DTOs. |

No feature may import another feature module, its store, or its public entry. Cross-feature actions, state, and navigation pass through typed core contracts or shell services/contexts supplied by the app composition root; contribution slots compose the resulting UI.

### Static imports and lazy loading
Current Phase 3 contributions use static component references. Local lazy contribution loading remains a target capability. The checker/test suite contains the static import/dynamic-import rules, and the release `prepare` gate is active at `--phase 4`.

- Contributions use static imports or a local `lazy(() => import("./Page"))` whose specifier is a literal bundled path.
- Lazy loading splits the application's own build output. It does not load arbitrary JavaScript from disk, a URL, a provider, or a plugin package.
- Only `import()` with exactly one string-literal, relative, bundler-resolvable specifier is allowed, and the target must be inside the importing feature subtree or an explicitly allowlisted local chunk. Variable/template-computed specifiers, absolute paths, file/HTTP URLs, package specifiers used as contribution loaders, `eval`, `new Function`, `document.write`, remote component URLs, runtime module federation, and untrusted HTML injection are prohibited.
- Contribution loaders are component references or locally declared lazy functions, never runtime string paths or URL values. The frontend registry cannot prove that an arbitrary runtime string refers to bundled code; the TypeScript AST checker and bundler resolve and verify literal local imports at build time.
- No feature may fetch and execute UI code based on a backend descriptor. The backend can describe capability availability, but executable UI is shipped and reviewed with the application.

## Cross-cutting rules

### Errors and recovery

- Treat all frontend input, provider payloads, local metadata, archives, and process output as untrusted.
- Validate at the boundary and again where a lower layer relies on invariants. Use bounded sizes, counts, depths, and timeouts.
- Return actionable errors with operation context; do not panic on malformed local files, network failures, or provider responses.
- Public command errors retain the contract's current display-ready string shape unless a deliberate contract change is approved. Internal error variants should remain typed enough to map consistently.
- A failed operation must not leave a false success state. Partial filesystem changes must be described in the result or cleaned up according to the operation's contract.
- Command results and committed durable state are authoritative. Default progress-event delivery is best-effort: a delivery failure is logged and does not fail an otherwise successful command or roll back committed state. If a contract explicitly marks an event as a required protocol step, failure to deliver it must return an error or typed partial-success result and must not report success; committed durable state is still not rolled back.
- Never include passwords, access tokens, refresh tokens, API keys, authorization headers, or unredacted launch commands in logs, events, errors, or UI notices.

### Events

- Event names and payloads are public contract. Existing `flint://...` names and DTO payloads must not be renamed as an internal refactor. Command results and committed durable state are authoritative; default progress events are best-effort and never roll back committed state. Explicitly required protocol events must fail the operation result when delivery fails, while still preserving already committed state.
- Event publication goes through a host adapter/port. Core, feature, and provider code must not import Tauri solely to emit events.
- Emit progress from the owning operation, not from unrelated retries or stale requests. Consumers must ignore stale results and unlisten on unmount.
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
- UI request IDs or equivalent guards must prevent stale completions from overwriting newer state.

### Security boundaries and limits

- The Tauri IPC/frontend boundary is trusted only for shape, never for safety. Commands revalidate all paths, identifiers, sizes, and operation context.
- External requests require HTTPS, bounded response bodies, timeouts, and redirect/protocol review. Providers may not bypass the shared network policy.
- Archive handling must reject traversal, absolute paths, duplicate/unsafe names, symlink escapes, and decompression/size bombs within documented limits.
- Downloaded mod archives are inspected but never executed by the launcher. Loader installer execution is limited to the trusted flows defined by the feature contract and a bounded Java process runner.
- Provider and feature IDs are not filesystem paths. Never derive a write location from a display name or unvalidated provider string.
- Do not broaden Tauri capabilities, window permissions, or the current content security configuration as an incidental part of feature work. Any security-surface change needs explicit review.
- Runtime native plugins are deferred because they introduce ABI stability, signing, isolation, crash containment, resource limits, and arbitrary-code trust problems that this design does not solve.

### Logging and observability

- Log operation, feature/provider ID, and a safe path or object identifier where useful.
- Redact secrets and sensitive headers. Prefer concise English diagnostics over dumping raw external responses.
- Do not log full filesystem contents, user tokens, or entire provider payloads.
- Errors surfaced to the user should be concise and actionable; detailed technical context may go to a safe log.
- Health and registry snapshots should expose stable IDs, versions, dependency state, and failure reasons without leaking credentials.

## Migration phases (Implementation status)

The sequence is deliberate. Do not start a later phase by silently creating its target structure while earlier contracts are unverified. Each phase must preserve the existing external contracts, name any intentional deviation, and satisfy the phase applicability matrix in [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md).

### Phase 1: ModProvider (Partial: typed API, contexts, and implementation crates implemented; Phase 5 integration pending)

Status: **Milestone implemented; full provider runtime phase pending.** The provider-neutral mod and loader contracts/registries are implemented in `src-tauri/crates/flint-provider-api`; the CurseForge/Modrinth and six loader implementation crates use core ports; the host composes/injects one immutable mod registry and one immutable loader registry while retaining the sole `ModSource` binding in the mods feature and the sole `LoaderKind` binding in the loaders feature. The active `--phase 4` gate enforces exact provider source/dependency/composition boundaries, all eight extracted crates, both composition registries, and exactly one `source_binding`/`loader_binding`. All seven feature crates are extracted; full provider runtime lifecycle hardening and Phase 5 remain open, while the Phase 4 checker gate is active.

Scope:

- Defined the provider-facing mod API and versioned provider registry in `flint-provider-api`.
- Implemented the extracted CurseForge/Modrinth provider boundary for existing mod source behavior.
- Move source-specific search/version/dependency/artifact logic behind provider implementations.
- Keep the shared dependency planner, validation, archive inspection, sidecar handling, installed-mod behavior, DTOs, commands, and events in the mod feature.
- Keep `ModSource` as the serialized contract enum and perform enum-to-requirement/credential mapping only in the single feature `source_binding` function.
- Defined the versioned provider descriptor, per-capability API/selection-key bindings, full capability/provider-version compatibility predicate, independent provider leaves, and typed `ModProviderContext` ports described in this architecture.
- Implemented the first phase of `scripts/check-architecture.mjs` and its CI gate, including exact extracted mod provider source/dependency/composition boundaries in the available Rust module/import graph.

Target acceptance criteria (full phase not yet met):

- No command body or shared mod service branches on individual mod sources after lookup.
- Duplicate provider IDs, duplicate capabilities, malformed duplicate selection-key bindings within one capability, incompatible provider/capability API versions, and unmet provider version requirements are rejected explicitly.
- Provider leaves initialize in lexicographic provider-ID order and have no provider-to-provider dependency graph. Selection filters active providers by the full requirement predicate, then requires exactly one match; zero or multiple matches fail without fallback.
- Mod and loader providers receive typed network/path/process/progress/logging/deadline/cancellation ports; direct Tauri/host handles and unrestricted filesystem/process/network access are outside extracted provider boundaries. The active `--phase 4` gate enforces exact mod/loader provider import, selection, dependency, composition, six-leaf, and sole-loader-adapter checks. The Phase 4 Cargo-metadata/source/typed-port gate is active; full provider runtime lifecycle hardening and Phase 5 remain later gates.
- Unknown, incompatible, ambiguous, or unavailable providers return an actionable error and never fall back silently.
- All existing mod commands, inputs, results, events, and error behavior remain contract-compatible.
- The provider cannot emit Tauri events, choose arbitrary write destinations, execute a downloaded mod, or import private host state.
- The provider milestone does not itself complete loader provider crate extraction, full provider runtime lifecycle, a runtime plugin loader, or dynamic provider discovery; the implemented Phase 4 workspace foundation adds the core/feature/provider API crates, the two typed mod provider crates, and explicit host composition without changing public provider behavior.
- Applicable Rust and TypeScript checks pass in CI, and the reviewer verifies logic and boundaries without running local build/test commands.
- The implemented registry/selection milestone is backed by the fail-closed `scripts/check-architecture.mjs`, its tests, and the active Phase 4 CI gate, which includes the Phase 1/2 provider milestone checks for module/provider import and selection boundaries plus active typed-port enforcement. Full provider runtime lifecycle hardening remains Phase 5 work.

### Phase 2: LoaderProvider (Partial: loader registry/leaf milestone implemented; full LoaderProvider ports pending)

Status: **Milestone implemented; full runtime phase pending.** The six loader provider crates implement `LoaderProvider` through core ports, `provider_composition.rs` owns one immutable loader registry, and `src-tauri/modules/flint-feature-loaders/src/lib.rs` owns the sole `LoaderKind` binding and typed orchestrator while the host `src-tauri/src/loaders.rs` adapter adapts progress sinks/events. It preserves deterministic selection, the 75-second metadata/600-second installer HTTP/600-second Java split, progress/event mapping, vanilla preparation, profile verification/fallback, and unchanged DTO/command behavior. Full provider runtime lifecycle hardening and Phase 5 remain pending; the Phase 4 checker gate is active.

Scope:

- Define the loader provider API, descriptor, registry, and shared installation context.
- Replace `LoaderKind` call-site branches with one registry lookup and provider calls.
- Move loader-specific discovery/profile/installer behavior behind provider implementations while retaining shared vanilla preparation, download, path, Java, logging, progress, and result behavior.
- Keep all existing loader DTOs, commands, events, and UI behavior stable.
- Define the versioned `LoaderProvider` per-capability bindings and typed `LoaderProviderContext` ports for network, safe paths/archives, Java execution, vanilla preparation, progress, cancellation/timeouts, settings, and logging. Providers remain independent leaves initialized in lexicographic ID order.

Target acceptance criteria (full phase not yet met):

- Loader list and install commands no longer contain provider-specific branches.
- Providers initialize as independent leaves in lexicographic provider-ID order after descriptor/capability validation; no provider-to-provider dependency graph or provider-cycle detection exists. Duplicate provider/capability IDs and malformed capability/key bindings fail explicitly.
- Shared safety services are mandatory: HTTPS/download limits, safe archive writes, bounded Java execution, and redacted command/log handling.
- Existing loader install result and progress contracts are unchanged.
- No loader provider imports Tauri, feature internals, or another provider. Provider cross-coordination goes through core services or feature orchestration.
- Selection filters active providers by host provider-API compatibility, capability ID and capability-API compatibility, provider-version requirement, and optional per-capability selection key; exactly one match is required. Zero/multiple matches fail without `HashMap` ordering or first-match fallback.
- Loader providers receive typed list/install contexts with core network/download, safe workspace/path, Java-runner, progress/logging, timeout, deadline, and cancellation ports. Direct Tauri, HTTP, filesystem, or process implementations are forbidden and checked. Provider runtime lifecycle hardening and the Phase 5 acceptance gate remain pending; the Phase 4 Cargo-metadata/port gate is active.
- CI verification is green before completion and the phase is reviewed for logic only by an independent reviewer.
- The architecture checker allowlist is extended for the loader registry/leaf milestone and passes in CI. Full Phase 2 runtime acceptance is not claimed while provider lifecycle hardening remains Phase 5 work; the Phase 4 typed-port checker gate is active.

### Phase 3: Frontend contribution registry (Implemented)

Status: **Implemented.** Page, navigation, settings-section, and instance-action contributions, validation, the frozen registry, and registry-driven rendering are implemented. The settings feature registers five mounted sections; the launch feature registers a mounted instance action; shared rendering lives in core; and the Phase 3 checker gate is active in the release workflow `prepare` job.

Scope:

- Introduce typed contribution contracts and a registry builder in a new frontend composition root.
- Replace the static `navItems`, `pageTitles`/`SectionId` conditional rendering, hard-coded settings grouping, and fixed instance/quick-action placement with registered contributions over time.
- Register navigation, page, settings-section, and instance-action contributions. Navigation uses `targetPageId` and must resolve by the canonical page contribution ID.
- Keep shared settings draft/persistence ownership and selected-instance context explicit.
- Preserve existing page behavior and any current URL/section semantics during the transition; page IDs and optional URL paths are not interchangeable identities. Phase 3 renders the statically registered, compiled feature set and does not depend on Rust lifecycle state or host IPC that is unavailable until Phase 4.

Acceptance criteria:

- The runtime registry rejects any duplicate contribution ID across all frontend slots, unknown `targetPageId` references, invalid page targets, and invalid ordering before rendering.
- Shell rendering is registry-driven; adding a feature page does not require editing a hard-coded navigation array or a page conditional chain.
- Contribution order is deterministic by `order` then stable ID.
- A feature contribution may import only its own subtree plus frontend core/shared contracts; it must not import another feature module, including another feature public entry.
- No remote UI, arbitrary dynamic import, `eval`, or untrusted HTML becomes part of the registry.
- Dynamic imports are limited to one literal, relative, bundler-resolvable specifier; contribution loaders are component/lazy-function references, never runtime string paths or URLs.
- The TypeScript-AST phase of `scripts/check-architecture.mjs` enforces frontend import boundaries and the dynamic-import rule, and passes in CI before Phase 3 acceptance.
- Tauri event listeners and API calls are typed, cleaned up on unmount, and routed through shared clients.
- Existing navigation, settings, accounts, downloads, saves, launch, and instance actions remain behaviorally compatible.
- Host active-feature availability filtering is implemented in the Phase 4 milestone and was never a Phase 3 dependency; Phase 3 acceptance verifies only the static compiled contribution registry and composition root.

### Phase 4: Workspace and module extraction (Phase 4 checker gate implemented; provider runtime lifecycle hardening and Phase 5 pending)

Status: **Phase 4 checker gate implemented: shared core policy/ports, typed mod/loader provider leaves, typed host loader adoption, and host lifecycle/availability milestone are implemented; provider runtime lifecycle hardening and Phase 5 remain pending.** `src-tauri/Cargo.toml` is the workspace root with exactly the host package, `flint-core`, `flint-feature-api`, `flint-provider-api`, seven `src-tauri/modules/flint-feature-*` crates, two `providers/flint-mod-provider-*` crates, and six `providers/flint-loader-provider-*` crates. `flint-core` owns `ApiVersion`/`ProviderRequirement`, shared policy modules, and object-safe typed port contracts; `flint-feature-api` owns the feature lifecycle contract/registry/coordinator and availability DTO; `flint-provider-api` owns the generic registry and typed mod/loader contracts. The mod/loader provider crates own source-specific HTTP/payload mapping through core ports. `provider_composition.rs` injects one immutable mod registry and one immutable loader registry; `src-tauri/modules/flint-feature-loaders/src/lib.rs` owns the sole `LoaderKind` binding and typed orchestrator, preserving 75/600/600, progress/event/DTO behavior, vanilla preparation, and profile verification; the host `src-tauri/src/loaders.rs` adapter owns only Tauri sink/event adaptation. `feature_host.rs` explicitly registers seven features and real provider metadata, starts the coordinator in Tauri setup, owns managed runtime/snapshot state, publishes `get_host_availability`, gates all 28 feature commands, and shuts down on `RunEvent::Exit`. All seven feature crates are extracted; full provider runtime lifecycle hardening and Phase 5 remain pending; the Phase 4 Cargo-metadata/source/typed-port gate is active.

Scope:

- Turn `src-tauri/Cargo.toml` into the workspace root while retaining the host package there.
- Create `flint-core`, feature/provider API crates, feature modules, and provider implementation crates.
- Move DTOs to their owning contracts and map at the Tauri adapter boundary.
- Introduce the explicit Rust host feature composition root and lifecycle/availability integration (implemented milestone); moving feature business orchestration into extracted feature crates remains pending.
- Keep `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`, `src-tauri/build.rs`, the target directory, and executable naming stable.
- Ensure the Tauri command list remains compile-time and thin.
- Extract the versioned feature/provider API contracts, typed feature/provider contexts, immutable registries, and lifecycle coordinator without changing public enum/DTO contracts.
- Expand `scripts/check-architecture.mjs` to the extracted-crate allowlist and full TypeScript import graph under CI.

Acceptance criteria:

- `cargo metadata` dependency edges satisfy the layer allowlist: no core-to-host, module-to-host, module-to-private-module, provider-to-host, provider-to-module, provider-to-provider, or Tauri dependency leaks; no allowlisted cycle exists; no broad directory/package wildcard hides an edge.
- The workspace has one lockfile at `src-tauri/Cargo.lock` and no member-local lockfiles.
- Release/version tooling still finds exactly the expected host package metadata and the same executable paths.
- `src-tauri/src/lib.rs` is a thin host entry: composition, command registration, and startup, with no provider-specific or feature business logic.
- Host command registration has exactly 30 compile-time `generate_handler!` entries: `launcher_info` and `get_host_availability` are ungated, and the remaining 28 commands are gated by the reviewed owning-feature map before I/O. No runtime command discovery exists.
- Feature initialization, shutdown, duplicate rejection, feature-dependency resolution/cycle detection, deterministic topo order, independent leaf provider initialization in provider-ID order, and full-requirement selection have observable tests or CI checks. Provider-to-provider dependencies/cycles do not exist.
- Startup and runtime lifecycle transitions have observable tests or CI checks: failure propagates to the failed feature plus all transitive dependents; an affected set containing any `Required` feature takes the fatal path (`StartupFailed` at startup, `RuntimeFailed` -> `Stopping` -> `Stopped`/exit at runtime); `Degraded` is valid only while every `Required` feature remains `Ready`; no `Required` feature remains `Ready` with a non-ready dependency/provider. Startup-fatal diagnostic cleanup, unavailable commands, atomic init cleanup, reverse shutdown including initialized provider leaves/core, and Tauri `RunEvent::Exit` integration are covered.
- Source/import checks reject module/provider bypass of network, filesystem, process, settings, and logging ports. The TypeScript AST checker rejects every cross-feature frontend import (public or private), and the full architecture-checker allowlist passes before Phase 4 acceptance.
- Frontend feature folders can be changed without editing the shell except at the composition root.
- Phase 4 acceptance includes the read-only active-feature snapshot and shell filtering: contributions whose owning feature is not `Ready` are not mounted, failed/blocked contributions remain hidden, and unavailable-feature diagnostics are shown without fallback. The implemented milestone satisfies the snapshot/IPC/filtering and 28-command gating behavior; full provider runtime lifecycle hardening and Phase 5 remain pending.
- The contract documents, Rust DTOs, TypeScript DTOs, adapters, and client wrappers remain synchronized.
- The full release/build path assumptions are re-validated by CI before the phase is considered complete.

### Phase 5: Hardening and documentation closeout (Pending)

Status: **Pending.** Transitional checker waivers, remaining migration shims, and final hardening are still open.

Scope:

- Remove temporary compatibility shims, unused exports, and in-place pilot structures that duplicate extracted contracts.
- Tighten tests for failure/shutdown paths, provider registry edge cases, stale UI requests, path traversal, archive limits, and redaction.
- Update architecture status, Architecture Decision Record implementation state, standards, and the documentation index.

Acceptance criteria:

- No target contract exists only in a migration comment.
- No duplicate old/new dispatch path remains.
- Review confirms minimal public surfaces and no hidden cross-feature imports.
- All temporary architecture-checker waivers and compatibility escapes are removed; the strict phase-current allowlist passes in CI.
- The finalized public contract update rule is reflected in standards: update the affected authoritative contract for any public behavior change, and both RPC contract files only for a shared model/command surface.
- CI is the recorded verification authority; local reviewer commands are not used as evidence.

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

These are the end-state acceptance gates. The current baseline satisfies the implemented Phase 1/2 provider milestones, the Phase 3 frontend contribution registry milestone, the Phase 4 workspace/core/feature-API/provider-API foundation, and the host feature lifecycle/availability IPC/gating/filtering milestone; provider runtime lifecycle hardening and Phase 5 remain pending; the Phase 4 checker gate is implemented. A change is architecture-compliant only when all applicable statements for its current phase hold:

- The current implementation is described as implemented, and every unimplemented target path is labeled planned/target/deferred.
- Applicable phase rules from the transition matrix in `DEVELOPMENT_STANDARDS.md` are satisfied; at full extraction, dependencies point inward/downward according to the layer table with no core-to-host, module-to-host, provider-to-host, or private-module edge.
- Cargo manifests and TypeScript imports are covered by the active Phase 4 checker in the release workflow `prepare` job at `--phase 4`. It enforces frontend manifest/composition/import boundaries, module/provider import and selection boundaries, forbidden std/process/fs/net/reqwest/Tauri patterns in all extracted provider leaves, exact mod/loader provider dependency/source/composition checks, exact six loader leaves/descriptors/requirements, both immutable registries, the sole `LoaderKind` binding in the loader feature crate, and exact allowlists. Legacy loader waivers are rejected. The Phase 4 Cargo-metadata/source/typed-port gate is active; Phase 5 remains future work. Cargo and TypeScript alone are not treated as enforcing arbitrary allowlists.
- Feature descriptors use `feature_version`, `api_version`, declared dependencies/provider requirements, startup policy, and declared backend contributions, with exactly one separately registered initialization entry point.
- Startup/runtime failure propagation includes the failed feature plus all transitive dependents. If the affected set contains any `Required` feature, the host is fatal: `StartupFailed` with feature/provider/core cleanup at startup, or `RuntimeFailed` -> `Stopping` -> `Stopped`/exit at runtime. `Degraded` is valid only when every `Required` feature remains `Ready` and the affected set is optional-only. No `Required` feature may remain `Ready` with a non-ready dependency/provider, and there is no feature-level `Degraded`. Unavailable commands, atomic init cleanup, and reverse shutdown are explicit.
- Progress events are best-effort and command results/durable state are authoritative; explicitly required protocol events fail the operation result without rolling back committed state.
- A feature has one stable ID, explicit feature and API versions, declared requirements, one initialization entry point, and declared contributions.
- Registries reject duplicate feature/provider/registration IDs, malformed capability/selection-key bindings, unknown requirements, incompatible API versions, and feature dependency cycles before side effects. Providers are independent leaves with no provider dependency graph.
- Feature order is deterministic topological order with feature-ID tie-breaking; provider leaves initialize in provider-ID order. Failure and shutdown behavior is explicit and tested.
- Provider selection uses active providers, per-capability selection-key bindings, and the full capability/API/provider-version requirement; exactly one match is required, with no fallback or call-site `match` branches after the relevant phase.
- Tauri commands are thin, compile-time registered, and contain no domain or provider logic.
- Frontend contribution IDs are globally unique across all slots, navigation uses canonical `targetPageId` page IDs, and feature modules import only their own subtree plus core/shared. Page, navigation, settings-section, and instance-action slots are populated and mounted in the Phase 3 implementation. The Phase 4 host active-feature availability filter is implemented and fails closed on unavailable/unready features. No arbitrary remote or runtime-string UI code is loaded.
- The affected authoritative contract, Rust/TypeScript DTOs, events, adapters, and documentation are updated together for every public behavior change; both RPC contract files are updated only when a shared model, command, or event surface is affected.
- No names, branding, identifiers, text, or code are copied from another project; references only inform expected behavior and the implementation remains original.
