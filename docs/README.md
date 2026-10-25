# Flint Documentation

This directory is the maintained documentation set for Flint. It separates current contracts and operational guidance from the accepted architecture decision and explicitly deferred future extensions.

## Status note

The application is a **compile-time modular monolith** with the migration implemented through Phase 5 in the working tree. GitHub Actions verification is pending; this documentation does not claim a green hosted run.

- Rust uses the nineteen-package Cargo workspace rooted at `src-tauri`: the host, `flint-core`, `flint-feature-api`, `flint-provider-api`, seven feature crates, two mod provider crates, and six loader provider crates. One `src-tauri/Cargo.lock` supplies the workspace lockfile.
- `ProviderRuntimeState` in `src-tauri/src/provider_composition.rs` owns the actual immutable mod and loader registries and implements the feature provider catalog. Provider-backed commands, lifecycle availability, provider loss, and shutdown all use that same state. `feature_host.rs` composes seven explicit features; `lib.rs` publishes `get_host_availability`, gates all 28 feature commands, and invokes idempotent shutdown on `RunEvent::Exit`.
- `flint-core` owns shared path/network/error policy and typed network, workspace/archive, process/Java, progress, logging, deadline, and cancellation ports. The typed mod/loader contracts live in `flint-provider-api`; all eight provider leaves consume those contracts and ports.
- The frontend registry supports pages, navigation, settings sections, instance actions, and overlays. The accounts dialog is a registry overlay rather than a shell-specific export. Tauri package access is confined to the exact five adapters for invoke, events, dialogs, opener, and window; `main.tsx` creates `HostServices`, and `AppPageContext` injects it into features. Shared request-generation/event-cleanup hooks protect stale and post-unmount work.
- Mod search accepts only the documented `fabric`, `forge`, `neoforge`, and `quilt` loader contexts. Provider-influenced progress messages are redacted, control-normalized, and bounded before publication. `LaunchResult.command` display is redacted; execution still receives the internal arguments.
- `scripts/check-architecture.mjs`, `scripts/test-architecture-checker.mjs`, and `scripts/test-frontend-contracts.mjs` are implemented. The release `prepare` job runs the contract tests, then `node scripts/check-architecture.mjs --phase 5`. The strict gate covers exact Tauri adapters/importers, overlays, stale-request/shim rules, the exact nineteen-package Cargo workspace and source roots, internal/external dependency allowlists and cycles, typed provider ports, shared actual-registry provider lifecycle state, the sole `ModSource`/`LoaderKind` bindings, and the 30-command/28-feature-command map. GitHub Actions is the final authority.
- Existing command names, arguments, success results, event names, and DTO shapes are preserved. The additive availability command, unavailable-feature gating, bounded/sanitized progress text, redacted launch-command display, and strict mod-loader validation are documented in the authoritative contracts.

Canonical ownership: [`ARCHITECTURE.md`](ARCHITECTURE.md) is the detailed source for boundaries, provider/registry semantics, lifecycle, frontend contributions, and phases; [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) owns process, checklists, concise summaries, and review/delivery rules; the [Architecture Decision Record](architecture-decision-records/0001-modular-core-and-feature-modules.md) documents rationale and consequences; this README is the documentation index and source-of-truth map.

## Documentation index

| Document | Purpose | Status |
| --- | --- | --- |
| [`README.md`](README.md) | Documentation index, reading order, and source-of-truth map. | Current |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Canonical current baseline, phase-status table, layer/provider/lifecycle rules, frontend contribution/adapters, strict Phase 5 checker, and acceptance gates. | Current design; migration implemented through Phase 5 in the working tree; hosted CI pending |
| [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) | Mandatory implementation, provider/frontend, security, documentation, review, and delivery rules with the completed phase matrix. | Current policy |
| [`architecture-decision-records/0001-modular-core-and-feature-modules.md`](architecture-decision-records/0001-modular-core-and-feature-modules.md) | Accepted architecture decision, consequences, rejected alternatives, and completed migration follow-ups. | Accepted; implemented through Phase 5 in the working tree; CI pending |
| [`API_CONTRACT.md`](API_CONTRACT.md) | Base frontend/backend commands, host availability/gating semantics, DTOs, events, and behavioral rules. | Current RPC source of truth |
| [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md) | Extended account, loader, mod, and save commands, DTOs, events, UI expectations, and strict mod-loader values. | Current RPC source of truth |
| [`INTEGRATIONS.md`](INTEGRATIONS.md) | External account/mod-catalog setup, local storage, privacy, and current-instance behavior. | Current operational guidance |
| [`CODE_REVIEW.md`](CODE_REVIEW.md) | Historical quality/readiness snapshot. | Historical only; not current CI evidence |

## Source-of-truth map

