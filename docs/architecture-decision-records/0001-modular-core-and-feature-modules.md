# Architecture Decision Record 0001: Modular Core and Feature Modules

## Status

**Status:** Accepted; implemented through Phase 5 in the working tree. GitHub Actions verification is pending and is not claimed green.

Phase 1 ModProvider and Phase 2 LoaderProvider are implemented through the shared actual-registry `ProviderRuntimeState`, typed provider contracts/ports, eight extracted provider leaves, and one provider lifecycle/catalog source used by both commands and feature availability. Phase 3 frontend contribution registration, validation, frozen registry, mounted pages/navigation/settings-section/instance-action slots, and registry-driven rendering are implemented. Phase 5 adds the typed overlay slot, exact five-file Tauri adapter boundary, injected `HostServices`, shared stale-request/event-cleanup hooks, strict loader parsing, archive preflight, bounded event redaction, and strict exception-free checker enforcement. The release checker runs frontend contract tests, architecture tests, and `node scripts/check-architecture.mjs --phase 5`; GitHub Actions remains the final verification authority. `src-tauri/Cargo.toml` is the workspace root, with `flint-core`, `flint-feature-api`, and `flint-provider-api` as extracted foundation crates. `flint-feature-api` implements the framework-neutral feature descriptor/registry/coordinator/gating/availability DTO, and `src-tauri/src/feature_host.rs` composes seven explicit features plus the shared `ProviderRuntimeState`, starts in Tauri setup, owns managed snapshot state, publishes `get_host_availability`, gates all 28 feature commands, filters all frontend slots, and shuts down on `RunEvent::Exit`. See [ARCHITECTURE.md](../ARCHITECTURE.md#canonical-migration-phase-status) for the canonical phase-status table.

## Date

2026-10-24

## Context

At acceptance time, the application worked as a modular monolith. The Rust backend was one package with sibling modules declared by `src-tauri/src/lib.rs`; all 29 Tauri commands were registered there and called module functions directly. Provider-specific mod and loader branches lived inside shared modules. The React frontend mounted one large composition component, kept shared application state there, hard-coded sidebar navigation, and conditionally rendered a fixed set of views. The Phase 1-2 registry/selection milestones and the Phase 3 frontend contribution registry have since been implemented. Phase 4 extraction, typed provider ports, the host lifecycle/availability IPC/gating/filtering milestone, and the active Cargo/source/typed-port checker gate are implemented. Phase 5 strict hardening is implemented in the working tree, including shared provider runtime lifecycle truth, exact frontend adapters/overlays, archive/redaction/stale-request hardening, and the strict Phase 5 checker gate; hosted verification remains pending.

The current structure has supported rapid delivery, but it makes several kinds of growth expensive:

- feature-specific changes spread through shared files;
- provider additions require new call-site matching and risk inconsistent safety policy;
- feature dependencies, initialization order, failure handling, and shutdown are implicit;
- the frontend shell must know every page and action;
- crate-level dependency direction cannot be enforced while all Rust code lives in one package;
- a future plugin-ready architecture could be added accidentally through ad hoc globals and dynamic branches rather than through a reviewed contract.

The external contracts are documented in [`API_CONTRACT.md`](../API_CONTRACT.md) and [`FEATURE_API_CONTRACT.md`](../FEATURE_API_CONTRACT.md). Phase 5 preserves command, event, and DTO names; it adds only documented gating, bounded/sanitized progress text, redacted launch-command display, and strict mod-loader validation behavior.

## Decision

Adopt a compile-time modular monolith with explicit layers, independent providers, lifecycle-aware feature registries, and typed frontend contributions.

1. **Layered Rust architecture.** Introduce a shared core, feature/provider API contracts, feature modules, provider implementations, and a thin Tauri host/adapter layer. Host and framework code stays at the edge; core, feature, and provider code never depends back on the host. Detailed dependency edges are canonical in [`ARCHITECTURE.md`](../ARCHITECTURE.md).
2. **Workspace extraction.** Preserve `src-tauri` as the host package/workspace root, one lockfile at `src-tauri/Cargo.lock`, the Tauri configuration, and the existing release target/executable paths.
3. **Descriptors and lifecycle registry.** Features declare stable IDs, `feature_version`, `api_version`, `startup_policy`, dependencies, provider capability requirements, one initialization entry point, and contributions. The registry validates before side effects, initializes deterministically, propagates Required/optional failure effects, gates unavailable commands, and shuts down in reverse successful-init order. No feature-level `Degraded` state exists.
4. **Independent providers.** Providers publish versioned, per-capability API/selection-key descriptors behind typed contexts. They are independent leaves initialized in lexicographic provider-ID order; provider-to-provider dependencies and provider dependency graphs are unsupported. One shared `ProviderRuntimeState` owns both actual registries, implements the feature provider catalog, records initialization/loss/shutdown state, and serves commands and feature availability. Selection uses the full capability/API/provider-version/selection-key/active-state predicate and requires exactly one match, with an unavailable or ambiguous error otherwise.
5. **Thin Tauri adapters.** Commands remain compile-time registered and gate before delegating to ready feature/application services. Adapters own DTO/event mapping, unavailable-feature errors, managed availability snapshot publication, and the `RunEvent::Exit` shutdown bridge. Provider-influenced progress messages are sanitized and bounded; launch-command display is redacted.
6. **Frontend contributions and adapters.** A typed contribution registry and explicit composition root provide pages, navigation, settings sections, instance actions, and overlays. The availability client validates `get_host_availability` and filters every slot by host/feature readiness. Tauri package access is confined to the exact typed adapters for invoke, events, dialogs, opener, and window; `main.tsx` injects `HostServices` through `AppPageContext`. Feature modules import only their own subtree plus core/shared contracts. Runtime string loaders, remote UI, and executable plugin code are forbidden.
7. **Completed incremental migration.** The ModProvider, LoaderProvider, frontend-registry, workspace/extraction, and Phase 5 hardening phases are implemented in the working tree, subject to GitHub Actions verification. Future extensions such as runtime plugins require a separate accepted decision and threat model.
8. **Boundary enforcement.** Cargo and TypeScript do not inherently enforce arbitrary layer allowlists. `scripts/check-architecture.mjs`, its architecture tests, and frontend contract tests run in the `flint-release.yml` release workflow `prepare` job and enforce the strict `--phase 5` policy: exact Tauri adapter/import and importer rules, overlay/host-service/stale-request contracts, exact Cargo workspace/internal/external dependency allowlists and cycles, all nineteen production source roots, module/provider import and selection boundaries, typed provider context ports, one shared actual-registry provider lifecycle source, forbidden raw std/process/fs/net/reqwest/Tauri patterns, exact allowlists, migration-shim/alias bans, and the host 30-command/28-feature-command gating map. Cargo is invoked only for workspace/package/dependency metadata and fails closed when unavailable or malformed. No temporary production exception remains. GitHub Actions is the final verification authority.
9. **Original work only.** References may inform expected behavior, but no names, branding, identifiers, text, or code may be copied from other projects. The implementation and documentation must be original.

Canonical ownership:

- [`ARCHITECTURE.md`](../ARCHITECTURE.md) is the canonical detailed source for boundaries, provider/registry semantics, lifecycle, frontend contributions, and migration phases.
- [`DEVELOPMENT_STANDARDS.md`](../DEVELOPMENT_STANDARDS.md) owns process, checklists, and concise mandatory summaries.
- This Architecture Decision Record documents rationale, the decision summary, consequences, and rejected alternatives.
- [`README.md`](../README.md) remains the index and source-of-truth map.

## Consequences

### Positive

- The strict Phase 5 checker enforces exact frontend Tauri adapters and importer paths, overlay/host-service/stale-request contracts, migration-shim bans, exact Cargo metadata/internal+external dependency rules and cycles, all-workspace source boundaries, one shared actual-registry provider lifecycle source, typed provider context ports, and exact allowlists through the release workflow `prepare` gate.
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
- Implementation is complete through Phase 5 in the working tree: provider contracts and eight leaves are extracted; Phase 3 contribution slots are implemented and mounted with an overlay slot; Phase 4 workspace/core/API/feature extraction, shared provider runtime state, lifecycle/availability IPC, frontend filtering, 28-command gates, and `RunEvent::Exit` shutdown are implemented; and Phase 5 strict adapters, shim removal, archive/redaction/stale-request hardening, and checker enforcement are present. The release workflow runs architecture and frontend contract tests followed by `--phase 5`; GitHub Actions remains the final authority and a green hosted run is not yet recorded.
- CI, not reviewer-run local commands, is the build/test authority. Completed work is committed locally and is not pushed unless requested.

## Rejected alternatives

### Folder-only refactor

Reorganizing files into `core`, `features`, and `providers` folders without crate boundaries was rejected. It can improve readability, but folders do not make forbidden dependencies impossible or force a stable public surface. Separate crates prevent some edges, yet a phase-aware allowlist checker is still required to enforce direction; the approved design uses the extracted `flint-core`/`flint-feature-api`/`flint-provider-api` foundation, all-workspace Cargo/source checks, and the strict Phase 5 frontend/provider gate.

### Manifest-only boundary enforcement

Relying on Cargo dependencies or TypeScript imports alone to enforce every layer rule was rejected as inaccurate. Cargo metadata can expose crate edges, but it cannot express arbitrary module/port policy; TypeScript has no built-in layer allowlist. The decision includes the implemented phase-aware checker using Rust source/import checks, Cargo metadata, and TypeScript compiler API/AST, active through the release workflow `prepare` gate at `--phase 5`; it enforces exact frontend adapter/importer, overlay/host-service/stale-request and shim rules, Cargo internal/external dependency rules and cycles, all-workspace source boundaries, shared provider lifecycle state, typed provider context ports, forbidden raw patterns, and exact allowlists.

### Immediate arbitrary native runtime plugins

Loading arbitrary native libraries at runtime was rejected for the initial architecture. A workable design needs a stable ABI, compatibility negotiation, trusted installation, provenance/signature policy, capability grants, isolation, resource limits, crash containment, update/removal semantics, and a plan for plugin-provided UI. Those requirements are not satisfied by adding a registry key or a dynamic library loader. Runtime plugins remain deferred pending a separate accepted Architecture Decision Record and threat model.

### Provider-to-provider dependency graphs

Provider-to-provider dependencies were rejected. Providers are independent leaves; linking them into a graph would add ordering, cycle, version, and lifecycle coupling that the provider model does not need. Cross-provider coordination is a core-service or feature-orchestration responsibility.

### Central feature switches and service locator

Keeping one host switch for every feature or introducing a generic global service locator was rejected. It would centralize knowledge, hide dependencies, make initialization order implicit, and encourage silent fallback. Descriptors, typed contexts, and explicit composition are required instead.

### One crate per existing file immediately

Splitting every current Rust file into a crate before the provider and frontend contracts are proven was rejected. File count is not an architecture. Extraction follows stable responsibility and dependency boundaries after the pilots establish the required contracts.

## Migration follow-ups

1. **Implemented through Phase 5; CI verification pending:** follow the canonical phase sequence and acceptance criteria in [`ARCHITECTURE.md`](../ARCHITECTURE.md), applying the mandatory rules in [`DEVELOPMENT_STANDARDS.md`](../DEVELOPMENT_STANDARDS.md).
2. **Implemented:** ModProvider and LoaderProvider contracts, independent provider leaves, per-capability selection-key bindings, full-requirement selection, typed operation contexts/ports, shared actual-registry `ProviderRuntimeState`, and strict checker enforcement are present.
3. **Implemented:** the frontend contribution registry/composition root renders pages, navigation, settings sections, instance actions, and overlays with frozen validation and shared boundaries. Availability filtering covers every slot and fails closed.
4. **Implemented:** the Cargo workspace root and foundation crates are present. `flint-feature-api` covers validated descriptors, deterministic registry/order, provider-catalog abstraction, affected-set propagation, Required-fatality and optional degradation, Required-Ready invariants, command gating, reverse/idempotent coordinator shutdown, and the availability DTO. `feature_host.rs` composes seven features and consumes the shared `ProviderRuntimeState`; the host publishes availability, filters frontend slots, gates all 28 feature commands, and invokes exit shutdown.
5. **Implemented:** the release workflow runs architecture and frontend contract tests, then `node scripts/check-architecture.mjs --phase 5`. The strict gate covers exact Tauri adapters/importers, overlays, stale-request/shim rules, all nineteen Cargo packages and source roots, internal/external dependencies/cycles, typed provider ports, shared provider runtime truth, and the host command gate map. GitHub Actions has not yet recorded a green run for this working tree.
6. **Pending operational evidence only:** record the hosted GitHub Actions result and complete live desktop/integration validation; do not treat local commands or this documentation update as CI evidence.
7. Require a new accepted Architecture Decision Record before runtime plugin loading, remote UI, dynamic command registration, provider-to-provider dependencies, or any weakening of the dependency/security/allowlist rules.

## Links

- [`ARCHITECTURE.md`](../ARCHITECTURE.md)
- [`DEVELOPMENT_STANDARDS.md`](../DEVELOPMENT_STANDARDS.md)
- [`API_CONTRACT.md`](../API_CONTRACT.md)
- [`FEATURE_API_CONTRACT.md`](../FEATURE_API_CONTRACT.md)
- [`README.md`](../README.md)
