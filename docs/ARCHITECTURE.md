# Flint Architecture

## Document status

This document is the canonical detailed source for architecture boundaries, provider/registry semantics, lifecycle, frontend contributions, and migration phases. It defines the implemented baseline, the accepted target design, and the migration path between them.

| Label | Meaning |
| --- | --- |
| **Implemented** | Present in the repository at the recorded baseline. |
| **Partially implemented** | A named milestone is present, but the full target or full phase is not complete. |
| **Planned** | Approved target design that has not been implemented yet. |
| **Deferred** | Explicitly outside the current design and requires a separate decision. |

Baseline source commit: `9731c19`. This architecture-documentation update is limited to `ARCHITECTURE.md`, `DEVELOPMENT_STANDARDS.md`, `README.md`, and the Architecture Decision Record; the release-workflow consolidation is a separate change. This document owns the canonical phase-status table below.

### Canonical migration phase status

| Area | Status |
| --- | --- |
| **Rust structure** | A single Cargo package rooted at `src-tauri`. No Cargo workspace or extracted core/API/feature/provider crates exists yet. |
| **Phase 1: ModProvider** | **Partial: pilot milestone implemented; full phase pending.** Generic versioned provider contracts/registry and mod provider adapters/leaves are implemented in `provider_api.rs` and `mods/providers.rs`, and shared mod operations select through the registry. Full typed provider network/path/process/settings/logging/cancellation ports and full checker enforcement remain open for Phase 4/5; provider-local helper waivers also remain. |
| **Phase 2: LoaderProvider** | **Partial: loader registry/leaf milestone implemented; full phase pending.** `loaders/providers.rs` has six independent loader leaves, typed list/install contexts, a progress sink, deterministic selection, and the preserved 75-second metadata/600-second installer timeout split. Full `LoaderProviderContext` port decomposition and full checker enforcement remain open for Phase 4/5. |
| **Phase 3: Frontend page/navigation registry** | **Implemented milestone; Phase 3 not fully complete.** Typed contracts, validation, the frozen registry, and the explicit static composition root are in `src/core/contributions.ts`, `src/app/composition.ts`, and `src/app/`; page and navigation rendering are registry-driven. `settingsSections` and `instanceActions` contracts/validation exist but are intentionally unpopulated; their UIs remain inside page components and those slots are not rendered/migrated yet. |
| **Architecture checker and CI** | **Implemented milestone at Phase 2; full port enforcement pending.** `scripts/check-architecture.mjs` and its tests run in the `flint-release.yml` release workflow `prepare` job after frontend dependency installation: `prepare` runs the checker tests, then runs `node scripts/check-architecture.mjs --phase 2`, and all build-matrix jobs depend on `prepare`. The active checker enforces module/provider import and selection boundaries, explicitly forbidden std/process/fs patterns, and exact allowlists, while intentionally allowing reqwest/shared-client and reviewed parent-helper waivers for provider adapters. It does **not** yet enforce full typed network/path/process/settings ports; full port enforcement is an explicit Phase 4/5 gate. Phase 3+ checker gates are not active yet. |
| **Phase 4: Workspace/module extraction and Rust feature lifecycle** | **Pending.** No Cargo workspace/extracted crates, Rust `FeatureDescriptor`/feature lifecycle registry/Rust composition root, or host active-feature snapshot/filtering exists. |
| **Phase 5: Hardening and documentation closeout** | **Pending.** Temporary transitional waivers and remaining migration work continue. |
| **Public RPC contract** | Existing public commands, DTOs, and events remain unchanged. |

Important current boundary: the application is still a **modular monolith**. Provider contract/registry and loader/provider pilot milestones are implemented inside the current structure, and the frontend page/navigation contribution registry is implemented. Full typed provider port decomposition, full provider lifecycle, the Cargo workspace, extracted API/feature/provider crates, Rust feature lifecycle registry/composition root, and host active-feature availability filtering remain planned. No target path is an implemented path merely because it appears in this document.

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

The launcher spans several independent capabilities: local version discovery, installation, launch, accounts, loader installation, mod discovery and management, saves, and settings. They still share one Rust package, while the frontend now has a page/navigation contribution registry. Provider and frontend registries are implemented inside the current structure; crate extraction, the Rust feature lifecycle, and complete slot migration remain target work.

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

The Rust backend is still a single Tauri v2 package rooted at `src-tauri`:

- `src-tauri/Cargo.toml` remains the one package manifest; `src-tauri/Cargo.lock` is its lockfile.
- `src-tauri/src/main.rs` is a thin desktop entry point that calls `flint_lib::run()`.
- `src-tauri/src/lib.rs` declares every Rust module, contains the 29 `#[tauri::command]` wrappers, and registers them with the compile-time `tauri::generate_handler!` macro.
- `src-tauri/src/models.rs` contains the serializable Rust DTOs. Existing public command/DTO/event contracts remain unchanged.
- Blocking network, filesystem, process, and archive work is generally moved off Tauri async commands with `tokio::task::spawn_blocking`.
- Public command failures are currently returned as display-ready `String` errors. `error.rs` maps background-task join failures.

The current Rust module responsibilities are:

| Module | Implemented responsibility |
| --- | --- |
| `provider_api.rs` | Generic versioned provider descriptors, capability bindings, provider requirements, compatibility checks, immutable deterministic provider registry, and unavailable/ambiguous selection errors. |
| `mods/providers.rs` | `ModProvider` trait and operation-scoped `ModProviderContext`; mod-source provider leaves; centralized `ModSource`-to-selection-key binding; mod provider registry construction and selection. |
| `loaders/providers.rs` | `LoaderProvider` trait; typed list/install contexts; progress sink; six independent loader leaves; centralized `LoaderKind`-to-selection-key binding; loader provider registry construction and selection. |
| `auth.rs` | Local account store, interactive account flow, token refresh, account selection/removal, launch identity resolution, and sensitive-value redaction. |
| `install.rs` | Remote game-version listing, verified downloads, asset and library installation, progress emission, and install result assembly. |
| `java.rs` | Java executable discovery, inspection, path validation, version parsing, and architecture parsing. |
| `launch.rs` | Local version resolution for launch, argument construction, native extraction, process start, command display, and launch-log path creation. |
| `loaders.rs` | Shared loader orchestration plus provider-specific helper implementations still resident in this module: validation, vanilla preparation, provider selection, install helpers, client/timeout construction, progress adaptation, and result assembly. |
| `mods.rs` | Shared mod orchestration plus provider-specific HTTP/payload helper implementations still resident in this module: validation, dependency planning, archive inspection, sidecar persistence, installed-mod management, provider selection, and progress adaptation. |
| `net.rs` | Shared blocking HTTP client creation, HTTPS validation, bounded response reads, atomic downloads, temporary paths, and directory creation. |
| `paths.rs` | Application and game directory resolution, settings path, path normalization, safe joining, deduplication keys, and SHA-1 helpers. |
| `saves.rs` | Save discovery, bounded NBT parsing, ZIP import/export/backup, rename, and delete behavior. |
| `settings.rs` | Settings validation, default creation, atomic JSON persistence, and Unix owner-only permissions. |
| `version.rs` | Local instance scanning, version inheritance resolution, platform/rules evaluation, library selection, and version metadata validation. |
| `models.rs` | Serializable request, response, progress, and enum DTOs shared across current modules. |
| `error.rs` | Conversion of background-task failures to public error strings. |