| Question | Authoritative document | Notes |
| --- | --- | --- |
| What is the exact command name, argument shape, result shape, or DTO field? | The applicable section of [`API_CONTRACT.md`](API_CONTRACT.md) or [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md) | The applicable contract wins over internal architecture or UI naming. Update both only when the surface appears in both. |
| What behavior must a command or feature preserve? | The applicable section of [`API_CONTRACT.md`](API_CONTRACT.md) or [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md) | Behavioral rules and UI expectations are part of the contract. Update both only for a shared model, command, or event surface. |
| What event name and payload are public, and what are their failure/redaction semantics? | The applicable section of [`API_CONTRACT.md`](API_CONTRACT.md) or [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md), plus [`ARCHITECTURE.md`](ARCHITECTURE.md) | Command results and committed state are authoritative; default progress events are best-effort. Provider-influenced progress text is sanitized and bounded. |
| How should Rust crates, modules, providers, registries, and lifecycle/availability behave? | [`ARCHITECTURE.md`](ARCHITECTURE.md) | Canonical. `ProviderRuntimeState` is the single actual-registry provider lifecycle/catalog source; `flint-feature-api` and `feature_host.rs` own deterministic feature lifecycle, gating, availability, and shutdown semantics. |
| How are duplicates, feature cycles, provider selection, startup degradation, `RuntimeFailed`, unavailable commands, and shutdown handled? | [`ARCHITECTURE.md`](ARCHITECTURE.md) | Failure propagates through all transitive dependents. Any affected `Required` feature is fatal (`StartupFailed` or `RuntimeFailed` -> `Stopping` -> `Stopped`/exit); `Degraded` is optional-only and valid only while every `Required` feature remains `Ready`. Providers are independent leaves. |
| How should feature UI appear in navigation, pages, settings sections, instance actions, and overlays? | [`ARCHITECTURE.md`](ARCHITECTURE.md) | All five slots are registry-driven and availability-filtered. Feature modules receive `HostServices` through `AppPageContext`; Tauri imports are confined to the exact five typed adapters. |
| May I add a module, change a contract, or choose a dependency direction now? | [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) | Follow the completed phase matrix and current strict rules. Completed migration structures are present; deferred future extensions require a separate decision where applicable. |
| How are dependency boundaries and Phase 5 hardening enforced? | [`ARCHITECTURE.md`](ARCHITECTURE.md) and [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) | The release `prepare` job runs architecture/frontend contract tests and `node scripts/check-architecture.mjs --phase 5`. It enforces exact frontend adapters/importers and shims, Cargo metadata/source/dependency/cycle rules, typed provider ports, shared provider runtime state, loader composition, stale-request contracts, overlays, exact allowlists, and the command gate map. |
| How is review and completion handled? | [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md) | Independent logic review; CI is the executable verification authority; local reviewer commands are not evidence; no push is performed unless requested. |
| How is an external account or mod-catalog integration configured, and where is local data stored? | [`INTEGRATIONS.md`](INTEGRATIONS.md) | Do not move secrets or storage paths based on this index alone. |
| Why was the modular architecture chosen, and what alternatives were rejected? | [`architecture-decision-records/0001-modular-core-and-feature-modules.md`](architecture-decision-records/0001-modular-core-and-feature-modules.md) | The Architecture Decision Record documents rationale and consequences; it does not replace current contracts. |
| What was the last recorded quality/readiness snapshot? | [`CODE_REVIEW.md`](CODE_REVIEW.md) | Historical only. It is not evidence for the current working tree or Phase 5 CI status. |

The repository source remains the evidence of what is implemented. A mismatch between current source and a current contract is a defect to resolve.

## Reading order

1. [`ARCHITECTURE.md`](ARCHITECTURE.md): read the canonical phase-status table, current baseline, layer/provider/lifecycle rules, frontend contribution/adapters, strict Phase 5 checker, and acceptance gates.
2. [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md): apply the completed phase matrix and current implementation, security, review, and delivery rules.
3. [`architecture-decision-records/0001-modular-core-and-feature-modules.md`](architecture-decision-records/0001-modular-core-and-feature-modules.md): understand why the architecture was accepted and which alternatives were rejected.
4. Read [`API_CONTRACT.md`](API_CONTRACT.md) and [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md) before changing public commands, events, DTOs, or user-visible behavior.
5. Read [`INTEGRATIONS.md`](INTEGRATIONS.md) for setup, local storage, privacy, and selected-instance behavior.
6. Use [`CODE_REVIEW.md`](CODE_REVIEW.md) only as a historical snapshot, never as current CI evidence.

## Documentation rules

- Label current behavior **Implemented** and reserve **Planned** or **Deferred** for genuinely future extensions. Do not describe completed Phase 5 adapters, overlays, provider lifecycle, archive/redaction/stale-request hardening, or the `--phase 5` gate as pending.
- Use only relative Markdown links that resolve inside `docs/`. Do not link to another checkout, an external site, or a repository file outside the documentation tree.
- Update the affected authoritative contract for every public behavior change. Update both RPC contract files only when a shared model, command, or event surface is affected. Update architecture, the Architecture Decision Record, standards, and this index when a boundary, phase, or verification status changes.
- Do not claim hosted CI is green without a recorded successful GitHub Actions result.
- Keep headings concrete and maintainable. Avoid marketing language and unrelated comparisons.
- Do not introduce names, branding, identifiers, copied text, or copied code from other projects.
