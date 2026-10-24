# Flint Documentation

This directory is the maintained documentation set for Flint. It separates current contracts and operational guidance from the accepted target architecture that is still being migrated.

## Status note

The application currently ships as a **partially modularized monolith**:

- Rust remains a single Cargo package rooted at `src-tauri`; no Cargo workspace or extracted core/API/feature/provider crates exists yet.
- Phase 1 ModProvider **registry/selection milestone** is implemented in `src-tauri/src/provider_api.rs` and `src-tauri/src/mods/providers.rs`, with shared mod operations selecting through the provider registry. The full phase remains partial because typed provider ports and full checker enforcement are deferred to Phase 4/5.
- Phase 2 LoaderProvider **registry/leaf milestone** is implemented in `src-tauri/src/loaders/providers.rs`: six independent loader leaves, list/install contexts, progress sink, deterministic selection, and the preserved 75-second/600-second timeout split. The full phase remains partial because full `LoaderProviderContext` port decomposition and checker enforcement are deferred to Phase 4/5.
- Phase 3 page/navigation registry is implemented in `src/core/contributions.ts`, `src/app/composition.ts`, `src/app/App.tsx`, and `src/app/AppSidebar.tsx`. `settingsSections` and `instanceActions` contracts/validation exist but are intentionally unpopulated and not mounted; existing settings sections/quick actions remain inside page components.
- `scripts/check-architecture.mjs`, its tests, the dedicated architecture CI workflow, and the release architecture gate are implemented. Both CI and release invoke `--phase 2`, which enforces module/provider import and selection boundaries, forbidden std/process/fs patterns, and exact allowlists while intentionally allowing reqwest/shared-client and reviewed parent-helper waivers. It does not enforce full typed ports; Phase 3+, full typed-port, and Phase 4/5 gates remain pending.
- Phase 4 workspace/crate extraction, Rust `FeatureDescriptor`/lifecycle registry/composition root, active-feature snapshot/filtering, and Phase 5 hardening remain pending.
- Existing public commands, DTOs, and events remain unchanged.

