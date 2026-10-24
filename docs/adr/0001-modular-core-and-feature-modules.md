# ADR 0001: Modular Core and Feature Modules

## Status

**Status:** Accepted; implementation pending.

## Date

2026-10-24

## Context

The application currently works as a modular monolith. The Rust backend is one package with sibling modules declared by `src-tauri/src/lib.rs`; all 29 Tauri commands are registered there and call module functions directly. Provider-specific mod and loader branches live inside shared modules. The React frontend mounts one large composition component, keeps shared application state there, hard-codes sidebar navigation, and conditionally renders a fixed set of views.

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
5. **Thin Tauri adapters.** Commands remain compile-time registered and delegate to ready feature/application services. Adapters own DTO/event mapping, unavailable-feature errors, and the planned `RunEvent::Exit`/runtime-failure shutdown bridge. Detailed event and lifecycle mechanics are canonical in [`ARCHITECTURE.md`](../ARCHITECTURE.md).
6. **Frontend contributions.** A typed contribution registry and composition root replace hard-coded shell feature lists. Phase 3 renders the statically registered compiled feature set; Phase 4 adds host active-feature availability filtering. Frontend features may import only their own subtree plus core/shared contracts; cross-feature composition goes through typed core contracts, shell services/contexts, or contribution slots. Runtime string loaders, remote UI, and executable plugin code are forbidden.
7. **Incremental migration.** The sequence is ModProvider, LoaderProvider, frontend registry, workspace/module extraction, and hardening. The phase applicability matrix in [`DEVELOPMENT_STANDARDS.md`](../DEVELOPMENT_STANDARDS.md) defines what is mandatory before each target structure exists. Runtime plugins require a separate accepted decision.
8. **Boundary enforcement.** Cargo and TypeScript do not inherently enforce arbitrary layer allowlists. A planned `scripts/check-architecture.mjs` will enforce phase-aware Rust and frontend edges in CI; it does not exist at acceptance time. Strict frontend imports prohibit cross-feature imports, including public feature entries.
9. **Original work only.** References may inform expected behavior, but no names, branding, identifiers, text, or code may be copied from other projects. The implementation and documentation must be original.

Canonical ownership:

- [`ARCHITECTURE.md`](../ARCHITECTURE.md) is the canonical detailed source for boundaries, provider/registry semantics, lifecycle, frontend contributions, and migration phases.
- [`DEVELOPMENT_STANDARDS.md`](../DEVELOPMENT_STANDARDS.md) owns process, checklists, and concise mandatory summaries.
- This ADR records rationale, the decision summary, consequences, and rejected alternatives.
- [`README.md`](../README.md) remains the index and source-of-truth map.

## Consequences

### Positive

- A planned manifest/source/TypeScript-AST checker can enforce explicit layer allowlists rather than relying on convention or the false assumption that Cargo/TypeScript inherently enforce them.
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
- CI, not reviewer-run local commands, is the build/test authority. Completed work is committed locally and is not pushed unless requested.

## Rejected alternatives

### Folder-only refactor

Reorganizing files into `core`, `features`, and `providers` folders without crate boundaries was rejected. It can improve readability, but folders do not make forbidden dependencies impossible or force a stable public surface. Separate crates prevent some edges, yet a phase-aware allowlist checker is still required to enforce direction; the approved design allows safe intermediate in-crate steps but targets explicit crate/API boundaries plus the planned checker.

### Manifest-only boundary enforcement

Relying on Cargo dependencies or TypeScript imports alone to enforce every layer rule was rejected as inaccurate. Cargo metadata can expose crate edges, but it cannot express arbitrary module/port policy; TypeScript has no built-in layer allowlist. The decision therefore includes a planned phase-aware architecture checker using cargo metadata, Rust source/import checks, and the TypeScript compiler API/AST, with CI gating after Phase 1.

### Immediate arbitrary native runtime plugins

Loading arbitrary native libraries at runtime was rejected for the initial architecture. A workable design needs a stable ABI, compatibility negotiation, trusted installation, provenance/signature policy, capability grants, isolation, resource limits, crash containment, update/removal semantics, and a plan for plugin-provided UI. Those requirements are not satisfied by adding a registry key or a dynamic library loader. Runtime plugins remain deferred pending a separate accepted ADR and threat model.

### Provider-to-provider dependency graphs

Provider-to-provider dependencies were rejected. Providers are independent leaves; linking them into a graph would add ordering, cycle, version, and lifecycle coupling that the provider model does not need. Cross-provider coordination is a core-service or feature-orchestration responsibility.

### Central feature switches and service locator

Keeping one host switch for every feature or introducing a generic global service locator was rejected. It would centralize knowledge, hide dependencies, make initialization order implicit, and encourage silent fallback. Descriptors, typed contexts, and explicit composition are required instead.

### One crate per existing file immediately

Splitting every current Rust file into a crate before the provider and frontend contracts are proven was rejected. File count is not an architecture. Extraction follows stable responsibility and dependency boundaries after the pilots establish the required contracts.

## Migration follow-ups

1. Follow the canonical five-phase sequence and acceptance criteria in [`ARCHITECTURE.md`](../ARCHITECTURE.md), applying the transition rules in [`DEVELOPMENT_STANDARDS.md`](../DEVELOPMENT_STANDARDS.md).
2. Pilot the ModProvider and LoaderProvider contracts with independent provider leaves, per-capability selection-key bindings, full-requirement selection, typed contexts, and the Phase 1 architecture-checker CI gate.
3. Introduce the Phase 3 frontend contribution registry/composition root over the static compiled feature set; add host active-feature filtering only in Phase 4.
4. Extract the Cargo workspace/module crates and implement the canonical startup/runtime lifecycle, including affected-set propagation, `StartupFailed`/`RuntimeFailed` fatal paths, the every-Required-Ready degraded invariant, unavailable commands, reverse shutdown, and Tauri `RunEvent::Exit` integration.
5. Expand `scripts/check-architecture.mjs` through the phase allowlists, including strict no-cross-feature frontend imports and full Cargo/TypeScript graph checks.
6. Remove temporary dispatch paths and checker waivers, then update implementation status in [`ARCHITECTURE.md`](../ARCHITECTURE.md) and the index in [`README.md`](../README.md).
7. Require a new accepted ADR before runtime plugin loading, remote UI, dynamic command registration, provider-to-provider dependencies, or any weakening of the dependency/security/allowlist rules.

## Links

- [`ARCHITECTURE.md`](../ARCHITECTURE.md)
- [`DEVELOPMENT_STANDARDS.md`](../DEVELOPMENT_STANDARDS.md)
- [`API_CONTRACT.md`](../API_CONTRACT.md)
- [`FEATURE_API_CONTRACT.md`](../FEATURE_API_CONTRACT.md)
- [`README.md`](../README.md)