The Phase 1 and Phase 2 provider registry/selection milestones are implemented inside this single package, while full typed-port and checker phases remain partial:

- `src-tauri/src/provider_api.rs` provides the shared `Provider` contract, `ApiVersion`, `ProviderCapability`, `ProviderDescriptor`, `ProviderRequirement`, and `ProviderRegistry`.
- The registry is immutable after construction, validates provider/capability/version/API/selection-key constraints, rejects duplicate provider/capability IDs and duplicate capability/selection-key bindings, sorts providers by stable ID, and returns explicit unavailable or ambiguous errors from selection.
- `src-tauri/src/mods/providers.rs` contains the implemented mod-provider layer. Shared mod operations build the registry and select by the central `ModSource` binding instead of branching on source at each call site.
- `src-tauri/src/loaders/providers.rs` contains six independent loader provider leaves with typed list/install contexts, a typed progress sink, and one central `LoaderKind` binding. `loaders.rs` constructs separate shared metadata and installer clients to preserve the 75-second metadata and 600-second installer timeout split.
- The provider implementations do not depend on one another. Current provider entries are active when successfully constructed in the immutable per-operation registry; the full Phase 4 provider/feature lifecycle and availability state are still pending.
- Provider adapter modules still use reviewed transitional local helper imports from the current `mods.rs`/`loaders.rs`; extracting those helpers behind the full provider/core ports and expiring the checker waivers belong to later phases.

The remaining Rust boundaries are still logical module boundaries, not crates. The current tree has **no** Rust `FeatureDescriptor`/feature lifecycle registry, Rust composition root, active-feature snapshot, Cargo workspace, or extracted `flint-core`/feature/provider API crates. Those remain Phase 4 work. Existing public commands, DTOs, and events are unchanged by Phases 1-3.

### Frontend shape

The frontend is a React 18 and TypeScript application rooted at `src`. Phase 3 page/navigation modularization is implemented without changing the public RPC contract:

- `src/main.tsx` mounts `src/app/App.tsx`.
- `src/core/contributions.ts` defines typed `FeatureDescriptor`, page, navigation, settings-section, and instance-action contracts; validates stable IDs, feature requirements/cycles, page targets, duplicate contribution IDs across slots, ordering, and component/handler references; then builds and deeply freezes a deterministic contribution registry.
- `src/app/composition.ts` is the explicit static frontend composition root. It imports the approved feature modules and exports the frozen `frontendRegistry`; there is no glob auto-discovery or remote contribution manifest.
- `src/app/AppSidebar.tsx` renders navigation contributions and resolves the active page by canonical `targetPageId`/page contribution ID.
- `src/app/App.tsx` owns shared app-level state and renders the active page contribution through `ContributionBoundary`, a typed error boundary. Page modules receive `AppPageContext` from `src/core/app-context.ts`.
- `src/modules/` contains the current feature page modules: launch, versions/downloads, saves, settings, about, and accounts. The first five expose pages/navigation; accounts exposes the account-dialog surface required by the shell.
- Existing settings fields/sections and quick actions still live inside their page components. All current `settingsSections` and `instanceActions` contribution arrays are intentionally empty, and the shell does not yet mount those slots. Their contracts and validation exist, but those UIs are not migrated or active contributions.
- `src/api.ts`, `src/types.ts`, `src/utils.ts`, and the remaining generic components still live at `src` and have not completed the target `core`/`shared` extraction.
- Some current modules and the shell still use reviewed transitional direct Tauri event/dialog/opener imports. Those are checker waivers expiring in Phase 5, so full typed adapter extraction is not complete.
- The frontend has no router or host active-feature availability filtering. Phase 3 renders the statically registered compiled feature set; Phase 4 planning will add the Rust lifecycle/availability snapshot and filtering.
- The implemented Phase 3 checker logic is static. The `flint-release.yml` release workflow `prepare` job currently installs frontend dependencies, runs the architecture checker tests, then runs the checker at `--phase 2`; all build-matrix jobs depend on `prepare`. Phase 3+ checks are not active in that gate yet.

Phase 3 therefore has a completed **page/navigation registry milestone**, not a completed full Phase 3 migration. Settings-section and instance-action contracts/validation are implemented, but population, shell rendering, and migration of existing UI remain pending.

### Build and release constraints (Implemented)

The current build and release automation assumes these paths and identities:

- The Cargo package root and Tauri application root are `src-tauri`.
- The Cargo lockfile is `src-tauri/Cargo.lock`.
- The Tauri configuration is `src-tauri/tauri.conf.json`.
- The frontend build output is repository-root `dist`, referenced by `frontendDist: "../dist"` in the Tauri configuration.
- The default Cargo target directory is `src-tauri/target`; the `flint-release.yml` release workflow reads executables from `src-tauri/target/<rust-target>/release/flint` or `flint.exe`.
- Release/version tooling inspects the package versions in `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.lock`, plus the root package metadata. The lockfile validator expects exactly one package named `flint`.
- `build.rs` performs Tauri code generation and adds the Windows GNU cdylib link argument.
- The library crate is named `flint_lib` and keeps staticlib, cdylib, and rlib crate types.
Current architecture-checker and CI facts:

- `scripts/check-architecture.mjs` and `scripts/test-architecture-checker.mjs` are implemented.
- The `prepare` job in the `flint-release.yml` release workflow installs frontend dependencies, runs `node --test scripts/test-architecture-checker.mjs`, and then runs `node scripts/check-architecture.mjs --phase 2`.
- All build-matrix jobs depend on `prepare`, so the checker tests and phase-2 gate must pass before any build starts.
- The Phase 2 checker performs static Rust source/module checks and TypeScript AST checks without invoking Cargo. Its active Rust scope covers module/provider import and selection boundaries, forbidden std/process/fs patterns, and exact allowlists, while intentionally allowing reqwest/shared-client and reviewed parent-helper waivers. It does not enforce full typed provider ports; full port enforcement is deferred to Phase 4/5, and Phase 4 workspace metadata checks require the not-yet-created Cargo workspace.

Consequences for migration:

1. Keep `src-tauri/Cargo.toml` as the workspace root when the workspace is introduced, unless the release tooling and documented paths are changed in the same scoped change.
2. Keep the host package `flint` and its `src-tauri/src` location. A workspace root can also be a package root; member crates can live under `src-tauri/crates`, `src-tauri/modules`, and `src-tauri/providers`.
3. Keep one lockfile at `src-tauri/Cargo.lock` and avoid member-local lockfiles.
4. Keep the default target directory and executable names stable until the release workflow and validation scripts have been deliberately updated.
5. Keep `main.rs` as the executable entry and `flint_lib::run()` as the library entry unless a release-path migration is explicitly approved.
6. Do not move or redefine the Tauri configuration or frontend output path during the architecture migration.

## Target architecture (Partially implemented; extraction/lifecycle pending)
The target layer model remains normative. The current tree has an implemented generic provider API/registry and a page/navigation frontend contribution registry inside the single Rust package and current frontend folders. The extracted crate layout, Rust feature lifecycle/composition root, full provider runtime lifecycle, and active-feature availability filtering are still target-only. The table below describes required end-state boundaries, not the current crate layout.

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
| `flint-core` | Shared domain policy, path/network/process/error primitives, and stable utilities that are independent of any one feature. | Standard library and vetted general-purpose crates. | Tauri host, frontend code, feature modules, provider implementations, feature descriptors, or product-specific branches. |
| Feature API crate(s) | Feature IDs, versions, descriptors, registry contracts, lifecycle types, and stable ports required by features. | `flint-core` only where a shared primitive is necessary. | Host, concrete modules, concrete providers, Tauri. |
| Provider API crate(s) | `ModProvider`, `LoaderProvider`, provider descriptors, request/context/result/error contracts, and progress/capability ports. | `flint-core` only where a shared primitive is necessary. | Host, concrete modules, concrete provider implementations, Tauri. |
| Feature modules | User-facing domain orchestration, feature-owned state and services, contribution declarations, and feature initialization. | Core, feature API, provider API, stable external libraries, and explicitly published cross-feature APIs. | Host, Tauri, another module's private files, provider implementations, React, or dynamic global state. |
| Provider implementations | Source- or loader-specific external I/O, payload mapping, and provider-specific planning/execution behind a provider API. | Provider API, core network/path/process policy, and provider-specific leaf libraries. | Host, Tauri, feature module internals, any other provider, provider-to-provider dependencies, or UI code. |
| Tauri host/adapters | App startup, command registration, DTO conversion, Tauri event/process/dialog adaptation, dependency injection, and the Rust composition root. | All approved core, API, module, and provider surfaces, plus Tauri. | Nothing is allowed to depend back on the host. |
| Frontend `core`/`shared` | API/event transports, registry primitives, generic UI, hooks, error presentation, and stable cross-feature types. | Framework/library code and its own public contracts. | Concrete feature modules or `app` code. |
| Frontend feature modules | UI for one capability, its local state, typed contribution registrations, and calls through the shared API client. | Frontend `core`, `shared`, and its own subtree/public surface. | Any other frontend feature module (including its public entry), remote code, direct `invoke` calls that bypass the API layer, or framework adapters from a feature's domain logic. |
| Frontend `app` | The single composition root, root providers, shell wiring, and explicit module list. | Every approved frontend module and core/shared surface. | Business logic that belongs to a feature. |

### Recommended workspace layout (Planned)

The following is a target layout, not a statement that these paths exist. Names may be adjusted during extraction, but the ownership and dependency rules must remain.