The full typed provider ports, target Cargo workspace, extracted crates, Rust feature lifecycle/composition root, complete four-slot frontend migration, and active-feature filtering remain planned. See the canonical phase-status table in [`ARCHITECTURE.md`](ARCHITECTURE.md#canonical-migration-phase-status), and [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) for the phase applicability matrix and concise current-phase cross-reference.

Canonical ownership: [`ARCHITECTURE.md`](ARCHITECTURE.md) is the detailed source for boundaries, provider/registry semantics, lifecycle, frontend contributions, and phases; [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) owns process/checklists/concise summaries; the [Architecture Decision Record](architecture-decision-records/0001-modular-core-and-feature-modules.md) documents rationale/consequences; this README is the index/source-of-truth map.

## Documentation index

| Document | Purpose | Status |
| --- | --- | --- |
| [`README.md`](README.md) | Documentation index, reading order, and source-of-truth map. | Current |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Canonical current baseline, phase-status table, target design, provider/registry/lifecycle/event rules, frontend contributions, and migration phases. | Current design; Phase 1/2 milestones implemented but both phases partial; Phase 3 partial |
| [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) | Phase applicability/transition matrix, concise cross-reference to the canonical phase-status table, and mandatory before-work, module-admission, provider, security, documentation, review, and delivery standards. | Current policy |
| [`architecture-decision-records/0001-modular-core-and-feature-modules.md`](architecture-decision-records/0001-modular-core-and-feature-modules.md) | Accepted architecture decision, consequences, rejected alternatives, and status-tracked follow-ups. | Accepted; implementation in progress |
| [`API_CONTRACT.md`](API_CONTRACT.md) | Base frontend/backend commands, DTOs, events, and behavioral rules. | Current RPC source of truth |
| [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md) | Extended account, loader, mod, and save commands, DTOs, events, and UI expectations. | Current RPC source of truth |
| [`INTEGRATIONS.md`](INTEGRATIONS.md) | External account/mod-catalog setup, local storage, privacy, and current-instance behavior. | Current operational guidance |
| [`CODE_REVIEW.md`](CODE_REVIEW.md) | Point-in-time quality, fixed-defect, readiness, and risk snapshot. | Historical snapshot; not proof of later checks |

## Source-of-truth map

| Question | Authoritative document | Notes |
| --- | --- | --- |
| What is the exact command name, argument shape, result shape, or DTO field? | The applicable section of [`API_CONTRACT.md`](API_CONTRACT.md) or [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md) | The applicable contract wins over internal architecture or UI naming. Update both only when the shared surface appears in both. |
| What behavior must a command or feature preserve? | The applicable section of [`API_CONTRACT.md`](API_CONTRACT.md) or [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md) | Behavioral rules and UI expectations are part of the contract. Update both contract files only when the shared model, command, or event surface is documented in both. |
| What event name and payload are public, and what is its failure semantics? | The applicable section of [`API_CONTRACT.md`](API_CONTRACT.md) or [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md), plus [`ARCHITECTURE.md`](ARCHITECTURE.md) | Public events must be updated in their authoritative contract with payload and failure semantics. Command results/durable state are authoritative; default progress events are best-effort; explicitly required protocol events must fail the result without rolling back committed state. Update both contract files only for a shared event surface. |
| How should internal Rust crates, modules, providers, and registries be structured? | [`ARCHITECTURE.md`](ARCHITECTURE.md) | Provider contract/registry milestones live in the single `src-tauri` package; full typed provider ports, workspace/crate extraction, and Rust feature lifecycle remain planned/target-only. |
| How are duplicates, feature cycles, provider capability/API/version selection, deterministic initialization, startup degradation, `RuntimeFailed`, unavailable commands, and shutdown handled? | [`ARCHITECTURE.md`](ARCHITECTURE.md) | Architecture is canonical. Current provider registry/selection milestones are immutable per operation, but full typed contexts/ports and the Rust feature lifecycle/active-state coordinator remain pending Phase 4/5. Target failure propagation includes all transitive dependents; any affected `Required` feature is fatal, and `Degraded` is valid only while every `Required` feature remains `Ready`. Providers are independent leaves; feature cycles are detected separately. |
| How should feature UI appear in navigation, pages, settings, and instance actions? | [`ARCHITECTURE.md`](ARCHITECTURE.md) | Page/navigation registry rendering is implemented. `settingsSections`/`instanceActions` contracts/validation exist but are unpopulated and unmounted; existing UI remains page-local. Phase 4 adds active-feature filtering. Contribution IDs are globally unique across all frontend slots; navigation uses canonical `targetPageId`. |
| May I add a module, change a contract, or choose a dependency direction now? | [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) | Follow the partial-Phase 1/2 plus partial-Phase 3 current transition path and phase applicability matrix; target-only requirements remain explicitly gated. |
| How are dependency boundaries enforced? | [`ARCHITECTURE.md`](ARCHITECTURE.md) and [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) | The implemented checker/tests run at `--phase 2` in dedicated and release CI, enforcing module/provider import and selection boundaries, forbidden std/process/fs patterns, and exact allowlists while allowing reqwest/shared-client and reviewed parent-helper waivers. It does not enforce full typed ports. Phase 3+ TypeScript gates, full typed-port enforcement, and Phase 4 Cargo workspace metadata gates are not active yet. |
| How is review and completion handled? | [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) | Independent logic review, no reviewer-run local build/test, CI authority, active `--phase 2` milestone checker gate (not full typed-port acceptance), local commit only unless push is requested. |
| How is an external account or mod-catalog integration configured, and where is local data stored? | [`INTEGRATIONS.md`](INTEGRATIONS.md) | Do not move secrets or storage paths based on this index alone. |
| Why was the modular architecture chosen, and what alternatives were rejected? | [`architecture-decision-records/0001-modular-core-and-feature-modules.md`](architecture-decision-records/0001-modular-core-and-feature-modules.md) | The Architecture Decision Record documents rationale; it does not replace current contracts. |
| What was the last recorded quality/readiness snapshot? | [`CODE_REVIEW.md`](CODE_REVIEW.md) | Do not treat it as evidence for a later or different commit. |

The repository source remains the evidence of what is implemented. A mismatch between current source and a current contract is a defect to resolve; a planned architecture document does not make an unimplemented path real.

## Reading order

1. [`ARCHITECTURE.md`](ARCHITECTURE.md): read the canonical phase-status table first, then the current baseline, target rules, provider/lifecycle/event semantics, phase-2 milestone checker scope, and remaining migration sequence.
2. [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md): start with the partial-Phase 1/2 plus partial-Phase 3 cross-reference and transition path, then use the phase matrix and apply the implementation, security, checklist, review, and delivery rules.
3. [`architecture-decision-records/0001-modular-core-and-feature-modules.md`](architecture-decision-records/0001-modular-core-and-feature-modules.md): understand why the architecture was accepted and which alternatives were rejected.
4. Read [`API_CONTRACT.md`](API_CONTRACT.md) and [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md) for the area being changed.
5. Read [`INTEGRATIONS.md`](INTEGRATIONS.md) for setup, local storage, privacy, and selected-instance behavior.
6. Use [`CODE_REVIEW.md`](CODE_REVIEW.md) only as a historical readiness snapshot, not as current CI evidence.

## Documentation rules

- Label behavior as **Implemented**, **Partially implemented**, **Planned**, or **Deferred**, and identify the migration phase/milestone. Never present full typed provider ports, target-only extracted crates, the Rust feature lifecycle/composition root, active-feature filtering, mounted settings/instance-action slots, or later-phase checker gates as implemented/active before they are.
- Use only relative Markdown links that resolve inside `docs/`. Do not link to another checkout, an external site, or a repository file outside the documentation tree.
- Update the affected authoritative contract for every public behavior change. Update both RPC contract files only when a shared model, command, or event surface is affected. Update architecture, Architecture Decision Record, standards, and this index when a boundary, phase, or migration status changes.
- Keep headings concrete and maintainable. Avoid marketing language and unrelated comparisons.
- Do not introduce names, branding, identifiers, copied text, or copied code from other projects. References may inform expected behavior only; implementation and documentation must be original.
- Do not mention unrelated products anywhere in this documentation set.
