# Architecture Decision Record 0001: Modular Core and Feature Modules

## Status

**Status:** Accepted; implementation in progress.

Phase 1 ModProvider and Phase 2 LoaderProvider registry/selection **milestones** are implemented, while full provider runtime lifecycle hardening remains Phase 5 work. Phase 3 frontend contribution registration, validation, frozen registry, mounted pages/navigation/settings-section/instance-action slots, and registry-driven rendering are implemented. The Phase 4 checker gate is active at `--phase 4`; it enforces frontend manifest/composition/import boundaries, exact Cargo metadata/internal+external dependency allowlists and cycles, all nineteen workspace source roots, typed provider context ports, exact workspace membership, and the host 30-command/28-feature-command gating map, while intentionally allowing reqwest/shared-client and reviewed parent-helper waivers. `src-tauri/Cargo.toml` is the workspace root, with `flint-core`, `flint-feature-api`, and `flint-provider-api` as extracted foundation crates. `flint-feature-api` implements the framework-neutral feature descriptor/registry/coordinator/gating/availability DTO, and `src-tauri/src/feature_host.rs` composes seven explicit features plus real provider metadata, starts in Tauri setup, owns managed snapshot state, publishes `get_host_availability`, gates all 28 feature commands, filters frontend contributions, and shuts down on `RunEvent::Exit`. The settings, saves, accounts, vanilla, launch, mods, and loaders feature crates are implemented; provider runtime lifecycle hardening and Phase 5 remain pending. See [ARCHITECTURE.md](../ARCHITECTURE.md#canonical-migration-phase-status) for the canonical phase-status table.

## Date

2026-10-24

## Context

At acceptance time, the application worked as a modular monolith. The Rust backend was one package with sibling modules declared by `src-tauri/src/lib.rs`; all 29 Tauri commands were registered there and called module functions directly. Provider-specific mod and loader branches lived inside shared modules. The React frontend mounted one large composition component, kept shared application state there, hard-coded sidebar navigation, and conditionally rendered a fixed set of views. The Phase 1-2 registry/selection milestones and the Phase 3 frontend contribution registry have since been implemented. Phase 4 has progressed through the workspace/core/feature-API/provider-API foundation, host lifecycle/availability IPC/gating/filtering milestone, and the active Cargo/source/typed-port checker gate; provider runtime lifecycle hardening and Phase 5 remain pending.

The current structure has supported rapid delivery, but it makes several kinds of growth expensive:

- feature-specific changes spread through shared files;
- provider additions require new call-site matching and risk inconsistent safety policy;
- feature dependencies, initialization order, failure handling, and shutdown are implicit;
- the frontend shell must know every page and action;
- crate-level dependency direction cannot be enforced while all Rust code lives in one package;
- a future plugin-ready architecture could be added accidentally through ad hoc globals and dynamic branches rather than through a reviewed contract.

The external contracts are already documented in [`API_CONTRACT.md`](../API_CONTRACT.md) and [`FEATURE_API_CONTRACT.md`](../FEATURE_API_CONTRACT.md). They must remain stable while internal boundaries improve.

## Decision

Adopt a compile-time modular monolith with explicit layers, independent providers, lifecycle-aware feature registries, and typed frontend contributions.

1. **Layered Rust architecture.** Introduce a shared core, feature/provider API contracts, feature modules, provider implementations, and a thin Tauri host/adapter layer. Host and framework code stays at the edge; core, feature, and provider code never depends back on the host. Detailed dependency edges are canonical in [`ARCHITECTURE.md`](../ARCHITECTURE.md).
2. **Workspace extraction.** Preserve `src-tauri` as the host package/workspace root, one lockfile at `src-tauri/Cargo.lock`, the Tauri configuration, and the existing release target/executable paths.
3. **Descriptors and lifecycle registry.** Features declare stable IDs, `feature_version`, `api_version`, `startup_policy`, dependencies, provider capability requirements, one initialization entry point, and contributions. The registry validates before side effects, initializes deterministically, and applies the canonical startup/runtime failure transitions from [`ARCHITECTURE.md`](../ARCHITECTURE.md). No feature-level `Degraded` state exists.
4. **Independent providers.** Providers publish versioned, per-capability API/selection-key descriptors behind typed contexts. They are independent leaves initialized in lexicographic provider-ID order; provider-to-provider dependencies and provider dependency graphs are unsupported. Selection uses the full capability/API/provider-version/selection-key/active-state predicate and requires exactly one match, with an unavailable or ambiguous error otherwise.
5. **Thin Tauri adapters.** Commands remain compile-time registered and delegate to ready feature/application services. Adapters own DTO/event mapping, the implemented unavailable-feature gate/error, managed availability snapshot publication, and the implemented `RunEvent::Exit` shutdown bridge. Detailed event and lifecycle mechanics are canonical in [`ARCHITECTURE.md`](../ARCHITECTURE.md).
6. **Frontend contributions.** A typed contribution registry and composition root replace hard-coded shell feature lists. Phase 3 renders the statically registered compiled feature set; the Phase 4 availability client validates `get_host_availability` and filters host/feature-unready contributions. Frontend features may import only their own subtree plus core/shared contracts; cross-feature composition goes through typed core contracts, shell services/contexts, or contribution slots. Runtime string loaders, remote UI, and executable plugin code are forbidden.
7. **Incremental migration.** The sequence is ModProvider, LoaderProvider, frontend registry, workspace/module extraction, and hardening. The phase applicability matrix in [`DEVELOPMENT_STANDARDS.md`](../DEVELOPMENT_STANDARDS.md) defines what is mandatory before each target structure exists. Runtime plugins require a separate accepted decision.
8. **Boundary enforcement.** Cargo and TypeScript do not inherently enforce arbitrary layer allowlists. The implemented `scripts/check-architecture.mjs` and its tests run in the `flint-release.yml` release workflow `prepare` job at `--phase 4` and enforce frontend manifest/composition/import boundaries, exact Cargo workspace/internal/external dependency allowlists and cycles, all nineteen production source roots, module/provider import and selection boundaries, typed provider context ports, forbidden raw std/process/fs/net/reqwest/Tauri patterns, exact allowlists, and the host 30-command/28-feature-command gating map; reviewed Phase 5 Tauri/helper waivers remain intentional. Cargo is invoked only for workspace/package/dependency metadata and fails closed when unavailable or malformed. Strict frontend imports prohibit cross-feature imports, including public feature entries.
9. **Original work only.** References may inform expected behavior, but no names, branding, identifiers, text, or code may be copied from other projects. The implementation and documentation must be original.

Canonical ownership:

- [`ARCHITECTURE.md`](../ARCHITECTURE.md) is the canonical detailed source for boundaries, provider/registry semantics, lifecycle, frontend contributions, and migration phases.
- [`DEVELOPMENT_STANDARDS.md`](../DEVELOPMENT_STANDARDS.md) owns process, checklists, and concise mandatory summaries.
- This Architecture Decision Record documents rationale, the decision summary, consequences, and rejected alternatives.
- [`README.md`](../README.md) remains the index and source-of-truth map.

## Consequences

### Positive

- The implemented Phase 4 checker enforces frontend manifest/composition/import boundaries, exact Cargo metadata/internal+external dependency rules and cycles, all-workspace source boundaries, typed provider context ports, and exact allowlists through the release workflow `prepare` gate at `--phase 4`, while intentionally retaining reviewed Phase 5 Tauri/helper waivers. Phase 5 hardening remains pending.
- Provider additions no longer require scattered branching in shared services.
- Startup order and failure behavior become explicit, deterministic, and testable.
- Features own their state and contributions, reducing shell complexity.
- Public contracts can evolve with clearer versioning and adapter boundaries.
- The release/build path can remain stable while internals are extracted.
- Future runtime plugins, if ever approved, have a reviewed starting point rather than hidden dynamic loading.

### Negative and costs

- The first migration adds manifests, modules, adapters, and validation code before it adds user-visible function.
- More crate boundaries increase build graph coordination and API-review overhead.
- Feature and provider versions must be maintained carefully; premature crate extraction could freeze unstable interfaces.
- The frontend composition root and contribution contracts require coordinated changes to types, tests, and state ownership. Strict no-cross-feature imports increase the need for explicit core/shell contracts, but prevent feature-to-feature coupling.
- Failure propagates to the failed feature plus all transitive dependents. Any affected set containing a `Required` feature is fatal: `StartupFailed` with cleanup at startup, or `RuntimeFailed` -> `Stopping` -> `Stopped`/exit at runtime. `Degraded` is valid only when every `Required` feature remains `Ready` and the affected set is optional-only; a `Required` feature never remains `Ready` with a non-ready dependency/provider. The canonical lifecycle is in [`ARCHITECTURE.md`](../ARCHITECTURE.md). Hot loading, automatic restart, and independent recovery of failed features are not part of the first implementation.
- Dynamic plugin capabilities such as isolation, stable ABI, signing, and crash containment are not available in this decision.

### Neutral and operational

- The application remains a modular monolith and ships as one executable.
- The frontend remains one bundled application; lazy chunks are local build artifacts only. Event delivery does not roll back committed durable state, and required vs best-effort event semantics are explicit.
- Existing commands, DTOs, events, and user-visible behavior remain authoritative throughout migration. Public behavior changes update the affected contract; both RPC contracts change only for shared surfaces.
- Implementation is in progress: Phase 1/2 registry/selection milestones are implemented; Phase 3 frontend contribution slots are implemented and mounted; Phase 4 workspace/core/feature-API/provider-API foundation, host lifecycle/availability milestone, and Cargo/source/typed-port checker gate are implemented, including the framework-neutral feature lifecycle registry/coordinator, explicit seven-feature host composition, managed startup/snapshot state, ungated availability IPC, frontend filtering, 28-command gates, and `RunEvent::Exit` shutdown. The settings, saves, accounts, vanilla, launch, mods, and loaders feature crates are implemented; provider runtime lifecycle hardening and Phase 5 remain pending. The checker/tests and the release workflow `prepare` Phase 4 gate are live.
- CI, not reviewer-run local commands, is the build/test authority. Completed work is committed locally and is not pushed unless requested.

## Rejected alternatives

### Folder-only refactor

Reorganizing files into `core`, `features`, and `providers` folders without crate boundaries was rejected. It can improve readability, but folders do not make forbidden dependencies impossible or force a stable public surface. Separate crates prevent some edges, yet a phase-aware allowlist checker is still required to enforce direction; the approved design allows safe intermediate in-crate steps and now has the active Phase 4 checker for frontend boundaries, exact Cargo metadata and all-workspace sources, and typed provider boundaries plus the extracted `flint-core`/`flint-feature-api`/`flint-provider-api` foundation, while Phase 5 hardening remains target work.

### Manifest-only boundary enforcement

Relying on Cargo dependencies or TypeScript imports alone to enforce every layer rule was rejected as inaccurate. Cargo metadata can expose crate edges, but it cannot express arbitrary module/port policy; TypeScript has no built-in layer allowlist. The decision includes the implemented phase-aware checker using Rust source/import checks, Cargo metadata, and TypeScript compiler API/AST, active through the release workflow `prepare` gate at `--phase 4`; it enforces frontend manifest/composition/import, exact Cargo internal/external dependency rules and cycles, all-workspace source boundaries, typed provider context ports, forbidden raw patterns, and exact allowlists. Phase 5 strict hardening remains future work.

### Immediate arbitrary native runtime plugins

Loading arbitrary native libraries at runtime was rejected for the initial architecture. A workable design needs a stable ABI, compatibility negotiation, trusted installation, provenance/signature policy, capability grants, isolation, resource limits, crash containment, update/removal semantics, and a plan for plugin-provided UI. Those requirements are not satisfied by adding a registry key or a dynamic library loader. Runtime plugins remain deferred pending a separate accepted Architecture Decision Record and threat model.

### Provider-to-provider dependency graphs

Provider-to-provider dependencies were rejected. Providers are independent leaves; linking them into a graph would add ordering, cycle, version, and lifecycle coupling that the provider model does not need. Cross-provider coordination is a core-service or feature-orchestration responsibility.

### Central feature switches and service locator

Keeping one host switch for every feature or introducing a generic global service locator was rejected. It would centralize knowledge, hide dependencies, make initialization order implicit, and encourage silent fallback. Descriptors, typed contexts, and explicit composition are required instead.

### One crate per existing file immediately

Splitting every current Rust file into a crate before the provider and frontend contracts are proven was rejected. File count is not an architecture. Extraction follows stable responsibility and dependency boundaries after the pilots establish the required contracts.

## Migration follow-ups

1. **Phase 4 checker gate implemented; Phase 5 pending:** follow the canonical five-phase sequence and acceptance criteria in [`ARCHITECTURE.md`](../ARCHITECTURE.md), applying the transition rules in [`DEVELOPMENT_STANDARDS.md`](../DEVELOPMENT_STANDARDS.md).
2. **Milestone implemented; runtime hardening pending:** ModProvider and LoaderProvider contracts, independent provider leaves, per-capability selection-key bindings, full-requirement selection, typed operation contexts/ports, and the active Phase 4 checker are implemented. Full provider runtime lifecycle hardening remains Phase 5 work.
3. **Implemented:** the frontend contribution registry/composition root renders pages, navigation, settings sections, and instance actions, with shared core boundaries and renderers. The Phase 4 availability client filters those slots from the host snapshot and fails closed.
4. **Phase 4 checker gate implemented; runtime hardening pending:** the Cargo workspace root and the `flint-core`/`flint-feature-api`/`flint-provider-api` foundation crates are implemented. `flint-feature-api` covers validated descriptors, deterministic feature registry/order, framework-neutral provider catalog abstraction, affected-set propagation, Required-fatality and optional degradation, Required-Ready invariants, command gating, reverse/idempotent coordinator shutdown, and the availability DTO. `src-tauri/src/feature_host.rs` explicitly registers seven backend features and real provider metadata, starts the coordinator in Tauri setup, owns managed startup/availability state, publishes `get_host_availability`, applies frontend snapshot filtering, gates all 28 feature commands, and invokes exit shutdown. The settings, saves, accounts, vanilla, launch, mods, and loaders feature crates are implemented; provider runtime lifecycle hardening and Phase 5 remain pending.
5. **Implemented Phase 4 gate:** `scripts/check-architecture.mjs` and its tests run in the `flint-release.yml` release workflow `prepare` job, which installs frontend dependencies, runs the checker tests, then runs the checker at `--phase 4`; all build-matrix jobs depend on `prepare`. That gate covers frontend boundaries, exact nineteen-package Cargo metadata/internal+external dependency allowlists/cycles, all nineteen production source roots, typed provider context ports, forbidden raw APIs, exact allowlists, and the host 30-command/28-feature-command gating map while retaining reviewed Phase 5 waivers. Phase 5 hardening remains pending.
6. **Pending Phase 5:** remove temporary dispatch paths and checker waivers, and update implementation status in [`ARCHITECTURE.md`](../ARCHITECTURE.md) and the index in [`README.md`](../README.md).
7. Require a new accepted Architecture Decision Record before runtime plugin loading, remote UI, dynamic command registration, provider-to-provider dependencies, or any weakening of the dependency/security/allowlist rules.

## Links

- [`ARCHITECTURE.md`](../ARCHITECTURE.md)
- [`DEVELOPMENT_STANDARDS.md`](../DEVELOPMENT_STANDARDS.md)
- [`API_CONTRACT.md`](../API_CONTRACT.md)
- [`FEATURE_API_CONTRACT.md`](../FEATURE_API_CONTRACT.md)
- [`README.md`](../README.md)