```text
src-tauri/
  Cargo.toml                      # existing path; host package plus workspace root
  Cargo.lock                      # existing path; one workspace lockfile
  tauri.conf.json                 # existing path; unchanged during extraction
  build.rs                        # existing path; host build script
  src/                            # existing host location; remains thin
    main.rs
    lib.rs
    composition/                  # explicit feature/provider assembly
    commands/                     # thin Tauri command adapters by feature
    adapters/                     # DTO, event, dialog, process, and framework adapters
  crates/
    flint-core/                   # shared core policy and ports
    flint-feature-api/            # feature descriptor, registry, lifecycle contracts
    flint-provider-api/           # ModProvider and LoaderProvider contracts
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
10. Cargo manifests and TypeScript imports define actual edges, but neither Cargo nor TypeScript inherently enforces an arbitrary layer allowlist. The implemented Phase 2 milestone checker (current release workflow `prepare` gate) enforces module/provider import and selection boundaries, forbidden std/process/fs patterns, and exact allowlists, with reqwest/shared-client and reviewed parent-helper waivers intentional. Full typed-port enforcement is deferred to Phase 4/5, and later-phase checker policies become binding when the gate advances.

Frontend:

1. `src/app` is the only layer allowed to import concrete feature modules for assembly. `src/core` and `src/shared` cannot import `src/app` or concrete feature modules.
2. A frontend feature module may import only frontend core/shared APIs and its own files. It cannot import another feature module at all, including that feature public entry, and cannot reach into another feature store.
3. Cross-feature UI composition, state, actions, and navigation are performed through typed core contracts, shell services/contexts supplied by the app composition root, or contribution slots. No feature imports another feature module as a cross-feature API.
4. Domain/API behavior belongs in feature services or the shared API client, not in generic shared components.
5. The composition root creates and freezes the frontend registry before rendering. Components read the registry and typed shell services; they do not mutate global registration state at render time.

### Architecture boundary checker (Phase 2 milestone implemented; full port enforcement pending)

The checker is implemented at `scripts/check-architecture.mjs`, with automated coverage in `scripts/test-architecture-checker.mjs`. It is phase-aware, fail-closed, and reads checked-in reviewed allowlists. A broad directory or package wildcard is not an allowlist. Temporary waivers name the exact rule, file/package, phase, owner, and removal condition and fail closed when the phase ends.

Current integration status:

- The release workflow `prepare` job installs frontend dependencies, runs the checker test suite, then runs `node scripts/check-architecture.mjs --phase 2`.
- All build-matrix jobs depend on `prepare`; the architecture tests and phase-2 checker are the single release-prepare gate.
- The implemented Phase 1/2 milestone boundaries are checked by the release workflow `prepare` gate. The script contains phase-aware policy for later phases, but Phase 3+ checks are not active in this gate yet; Phase 4 workspace metadata and full typed-port enforcement must wait for the Cargo workspace/extracted-crate and port work.

#### Rust checks

- The active Phase 2 path performs static Rust source/module analysis; it does not invoke Cargo. It masks comments/strings, scopes test-only modules correctly, enforces reviewed module/provider import and selection boundaries, rejects the explicitly forbidden std/process/fs patterns, and uses exact allowlists. It intentionally allows reqwest/shared-client use and reviewed parent-helper waivers for provider adapters. It does **not** enforce full typed network/path/process/settings/logging/cancellation ports; that is an explicit Phase 4/5 acceptance gate.
- For Phase 4 and later, the checker contains Cargo metadata logic for the planned workspace: `cargo metadata --manifest-path src-tauri/Cargo.toml --no-deps --format-version 1` parses workspace packages and dependency edges against reviewed crate allowlists. This logic is not an active gate while Rust remains one package and the workspace is pending.
- The target crate rules remain mandatory when Phase 4 begins: reject host/Tauri dependencies from core, API, feature, and provider crates; reject cycles; reject provider-to-module, provider-to-provider, and module-to-private-module edges; allow only explicit cross-feature public Rust API edges. Frontend feature imports are stricter and never cross feature boundaries.
- The target source rules remain mandatory as ports and extracted crates appear: reject forbidden direct I/O, network, process, and framework imports in feature/provider implementation code once the corresponding ports exist. Core port implementations and host adapters are the only approved homes for underlying I/O/process/framework calls.
- Manifest checks reject dependencies that would bypass approved ports; source checks catch standard-library or macro paths that Cargo metadata cannot see. The checker is not a security sandbox: review must reject obfuscation, FFI, or unsafe escape hatches intended to avoid the allowlist, and unsafe FFI is prohibited absent a separate accepted decision.

#### TypeScript checks

- The checker uses the installed TypeScript compiler API to parse every `.ts`/`.tsx` file and collect static imports, re-exports, side-effect imports, `import type`, `require` forms if any, and dynamic `import()` calls.
- The implemented target frontend rules are strict: `src/app` is the only assembly layer allowed to import all concrete feature modules; `src/core` and `src/shared` cannot import `src/app` or `src/modules`; a feature module may import only its own subtree plus frontend core/shared APIs. No feature may import another feature module at all, including that feature public entry. Cross-feature composition/state/actions use typed core contracts, shell services/contexts, or contribution slots.
- Framework imports are restricted to approved transport/adapter layers. Feature components should call typed API/event clients rather than importing Tauri packages directly; reviewed temporary Phase 5 waivers currently remain in the checker.
- External packages are part of the allowlist. Adding a new package to a feature requires dependency review, not merely an import that happens to compile.
- Dynamic imports are allowed only with exactly one string-literal, relative, bundler-resolvable specifier inside the importing feature's own subtree or an explicitly allowlisted local chunk. Reject variable/template-computed specifiers, absolute paths, file/HTTP URLs, package specifiers for contribution loading, `eval`, and `new Function`.
- A contribution loader is a component reference or a locally declared lazy function. It must never be a string path that the registry evaluates at runtime. The checker and bundler can prove a literal import target; the registry cannot runtime-prove an arbitrary filesystem or URL string.
- The checker contains Phase 3 static manifest/import/registry-shape checks, but the current release `prepare` gate remains `--phase 2`. Advancing the gate is part of finishing Phase 3/4.

#### CI gate

- CI failure blocks completion; there is no warn-only mode.
- The current gate is the release workflow `prepare` job at Phase 2. The implemented checker milestone covers module/provider import and selection boundaries, forbidden std/process/fs patterns, and exact allowlists, while intentionally allowing reqwest/shared-client and reviewed parent-helper waivers. It does not establish full Phase 1/2 typed-port acceptance. Full network/path/process/settings/logging/cancellation port enforcement remains Phase 4/5 work.
- Advancing the gate to Phase 3 requires making the implemented frontend contribution/import checks the active gate, resolving/expiring transitional Tauri import waivers only when appropriate, and meeting full Phase 3 acceptance. Advancing to Phase 4 requires the Cargo workspace and activated metadata gate.
- Any allowlist change is an architecture change and must be reviewed as such.

## Rust feature contract and lifecycle registry (Planned)
Status: **Pending Phase 4.** The current tree has provider registries and a frontend contribution `FeatureDescriptor`, but it does not have the Rust `FeatureDescriptor`, feature dependency/lifecycle registry, Rust composition root, or active-feature snapshot described in this section. Event/error/lifecycle rules below remain the target specification.

A feature is registered by declaration and has one initialization entry point. A planned Rust-shaped contract looks like the following; names may change during implementation, but the semantics must not be weakened:

```rust
pub struct FeatureDescriptor {
    pub id: FeatureId,
    pub feature_version: Version,
    pub api_version: ApiVersion,
    pub startup_policy: StartupPolicy,
    pub requires: Vec<FeatureRequirement>,
    pub provider_requirements: Vec<ProviderRequirement>,
    pub init: FeatureInit,
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

The feature may return a runtime handle after initialization with lifecycle methods such as `health` and `shutdown`. Initialization must be atomic from the registry's perspective: if it returns an error after acquiring resources or starting workers, the feature itself must release those partial resources before returning the error. The registry cannot infer a failed feature's partial state. The initial implementation should keep the lifecycle small and explicit; do not invent a general-purpose dependency-injection framework.

### Feature ID and version rules

- IDs are compared exactly after validation; no case-insensitive aliases, display-name matching, or auto-generated IDs.
- A renamed capability keeps the old ID as a documented alias only during a migration and must update every contribution and contract reference in the same change. Prefer adding a new versioned capability over silently changing semantics.
- `feature_version` describes the feature implementation; `api_version` describes the feature contract boundary; `provider_version` describes a provider implementation. Do not overload one field for another.
- Provider IDs follow the same stable-ID discipline and are distinct from feature IDs.
- Backend registration IDs are globally unique across the backend registry and prefixed with the owning feature ID. Frontend contribution IDs are a separate namespace and use the single global-across-all-slots rule in the frontend section.

## Rust feature registration and lifecycle (Planned)
Status: **Pending Phase 4.** Provider registry construction and deterministic selection exist, but the Rust feature dependency graph, feature initialization lifecycle, active-feature availability snapshot, and application-level lifecycle coordinator do not. These rules remain target requirements.

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
- Phase 3 does not depend on Rust lifecycle state or availability IPC; it renders the statically registered compiled feature set. Starting in Phase 4, the frontend receives the read-only availability snapshot. If that snapshot is exposed over Tauri IPC, it must be added to the authoritative contract rather than introduced as an undocumented command. The shell then filters navigation, pages, settings sections, and instance actions by the owning feature active state.

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

## Provider contracts (Phase 1/2 milestones implemented; full ports/extraction/lifecycle pending)

The provider layer separates reusable feature behavior from provider-specific external I/O. The Phase 1/2 registry/selection milestones are implemented in the current single Rust package: `provider_api.rs`, `mods/providers.rs`, and `loaders/providers.rs`. Existing serialized `ModSource`, `LoaderKind`, and DTO contracts remain unchanged. Full typed provider contexts/ports, extracted provider API crates, and the full runtime provider lifecycle remain Phase 4/5 work.

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
Current implementation status: the provider registry is immutable per operation and every successfully constructed entry is active for selection. There is no separate mutable provider health/lifecycle registry yet; that active-state integration belongs to the pending Phase 4 Rust lifecycle work.

### `ModProvider` (Phase 1 pilot milestone implemented; full ports pending)

`ModProvider` owns source-specific catalog and version metadata behavior while the shared mod feature owns common dependency planning, downloads, archive inspection, and filesystem management.

Implemented method shape:

```text
ModProvider
  search(ctx, query, game_version, loader, offset, limit) -> ModSearchPage
  list_versions(ctx, project_id, game_version, loader) -> [ModVersionInfo]
  version_by_id(ctx, project_id, version_id, game_version, loader) -> ModVersionInfo
  dependency_version(ctx, dependency, game_version, loader) -> Option<ModVersionInfo>
```

Current implementation:

- `src-tauri/src/mods/providers.rs` defines the `ModProvider` trait, two concrete source providers, the `ModProviderContext`, provider descriptors, and selection bindings.
- The current `ModProviderContext` is operation-scoped and exposes only a shared blocking HTTP client and an optional provider credential/API key. It contains no Tauri handle or settings-file path. Shared mod code controls validation, dependency graph limits, downloads, archive checks, sidecar persistence, installed-mod management, and progress publication.
- The provider registry is built in the shared mod feature for each operation; provider methods return public DTOs or provider data to that feature rather than performing final game-directory writes.

Target Phase 4/5 context expectations remain:

- replace feature-local HTTP/credential details with approved core network, safe instance/path, scoped settings/credential, logging, and cancellation/deadline ports;
- keep final mod placement and sidecar writes in the shared mod feature;
- forbid direct Tauri handles, raw windows, unrestricted filesystem roots, unrestricted process runners, and direct settings-file paths.

### `LoaderProvider` (Phase 2 registry/leaf milestone implemented; full ports pending)

`LoaderProvider` owns loader-specific discovery and installation implementation while the shared loader feature owns validation, vanilla preparation, shared clients/safety policy, progress adaptation, and result assembly.

Implemented method shape:

```text
LoaderProvider
  list_versions(ctx: LoaderListContext) -> [LoaderVersionInfo]
  install(ctx: LoaderProviderContext) -> LoaderInstallOutput
```

Current implementation:

- `src-tauri/src/loaders/providers.rs` defines `LoaderProvider`, `LoaderListContext`, `LoaderProviderContext`, a typed `LoaderProgressSink`, six independent loader leaves, and centralized `LoaderKind` selection bindings.
- The six leaves are the implemented loader providers. They do not depend on one another; registry construction is deterministic and shared loader code performs one registry selection.
- `LoaderListContext` carries only the metadata client, requested game version, and progress sink. `LoaderProviderContext` carries separate metadata and installer clients, the resolved game root, the install request, and the progress sink.
- `src-tauri/src/loaders.rs` retains shared orchestration: request validation, vanilla baseline preparation, separate 75-second metadata and 600-second installer clients, provider selection, progress adaptation, version validation, and final `LoaderInstallResult` assembly. Provider methods do not receive `AppHandle` or emit Tauri events directly.
- The current contexts are typed but are not yet the full Phase 4/5 decomposition into core network, safe archive/path, Java-runner, settings, logging, and deadline/cancellation ports.

Target Phase 4/5 context expectations remain:

- approved core network/download and safe archive/path ports;
- a bounded Java process runner with captured/redacted output;
- shared vanilla baseline installation;
- progress/logging sinks and deadline/cancellation;
- read-only settings needed by the operation.

Loader providers may run only trusted installer flows, with bounded processes, captured/redacted output, timeouts, and safe archive policy. A mod provider must never execute a downloaded mod archive.

### Enforcing shared ports

Phase 2 checker status:

- The implemented Phase 2 checker enforces reviewed module/provider import and selection boundaries, explicitly forbidden std/process/fs patterns, and exact allowlists through static Rust source analysis. It intentionally allows reqwest/shared-client use and reviewed parent-helper waivers for provider adapters. It does not invoke Cargo and does not enforce full typed ports.
- The current typed provider contexts already hide Tauri handles, settings-file paths, and direct final destination selection from providers, but full network/path/process/settings/logging/cancellation port decomposition is not complete.
- The current checker contains transitional Tauri import waivers that expire at a later phase; the current active gate remains Phase 2.

Target Phase 4/5 enforcement rules remain:

- Extracted provider and feature-module Cargo manifests may depend only on reviewed phase allowlist entries. Direct Tauri, HTTP, process, and filesystem implementation dependencies are rejected where the layer must use ports.
- The source checker rejects restricted standard-library paths and calls such as `std::fs`, `std::process`, `std::net`, `Command::new`, and direct process spawning in module/provider implementation code once the corresponding ports exist.
- Only core implementations and host adapters may implement the network, path, Java-runner, settings, logging, and progress ports.
- Provider API crates expose context/port traits, not a handle to a global adapter, so a provider cannot obtain unrestricted file/process/network access through the contract.
- The checker is not a sandbox. Review rejects obfuscation, unsafe FFI, macro tricks, or new manifest paths intended to evade the allowlist; any temporary exception is explicit, scoped, and phase-expiring.

The checker is implemented for its Phase 2 milestone. Full typed network/path/process/settings/logging/cancellation port enforcement is an explicit Phase 4/5 acceptance gate, alongside Phase 4 Cargo metadata checks. See **Architecture boundary checker (Phase 2 milestone implemented; full port enforcement pending)** above for current and target responsibilities.

### Enum and DTO compatibility during migration
The Phase 1/2 registry/selection milestones are implemented without changing public commands, DTOs, enum serialization, or events. Provider selection uses centralized source/loader bindings in the provider adapters; provider-internal IDs/versions remain internal. Full typed-port and checker acceptance remains Phase 4/5 work.

- `ModSource` and `LoaderKind` remain serialized enum/DTO values while the frontend contract depends on them.
- The active provider adapter translates the deserialized `ModSource`/`LoaderKind` enum to a provider selection key in one place. Shared feature services use registry lookup, not provider-specific branching.
- Unknown enum values fail during deserialization as they do today.
- Adding or removing an enum variant is a public contract change: update Rust and TypeScript DTOs and the affected authoritative contract together.
- Provider IDs, provider versions, capability versions, and provider payloads are internal. They must not replace or appear inside public DTOs unless a contract change explicitly introduces them.
- Provider-internal response types are normalized and validated into public DTOs before crossing the adapter boundary.
- Provider API/capability versions are separate from the public RPC contract version and from `feature_version`.

### Provider migration order and status

1. **Milestone implemented; full phase pending:** introduced the provider API, capability model, operation contexts, and provider registry inside the current crate.
2. **Milestone implemented; full phase pending:** implemented the ModProvider pilot with explicit provider leaves while keeping command wrappers and DTOs stable.
3. **Milestone implemented; full phase pending:** moved mod-source dispatch to the provider registry and kept shared dependency/install logic in the mod feature.
4. **Milestone implemented; full phase pending:** repeated the migration for LoaderProvider, including typed list/install contexts, six independent leaves, and the preserved metadata/installer timeout split.
5. **Pending Phase 4/5:** extract provider API crates and complete the target typed ports, provider-local helper cleanup, runtime lifecycle, and full checker enforcement.

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

## Frontend architecture and contribution registry (Phase 3 page/navigation implemented; full slots pending)
Status: **Partially implemented.** The typed registry/contracts, frozen validation, explicit static composition root, and registry-driven page/navigation rendering are implemented. `settingsSections` and `instanceActions` contracts/validation are implemented but currently unpopulated and not mounted; existing settings sections and quick actions remain page-local. Host availability filtering, complete core/shared extraction, and full Phase 3 population/renderer migration remain pending.

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

This tree is the target shape, not a complete current tree. The implemented subset is `src/app/` (`App.tsx`, `AppSidebar.tsx`, `ContributionBoundary.tsx`, `composition.ts`), `src/core/` (`contributions.ts`, `app-context.ts`), and `src/modules/` feature page modules. `src/shared/`, the remaining `src/core` transport/event/error/type extraction, and a separate generic `AppShell`/bootstrap/provider layer are still target-only. `src/main.tsx` currently mounts `src/app/App.tsx` directly.

### Contribution slots

The frontend registry accepts typed contributions for at least these slots:

| Slot | Required target semantics | Current implementation status |
| --- | --- | --- |
| **Navigation** | `id`, owning `featureId`, label, icon, `targetPageId`, numeric order, and optional typed visibility/capability selector. `targetPageId` must equal the canonical `id` of a registered page contribution. The shell renders the ordered list; it must not maintain a second hard-coded nav array. | Implemented and rendered. Current modules provide ordered nav contributions and optional indicators. |
| **Page** | `id` (the canonical page contribution ID and navigation target), owning `featureId`, title, component or local lazy-loader, numeric order, and any declared shell inputs. An optional URL path may be presentation metadata, but it is not a second identity or target. A page is mounted only through the shell contribution boundary. | Implemented and rendered through `ContributionBoundary`; current modules use static component references. |
| **Settings section** | `id`, owning `featureId`, title, component or local lazy-loader, order, and a declared settings namespace/validation hook. Sections edit through the shared settings owner; they do not write their own copy of the settings file. | Contract, validation, and frozen registry support are implemented. All current feature manifests provide empty arrays; the shell does not mount this slot; the existing settings sections remain inside the settings page. |
| **Instance action** | `id`, owning `featureId`, label, icon, order, typed enabled/visible selector, and an action handler receiving an `InstanceActionContext` with the selected instance, effective game directory, and approved navigation/API services. Examples include launch, open directory, manage content, and open a launch log. | Contract, validation, and frozen registry support are implemented. All current feature manifests provide empty arrays; quick/instance actions remain page-local and this slot is not mounted/migrated. |

A contribution declaration must be data plus a locally bundled component/function reference only. It must not perform registration side effects such as I/O, network calls, timers, event listeners, or stores during module evaluation. A contribution loader is never a runtime string path: the registry can validate metadata, but only the TypeScript AST checker and bundler can prove that a literal module specifier resolves to bundled code.

### Registry rules
Current Phase 3 implementation validates feature descriptors and all four contribution-slot shapes, enforces globally unique IDs/page targets/ordering/feature requirement cycles, freezes the registry, and renders page/navigation contributions from `frontendRegistry`. The Phase 3 static checker rules are implemented in the checker/test suite but are not yet active in the release `prepare` gate (currently `--phase 2`). The rules below remain the full target contract.

- Every frontend contribution has a globally unique stable ID across every frontend slot, not merely within its slot, and is prefixed by the owning feature ID (for example `flint.mods.page.discovery`).
- Any duplicate contribution ID across any slot is rejected before render; last-registration-wins is forbidden. Navigation targets must resolve to exactly one registered page contribution by its canonical page ID.
- The composition root explicitly imports approved feature modules and passes their contribution lists to the registry builder. Glob-based auto-discovery, remote manifests, and runtime module URLs are forbidden in the initial architecture.
- The registry validates required fields, numeric finite ordering, global ID uniqueness, known `targetPageId` references, owning feature IDs, and local lazy-loader shape. A URL path is never used to resolve a navigation target.
- Ordering is `order` ascending, then stable contribution ID lexicographically. The shell must not depend on insertion order from an object or module loader.
- The registry is frozen before React renders. No feature can add, replace, or remove a contribution while the app is running. A later dynamic plugin proposal must not bypass this rule.
- Unknown page IDs produce a deliberate fallback or diagnostic page. They must not accidentally render another feature.
- The shell owns the selected page contribution ID and rendering boundary. Conditional feature logic belongs in typed selectors or feature-owned components, not in a growing chain of shell checks.
- After Phase 4 introduces the host availability snapshot, the shell filters contributions against it. Contributions owned by a feature that is not `Ready` are not mounted in normal navigation, page, settings, or instance-action slots. A diagnostics surface may list unavailable features and reasons; it must not fetch remote code or substitute another feature. Before Phase 4, Phase 3 renders the statically registered compiled set.

### Composition root

Current implemented sequence:

```text
src/main.tsx
  -> src/app/App.tsx
  -> src/app/composition.ts imports approved feature modules
  -> buildContributionRegistry validates/freezes frontendRegistry
  -> AppSidebar renders navigation and App renders the active page contribution
```

`src/app/composition.ts` is the only current frontend file that imports the complete feature-module list. The shell keeps shared state and passes `AppPageContext` into page components. The page/navigation registry is live; settings-section and instance-action slots are validated but not yet populated or mounted.

Target evolution: install the remaining shared core/event/error/type providers and a fully generic shell, populate/mount settings and instance-action slots, and add the Phase 4 host active-feature availability snapshot/filter. The frontend composition root remains the only place that imports concrete feature modules for assembly.

### State ownership
Current Phase 3 state: `src/app/App.tsx` owns shared shell state and exposes `AppPageContext`; feature page modules own their local UI state. Settings sections remain inside the settings page, and quick/instance actions remain page-local. Accounts contributes a dialog surface while the shell still owns shared account state. Settings-section and instance-action contribution state/renderers are not yet active. The table below is the target ownership model.

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
Current Phase 3 page/navigation contributions use static component references. Local lazy contribution loading remains a target capability. The checker/test suite contains the static import/dynamic-import rules, but the current release `prepare` gate remains `--phase 2`.

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
- `src-tauri/src/paths.rs` and `src-tauri/src/settings.rs` are the current baseline for this policy; they are not yet extracted into core.
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

### Phase 1: ModProvider pilot (Partial: registry/selection milestone implemented; full ports/checker pending)

Status: **Partial.** The provider contract/registry and mod selection milestone is implemented in `src-tauri/src/provider_api.rs` and `src-tauri/src/mods/providers.rs`, and shared mod operations select through the immutable registry. The active Phase 2 checker covers module/provider import and selection boundaries plus forbidden std/process/fs patterns; it allows reqwest/shared-client and reviewed parent-helper waivers by design. It does not yet enforce the full typed network/path/process/settings/logging/cancellation ports. Full Phase 1 acceptance therefore remains open for Phase 4/5 port and checker enforcement.

Scope:

- Define a provider-facing mod API and a small provider registry inside the current crate.
- Implement a pilot provider boundary for existing mod source behavior.
- Move source-specific search/version/dependency/artifact logic behind provider implementations.
- Keep the shared dependency planner, validation, archive inspection, sidecar handling, installed-mod behavior, DTOs, commands, and events in the mod feature.
- Keep `ModSource` as the serialized contract enum and perform enum-to-provider lookup in one adapter/mapping location.
- Define the versioned provider descriptor, per-capability API/selection-key bindings, full capability/provider-version compatibility predicate, independent provider leaves, and `ModProviderContext` ports described in this architecture.
- Implement the first phase of `scripts/check-architecture.mjs` and its CI gate for the available Rust module/import and frontend import graph.

Target acceptance criteria (full phase not yet met):

- No command body or shared mod service branches on individual mod sources after lookup.
- Duplicate provider IDs, duplicate capabilities, malformed duplicate selection-key bindings within one capability, incompatible provider/capability API versions, and unmet provider version requirements are rejected explicitly.
- Provider leaves initialize in lexicographic provider-ID order and have no provider-to-provider dependency graph. Selection filters active providers by the full requirement predicate, then requires exactly one match; zero or multiple matches fail without fallback.
- Target: providers receive only the full typed network/path/process/settings/logging/cancellation ports; direct Tauri/host handles and unrestricted filesystem/process access are outside the provider boundary. The active Phase 2 checker enforces only the implemented import/selection/forbidden-pattern boundaries and intentionally allows reqwest/shared-client and reviewed parent-helper waivers, so full port enforcement is deferred to Phase 4/5.
- Unknown, incompatible, ambiguous, or unavailable providers return an actionable error and never fall back silently.
- All existing mod commands, inputs, results, events, and error behavior remain contract-compatible.
- The provider cannot emit Tauri events, choose arbitrary write destinations, execute a downloaded mod, or import private host state.
- The phase does not introduce a Cargo workspace, runtime plugin loader, or dynamic provider discovery.
- Applicable Rust and TypeScript checks pass in CI, and the reviewer verifies logic and boundaries without running local build/test commands.
- The implemented registry/selection milestone is backed by the fail-closed `scripts/check-architecture.mjs`, its tests, and the Phase 2 CI gate for module/provider import and selection boundaries. Full Phase 1 acceptance is not claimed while typed provider ports and full port checks remain deferred to Phase 4/5.

### Phase 2: LoaderProvider (Partial: loader registry/leaf milestone implemented; full LoaderProvider ports pending)

Status: **Partial.** The loader provider/registry milestone is implemented in `src-tauri/src/loaders/providers.rs`: six independent loader leaves use typed list/install contexts and a progress sink, deterministic registry selection is preserved, and the 75-second metadata/600-second installer timeout split remains intact. Full `LoaderProviderContext` decomposition into network/path/archive/process/settings/logging/cancellation ports remains Phase 4/5 work. The Phase 2 checker active in CI verifies import and selection boundaries but intentionally does not enforce those full ports.

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
- Target: loader providers receive only the fully decomposed `LoaderProviderContext` ports; direct Tauri, HTTP, filesystem, or process implementations that bypass those ports fail the full Phase 4/5 architecture checker. The active Phase 2 checker intentionally allows reqwest/shared-client and reviewed parent-helper waivers, so this criterion is not yet met.
- CI verification is green before completion and the phase is reviewed for logic only by an independent reviewer.
- The architecture checker allowlist is extended for the loader registry/leaf milestone and passes in CI. Full Phase 2 acceptance is not claimed while full typed ports and full port enforcement remain deferred to Phase 4/5.

### Phase 3: Frontend contribution registry (Partial: page/navigation complete; slots pending)

Status: **Partial.** Page/navigation contributions, validation, the frozen registry, and registry-driven rendering are implemented. `settingsSections` and `instanceActions` contracts/validation are implemented but intentionally unpopulated and not mounted; existing settings sections and quick actions remain page-local. Full Phase 3 also needs the Phase 3 checker gate to become active in the release workflow `prepare` gate and those slots to be populated/migrated.

Scope:

- Introduce typed contribution contracts and a registry builder in a new frontend composition root.
- Replace the static `navItems`, `pageTitles`/`SectionId` conditional rendering, hard-coded settings grouping, and fixed instance/quick-action placement with registered contributions over time.
- Register navigation, page, settings-section, and instance-action contributions. Navigation uses `targetPageId` and must resolve by the canonical page contribution ID.
- Keep shared settings draft/persistence ownership and selected-instance context explicit.
- Preserve existing page behavior and any current URL/section semantics during the transition; page IDs and optional URL paths are not interchangeable identities. Phase 3 renders the statically registered, compiled feature set and does not depend on Rust lifecycle state or host IPC that is unavailable until Phase 4.

Acceptance criteria:

- The registry rejects any duplicate contribution ID across all frontend slots, unknown `targetPageId` references, invalid page targets, and invalid ordering before rendering.
- Shell rendering is registry-driven; adding a feature page does not require editing a hard-coded navigation array or a page conditional chain.
- Contribution order is deterministic by `order` then stable ID.
- A feature contribution may import only its own subtree plus frontend core/shared contracts; it must not import another feature module, including another feature public entry.
- No remote UI, arbitrary dynamic import, `eval`, or untrusted HTML becomes part of the registry.
- Dynamic imports are limited to one literal, relative, bundler-resolvable specifier; contribution loaders are component/lazy-function references, never runtime string paths or URLs.
- The TypeScript-AST phase of `scripts/check-architecture.mjs` enforces frontend import boundaries and the dynamic-import rule, and passes in CI before Phase 3 acceptance.
- Tauri event listeners and API calls are typed, cleaned up on unmount, and routed through shared clients.
- Existing navigation, settings, accounts, downloads, saves, launch, and instance actions remain behaviorally compatible.
- Host active-feature availability filtering is a Phase 4 requirement, not a Phase 3 dependency; Phase 3 acceptance verifies only the static compiled contribution registry and composition root.

### Phase 4: Workspace and module extraction (Pending)

Status: **Pending.** Rust remains one package; the Cargo workspace/extracted crates, Rust feature lifecycle registry, Rust composition root, provider runtime lifecycle, and active-feature availability filtering are not implemented.

Scope:

- Turn `src-tauri/Cargo.toml` into the workspace root while retaining the host package there.
- Create `flint-core`, feature/provider API crates, feature modules, and provider implementation crates.
- Move DTOs to their owning contracts and map at the Tauri adapter boundary.
- Move orchestration to feature modules and introduce the explicit Rust composition root.
- Keep `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`, `src-tauri/build.rs`, the target directory, and executable naming stable.
- Ensure the Tauri command list remains compile-time and thin.
- Extract the versioned feature/provider API contracts, typed feature/provider contexts, immutable registries, and lifecycle coordinator without changing public enum/DTO contracts.
- Expand `scripts/check-architecture.mjs` to the extracted-crate allowlist and full TypeScript import graph under CI.

Acceptance criteria:

- `cargo metadata` dependency edges satisfy the layer allowlist: no core-to-host, module-to-host, module-to-private-module, provider-to-host, provider-to-module, provider-to-provider, or Tauri dependency leaks; no allowlisted cycle exists; no broad directory/package wildcard hides an edge.
- The workspace has one lockfile at `src-tauri/Cargo.lock` and no member-local lockfiles.
- Release/version tooling still finds exactly the expected host package metadata and the same executable paths.
- `src-tauri/src/lib.rs` is a thin host entry: composition, command registration, and startup, with no provider-specific or feature business logic.
- Feature initialization, shutdown, duplicate rejection, feature-dependency resolution/cycle detection, deterministic topo order, independent leaf provider initialization in provider-ID order, and full-requirement selection have observable tests or CI checks. Provider-to-provider dependencies/cycles do not exist.
- Startup and runtime lifecycle transitions have observable tests or CI checks: failure propagates to the failed feature plus all transitive dependents; an affected set containing any `Required` feature takes the fatal path (`StartupFailed` at startup, `RuntimeFailed` -> `Stopping` -> `Stopped`/exit at runtime); `Degraded` is valid only while every `Required` feature remains `Ready`; no `Required` feature remains `Ready` with a non-ready dependency/provider. Startup-fatal diagnostic cleanup, unavailable commands, atomic init cleanup, reverse shutdown including initialized provider leaves/core, and Tauri `RunEvent::Exit` integration are covered.
- Source/import checks reject module/provider bypass of network, filesystem, process, settings, and logging ports. The TypeScript AST checker rejects every cross-feature frontend import (public or private), and the full architecture-checker allowlist passes before Phase 4 acceptance.
- Frontend feature folders can be changed without editing the shell except at the composition root.
- Phase 4 acceptance requires the read-only active-feature snapshot and shell filtering: contributions whose owning feature is not `Ready` are not mounted, failed/blocked contributions remain hidden, and unavailable-feature diagnostics are shown without fallback. Phase 3 itself renders only the static compiled feature set and has no host-availability dependency.
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

These are the end-state acceptance gates. The current baseline satisfies the implemented Phase 1/2 provider milestones and the Phase 3 page/navigation milestone only; Phase 3 settings/action migration, Phase 4, and Phase 5 remain pending. A change is architecture-compliant only when all applicable statements for its current phase hold:

- The current implementation is described as implemented, and every unimplemented target path is labeled planned/target/deferred.
- Applicable phase rules from the transition matrix in `DEVELOPMENT_STANDARDS.md` are satisfied; at full extraction, dependencies point inward/downward according to the layer table with no core-to-host, module-to-host, provider-to-host, or private-module edge.
- Cargo manifests and TypeScript imports are covered by the implemented Phase 2 milestone checker in the release workflow `prepare` job at `--phase 2`. It enforces module/provider import and selection boundaries, forbidden std/process/fs patterns, and exact allowlists while intentionally allowing reqwest/shared-client and reviewed parent-helper waivers. It does not enforce full typed ports; full typed-port and later-phase checks become normative when their gates advance. Cargo and TypeScript alone are not treated as enforcing arbitrary allowlists.
- Feature descriptors use `feature_version`, `api_version`, declared dependencies/provider requirements, startup policy, one initialization entry point, and declared backend contributions.
- Startup/runtime failure propagation includes the failed feature plus all transitive dependents. If the affected set contains any `Required` feature, the host is fatal: `StartupFailed` with feature/provider/core cleanup at startup, or `RuntimeFailed` -> `Stopping` -> `Stopped`/exit at runtime. `Degraded` is valid only when every `Required` feature remains `Ready` and the affected set is optional-only. No `Required` feature may remain `Ready` with a non-ready dependency/provider, and there is no feature-level `Degraded`. Unavailable commands, atomic init cleanup, and reverse shutdown are explicit.
- Progress events are best-effort and command results/durable state are authoritative; explicitly required protocol events fail the operation result without rolling back committed state.
- A feature has one stable ID, explicit feature and API versions, declared requirements, one initialization entry point, and declared contributions.
- Registries reject duplicate feature/provider/registration IDs, malformed capability/selection-key bindings, unknown requirements, incompatible API versions, and feature dependency cycles before side effects. Providers are independent leaves with no provider dependency graph.
- Feature order is deterministic topological order with feature-ID tie-breaking; provider leaves initialize in provider-ID order. Failure and shutdown behavior is explicit and tested.
- Provider selection uses active providers, per-capability selection-key bindings, and the full capability/API/provider-version requirement; exactly one match is required, with no fallback or call-site `match` branches after the relevant phase.
- Tauri commands are thin, compile-time registered, and contain no domain or provider logic.
- Frontend contribution IDs are globally unique across all slots, navigation uses canonical `targetPageId` page IDs, and feature modules import only their own subtree plus core/shared. The current implementation covers page/navigation contributions; `settingsSections`/`instanceActions` contracts validate but remain unpopulated/unmounted until full Phase 3 migration. Phase 4 adds host active-feature filtering. No arbitrary remote or runtime-string UI code is loaded.
- The affected authoritative contract, Rust/TypeScript DTOs, events, adapters, and documentation are updated together for every public behavior change; both RPC contract files are updated only when a shared model, command, or event surface is affected.
- No names, branding, identifiers, text, or code are copied from another project; references only inform expected behavior and the implementation remains original.
