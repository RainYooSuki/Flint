# Flint Documentation

This directory is the maintained documentation set for Flint. It separates current contracts and operational guidance from the accepted target architecture that is still being migrated.

## Status note

The application currently ships as a **modular monolith**:

- the Rust backend is one package rooted at `src-tauri`;
- commands are registered statically in `src-tauri/src/lib.rs`;
- provider-specific branches still exist inside the shared mod and loader modules;
- the React frontend uses a large app composition surface and hard-coded navigation.

The target Cargo workspace, `flint-core`, feature/provider API crates, feature modules, backend registries, provider contexts, lifecycle coordinator, frontend contribution registry, and `scripts/check-architecture.mjs` are **planned and not yet implemented**. See [`ARCHITECTURE.md`](ARCHITECTURE.md) for the implemented/planned distinction and [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) for the phase applicability matrix.
Canonical ownership: [`ARCHITECTURE.md`](ARCHITECTURE.md) is the detailed source for boundaries, provider/registry semantics, lifecycle, frontend contributions, and phases; [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) owns process/checklists/concise summaries; the [`ADR`](adr/0001-modular-core-and-feature-modules.md) records rationale/consequences; this README is the index/source-of-truth map.

## Documentation index

| Document | Purpose | Status |
| --- | --- | --- |
| [`README.md`](README.md) | Documentation index, reading order, and source-of-truth map. | Current |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Current baseline, target layers, dependency rules, versioned provider/capability contracts and contexts, registry/lifecycle/event semantics, frontend contributions, planned architecture checker, and migration phases. | Current design; target implementation pending |
| [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) | Phase applicability/transition matrix plus mandatory before-work, module-admission, provider, security, documentation, review, and delivery standards. | Current policy |
| [`adr/0001-modular-core-and-feature-modules.md`](adr/0001-modular-core-and-feature-modules.md) | Accepted architecture decision, consequences, rejected alternatives, and follow-ups. | Accepted; implementation pending |
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
| How should internal Rust crates, modules, providers, and registries be structured? | [`ARCHITECTURE.md`](ARCHITECTURE.md) | Target structures are explicitly labeled planned. |
| How are duplicates, feature cycles, provider capability/API/version selection, deterministic initialization, startup degradation, `RuntimeFailed`, unavailable commands, and shutdown handled? | [`ARCHITECTURE.md`](ARCHITECTURE.md) | Architecture is canonical for registry/provider/lifecycle mechanics. Failure propagates to all transitive dependents; any affected `Required` feature is fatal, and `Degraded` is valid only while every `Required` feature remains `Ready`. Providers are independent leaves initialized by provider ID; feature cycles are detected separately. |
| How should feature UI appear in navigation, pages, settings, and instance actions? | [`ARCHITECTURE.md`](ARCHITECTURE.md) | Contribution IDs are globally unique across all frontend slots; navigation uses canonical `targetPageId` page IDs. Phase 3 renders the static compiled set; Phase 4 adds active-feature filtering. Feature modules import only their own subtree plus core/shared; cross-feature composition uses typed core/shell contracts or contribution slots. |
| May I add a module, change a contract, or choose a dependency direction now? | [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) | Use the phase applicability matrix for the compliant pre-phase path; target-only requirements are explicitly gated. |
| How are dependency boundaries enforced? | [`ARCHITECTURE.md`](ARCHITECTURE.md) and [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) | A planned phase-aware `scripts/check-architecture.mjs` uses cargo metadata, Rust source/import checks, and TypeScript compiler API/AST in CI. The AST checker forbids all cross-feature imports, including public entries. Cargo and TypeScript do not inherently enforce arbitrary allowlists; the checker is not implemented yet. |
| How is review and completion handled? | [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) | Independent logic review, no reviewer-run local build/test, CI authority, architecture-checker gate once established, local commit only unless push is requested. |
| How is an external account or mod-catalog integration configured, and where is local data stored? | [`INTEGRATIONS.md`](INTEGRATIONS.md) | Do not move secrets or storage paths based on this index alone. |
| Why was the modular architecture chosen, and what alternatives were rejected? | [`adr/0001-modular-core-and-feature-modules.md`](adr/0001-modular-core-and-feature-modules.md) | The ADR records rationale; it does not replace current contracts. |
| What was the last recorded quality/readiness snapshot? | [`CODE_REVIEW.md`](CODE_REVIEW.md) | Do not treat it as evidence for a later or different commit. |

The repository source remains the evidence of what is implemented. A mismatch between current source and a current contract is a defect to resolve; a planned architecture document does not make an unimplemented path real.

## Reading order

1. [`ARCHITECTURE.md`](ARCHITECTURE.md): understand the current baseline, target layers, provider/lifecycle/event semantics, planned architecture checker, and migration sequence.
2. [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md): before starting work, use the phase applicability matrix and compliant pre-phase path, then apply the implementation, security, checklist, review, and delivery rules.
3. [`adr/0001-modular-core-and-feature-modules.md`](adr/0001-modular-core-and-feature-modules.md): understand why the architecture was accepted and which alternatives were rejected.
4. Read [`API_CONTRACT.md`](API_CONTRACT.md) and [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md) for the area being changed.
5. Read [`INTEGRATIONS.md`](INTEGRATIONS.md) for setup, local storage, privacy, and selected-instance behavior.
6. Use [`CODE_REVIEW.md`](CODE_REVIEW.md) only as a historical readiness snapshot, not as current CI evidence.

## Documentation rules

- Label behavior as **Implemented**, **Planned**, or **Deferred**, and identify the migration phase for target-only rules. Never present a target crate, module, registry, provider, context, composition root, architecture checker, folder, or workflow as existing before it does.
- Use only relative Markdown links that resolve inside `docs/`. Do not link to another checkout, an external site, or a repository file outside the documentation tree.
- Update the affected authoritative contract for every public behavior change. Update both RPC contract files only when a shared model, command, or event surface is affected. Update architecture, ADR, standards, and this index when a boundary, phase, or migration status changes.
- Keep headings concrete and maintainable. Avoid marketing language and unrelated comparisons.
- Do not introduce names, branding, identifiers, copied text, or copied code from other projects. References may inform expected behavior only; implementation and documentation must be original.
- Do not mention unrelated products anywhere in this documentation set.
