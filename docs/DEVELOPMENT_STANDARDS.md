# Development Standards

## Status and scope

These standards are mandatory when applicable to a change. General contract, documentation, security, and quality rules apply immediately. Target-only structure and composition requirements are gated by the phase applicability matrix below; do not require target crates, registries, or contribution roots before their migration phase introduces them.

The modular architecture in [`ARCHITECTURE.md`](ARCHITECTURE.md) is **partially implemented**. The Phase 4 fourteen-crate workspace contains `flint-core`/`flint-feature-api`/`flint-provider-api`, the extracted settings/saves feature crates, two extracted mod provider crates, and six extracted loader provider crates; shared core policy/typed-port contracts and the host feature lifecycle/availability milestone are implemented. `flint-feature-api` implements the framework-neutral feature registry/lifecycle coordinator, and `feature_host.rs` composes/starts it, publishes `get_host_availability`, gates the 28 feature commands, filters frontend contributions, and shuts down on exit. Typed loader host adoption is implemented in `src-tauri/src/loaders.rs`; the settings and saves feature crates are implemented, while extraction of the remaining five feature crates, full provider runtime lifecycle, full 19-crate extraction, and later-phase checker gates remain pending. The typed mod/loader contracts in `flint-provider-api` and generic Minecraft core support in `flint-core` are implemented. Treat a path as real only when it is present in the repository and the architecture document labels that phase/milestone implemented.

The system remains a modular monolith until the migration phases in the architecture document are completed. There is no runtime plugin discovery, arbitrary native loading, remote UI loading, or dynamic command registration in the approved scope.

### Current implementation status

The canonical phase-status table is [`ARCHITECTURE.md`](ARCHITECTURE.md#canonical-migration-phase-status); do not duplicate phase-status details here.

Current summary: the typed mod and loader provider APIs, two extracted CurseForge/Modrinth implementation crates, six extracted Fabric/Quilt/Forge/NeoForge/LiteLoader/OptiFine implementation crates, two extracted settings/saves feature crates, and two host-composed immutable provider registries are implemented at milestone level. Mod and loader providers use core ports; loader host adoption builds typed contexts and preserves the 75/600/600 split through the sole `LoaderKind` adapter in `loaders.rs`. The settings and saves feature crates are implemented; extraction of the remaining five feature crates, full provider runtime lifecycle, full typed-port checker acceptance, the full 19-crate extraction, the Phase 4 Cargo-metadata gate, and Phase 5 remain pending. Phase 3 frontend contribution slots are implemented and mounted, and the checker gate is active at `--phase 3`. Phase 4 is **in progress** with the fourteen-crate workspace, shared core policy/typed-port contracts, two immutable provider registries, and host lifecycle/availability milestone implemented.

Future work follows this partial-Phase 1/2, implemented-Phase 3, and in-progress Phase 4 boundary with the host lifecycle/availability milestone implemented: use the implemented provider registries for mod/loader changes, use the implemented frontend contribution slots and availability snapshot for feature UI, and do not treat remaining Phase 4 extraction/full-port work or later checker gates as active until explicitly advanced.
Canonical ownership: [ARCHITECTURE.md](ARCHITECTURE.md) is the detailed source for boundaries, provider/registry semantics, lifecycle, frontend contributions, and phases. This document owns process, checklists, and concise mandatory summaries; it links to the architecture instead of duplicating detailed mechanics. The Architecture Decision Record documents rationale/consequences, and [README.md](README.md) is the index/source-of-truth map.

## Sources of truth and precedence

1. An explicit task instruction governs the immediate work scope, including when a task prohibits commits or limits writable files.
2. The applicable sections of [`API_CONTRACT.md`](API_CONTRACT.md) and [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md) govern externally visible frontend/backend commands, arguments, DTO fields, enum strings, events, and behavioral rules. Update both only when a shared model, command, or event surface is affected.
3. [`ARCHITECTURE.md`](ARCHITECTURE.md) is the canonical detailed source for internal boundaries, dependency direction, provider/registry semantics, lifecycle, frontend contributions, target layout, and migration sequencing. This document provides process, checklists, and concise summaries with links to that detail.
4. This document governs implementation process, quality bars, review, and delivery.
5. [`architecture-decision-records/0001-modular-core-and-feature-modules.md`](architecture-decision-records/0001-modular-core-and-feature-modules.md) records why the architecture decision was made. It does not override the contracts or detailed architecture.
6. [`INTEGRATIONS.md`](INTEGRATIONS.md) records current integration setup and local-storage guidance.
7. [`CODE_REVIEW.md`](CODE_REVIEW.md) is a point-in-time quality and readiness snapshot, not standing proof that a later change has passed anything.

If two sources conflict about a public command, event, DTO, or user-visible rule, stop and resolve the conflict explicitly before implementing. Do not silently choose one and leave the others stale.

## Phase applicability and transition matrix

The target architecture is phase-gated. A rule is not mandatory before the structure it governs exists, but work must always follow a compliant current-phase path. Requirements introduced by an implemented phase milestone apply to new work and migrated code even while full phase acceptance remains pending; after a phase is accepted, the applicable prohibition remains binding.

A **target-only** item is not required before the named phase and must be marked N/A with that phase basis. **Mandatory now** items apply to the current modular monolith. A phase gate is satisfied only when its stated structure/checker exists and its acceptance criteria pass; documentation labels and implementation status must match.

| Rule group | Baseline / transition | Phase 1: ModProvider | Phase 2: LoaderProvider | Phase 3: Frontend registry | Phase 4: Workspace/modules | Phase 5: Hardening |
| --- | --- | --- | --- | --- | --- | --- |
| Contracts, documentation, security, error/file/network rules | Mandatory now. Update the affected authoritative contract for public behavior; both RPC contracts only for a shared model/command/event surface. | Mandatory. | Mandatory. | Mandatory. | Mandatory. | Mandatory; all claims must match implemented behavior. |
| Dependency direction and cycle hygiene | Apply the ownership rules to existing modules: no new cycles, no domain logic added to the Tauri wrapper, no private cross-module reach, and no unrelated host coupling. Full crate allowlists are target-only. | Typed ModProvider API and extracted provider leaves stay inside approved boundaries; no private host import. | Loader pilot follows the same rule. | Frontend features may import only their own subtree plus core/shared; no cross-feature imports, including public entries. | Full crate and TypeScript allowlists become mandatory. | Strict allowlists; no temporary boundary waiver remains. |
| Provider API, versioned capabilities, registry, and typed contexts | New mod/loader provider work starts from `flint-provider-api` and the extracted `providers/flint-mod-provider-*` and `providers/flint-loader-provider-*` crates; no provider work starts in the host. | **Milestone implemented:** typed mod data/request/context contracts and two extracted leaves. Remaining full runtime lifecycle. | **Milestone implemented:** typed loader contracts, six extracted leaves, core ports, exact 75/600/600 split, and injected immutable registry. | Generic provider contracts plus typed mod/loader contracts live in the extracted API crate; all eight provider leaves are extracted with no frontend provider branching. | **In progress:** fourteen-crate workspace, two immutable registries, host typed loader adoption, and host feature lifecycle/availability milestone exist; extraction of the remaining five feature crates and full provider runtime lifecycle remain. | **Pending:** extraction of the remaining five feature crates, full provider runtime lifecycle, strict allowlists/full ports, and Phase 4 Cargo-metadata/Phase 5 gates. |
| FeatureDescriptor, Rust feature registry, startup lifecycle, and Rust composition root | **Implemented milestone:** `flint-feature-api` supplies descriptors, validated registry, lifecycle coordinator/state machine, availability DTO, provider catalog, and gating; `src-tauri/src/feature_host.rs` composes seven explicit features and starts/manages the coordinator. Host startup, availability IPC, frontend filtering, 28-command gating, and exit shutdown are implemented. Loader and mod descriptors/requirements now come from `provider_composition.rs`; settings and saves implementations live in their extracted feature crates. | Typed mod/loader provider registries/API required and consumed by the host milestone. | Provider registries remain; feature lifecycle API is consumed by the host milestone. | Frontend contribution registry and snapshot-driven availability filtering are implemented for pages, navigation, settings sections, and instance actions. | **Pending within in-progress Phase 4:** full provider runtime lifecycle, full 19-crate extraction, full-port checker enforcement, and Phase 5 hardening. | **Pending:** full typed-provider acceptance and complete extraction. |
| Frontend contribution registry and composition root | Prior hard-coded feature shell is superseded by the implemented Phase 3 registry. | Current pattern allowed for not-yet-migrated provider/UI integration. | Current pattern allowed; do not introduce partial slots. | **Implemented:** page/navigation, settings-section, and instance-action contributions, validation, frozen registry, mounted renderers, per-contribution boundaries, and snapshot-driven availability filtering. | **Implemented milestone:** host active-feature snapshot/filtering with fail-closed unavailable/startup-failed behavior. | **Pending:** strict contribution rules and removal of transitional frontend adapter waivers. |
| Tauri command thinness and lifecycle | Keep changed commands thin and preserve current compile-time registration. | **Milestone implemented:** mod commands use an injected registry-selected provider; wrappers remain in `lib.rs`. | **Milestone implemented:** loader commands inject the immutable loader registry and use the sole `LoaderKind` adapter; wrappers remain in `lib.rs` and preserve gates, `spawn_blocking`, and 30 handlers. | Frontend calls remain typed; all contribution slots render statically. Transitional direct-Tauri frontend imports remain waived until later phases. | **Pending:** RuntimeFailed transition, reverse shutdown, and remaining lifecycle hardening. | **Pending:** end-to-end hardening through CI. |
| Architecture checker and CI gate | **Milestone implemented:** phase-aware checker/tests and the release workflow `prepare` gate at `--phase 3` exist. `prepare` installs frontend dependencies, runs checker tests, then runs the checker; all build-matrix jobs depend on `prepare`. The gate enforces exact fourteen-member workspace membership, host feature/loader path dependencies, exact settings/saves and mod/loader provider manifests/dependency allowlists, all extracted feature/provider source leaves, forbidden provider std/process/fs/net/reqwest/Tauri patterns, forbidden feature Tauri/host-private imports, both immutable composition registries only in `provider_composition.rs`, exact six loader descriptors/requirements, and the sole `LoaderKind` adapter in `loaders.rs`. Legacy loader adapter/API alternatives/waivers fail closed. | **Milestone implemented:** typed ModProvider contract, extracted leaves, core-port context, import/selection/dependency/composition boundaries covered by `prepare`. | **Milestone implemented:** typed loader contract, six leaves, typed host adapter, exact registry/descriptor/dependency boundaries covered by `prepare`. | **Implemented:** Phase 3 TypeScript manifest/composition/dynamic-import checks are active in the `prepare` gate. | **Pending:** full 19-crate metadata allowlists and full frontend graph gate; Phase 4 Cargo metadata/full typed-port gates are not active. | **Pending:** full typed-port enforcement and Phase 5 strict allowlists. |

### Mandatory before the target exists

The following are not phase-gated and must be followed now:

- Preserve public command, DTO, enum, event, settings, and user-visible behavior unless an intentional contract change is documented and coordinated.
- Update the authoritative contract for a public behavior change. Update both [`API_CONTRACT.md`](API_CONTRACT.md) and [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md) only when the shared model, command, or event surface is affected.
- Apply security, path/archive/network safety, secret redaction, bounded I/O, and no-panic rules to all changed code.
- Keep changes inside the owning existing module or adapter, avoid new cycles, and do not put business logic in Tauri command wrappers.
- Do not claim the remaining five unextracted feature crates, full provider runtime lifecycle, full 19-crate extraction, the Phase 4 Cargo-metadata gate, or future checker phases exist/are active before implementation. The implemented areas currently claimed are the `flint-core`/`flint-feature-api`/`flint-provider-api` foundation, the extracted settings/saves feature crates, typed mod/loader contracts and all eight provider implementation crates, two immutable host composition registries, typed host loader adoption, shared core policy/typed-port contracts, the framework-neutral feature lifecycle contract/registry/coordinator, and the host lifecycle/availability IPC/gating/filtering milestone.
- Do not introduce copied names, branding, identifiers, prose, code, or unrelated product references.

### Current transition path (Phase 1/2 milestones implemented; full phases partial; Phase 3 implemented; Phase 4 typed provider/host lifecycle milestone implemented; full phase in progress)

A change in the current host-lifecycle/availability and Phase 3 frontend-registry state is compliant when it:

1. Preserves the current contract and places behavior in the existing owner module with the narrowest necessary surface.
2. Avoids creating an orphan partial target structure. The relevant phase introduces the contract/registry/crate rather than a pre-phase file tree with no lifecycle owner.
3. Uses the implemented provider registries for mod/loader work. Do not reintroduce provider-specific call-site branches or add a parallel dispatch path.
4. Uses the implemented frontend contribution registry for new pages, navigation, settings sections, and instance actions, and filters them through the validated host availability snapshot. Register UI through the explicit composition root and mount it through the typed shell/context renderer rather than adding a parallel hard-coded slot.
5. Records target-only checklist items as `N/A` with the phase basis, rather than falsely claiming the target exists or silently omitting the rule.
6. Uses the implemented checker/tests through the release workflow `prepare` gate at `--phase 3`, including the static exact workspace and host 30-command/28-feature-command gate map. Cargo metadata and full typed-port checker behavior are not claimed active until that gate is advanced explicitly.


## Before-work checklist

- [ ] Confirm the task targets the correct repository and checkout. Do not modify another checkout or carry assumptions from a different working tree.
- [ ] Read the relevant contract sections, the architecture boundary rules, and the phase applicability matrix before editing.
- [ ] Inspect the current implementation rather than assuming the target structure already exists.
- [ ] Write down the user-visible behavior, compatibility constraints, and non-goals of the change.
- [ ] Classify the work and identify its phase: core service, provider, feature module, adapter, shared utility, or documentation-only.
- [ ] Identify the exact writable scope and list files/directories expected to change.
- [ ] Identify every contract, DTO, command, event, settings field, or UI contribution affected, and which authoritative contract must change.
- [ ] Draw the intended dependency direction and check it against the current phase and forbidden/cyclic imports.
- [ ] Identify filesystem, network, archive, process, secret, and path-safety risks.
- [ ] Decide lifecycle/failure behavior: affected-set propagation, effective criticality (any affected Required is fatal), Required-Ready degraded invariant, dependency skipping, unavailable commands, diagnostic StartupFailed cleanup, partial success, cleanup/shutdown, event semantics, retry, cancellation, and stale UI requests.
- [ ] Identify the CI checks needed for the change, including the implemented architecture checker at the currently active `--phase 3` gate. Do not substitute reviewer-run local commands for CI.
- [ ] Define an independent logic reviewer before handoff.
- [ ] Decide whether the work is expected to be committed locally; do not push unless requested.

## Boundary selection

Choose the narrowest boundary that matches ownership. Do not create a new crate or folder merely to make a file smaller.

| Candidate | Use it when | Required characteristics | Common mistakes |
| --- | --- | --- | --- |
| **Core service** | Two or more features need the same stable policy or primitive, such as path safety, bounded network access, task scheduling, or shared error mapping. | Feature-agnostic; no host or module imports; small stable API; deterministic failure behavior. | Putting feature names, provider branches, UI language, or all DTOs into core. |
| **Feature/provider API crate** | A contract crosses a crate boundary and needs independent versioning. | Contains descriptors, IDs, traits, ports, and contract types only; minimal dependencies; semver discipline. | Housing implementation logic, concrete providers, or Tauri-specific types. |
| **Feature module** | The code owns a cohesive user-facing capability, its orchestration, state, contributions, and failure behavior. | Stable ID/version; explicit initialization; declared requirements; no host import; no private cross-module imports. | A catch-all "utils" module or a module that only wraps two lines of another module. |
| **Provider** | Behavior varies by an external source, catalog, loader, format, or protocol behind one contract. | Stable provider ID; provider API version; normalized public DTO mapping; shared safety policy; no UI/host imports. | Provider-specific `match` branches spread through commands or services. |
| **Tauri adapter** | Code translates Tauri/framework types, command parameters, events, dialogs, or process/window handles. | Thin, declarative mapping; one call into a feature/application service; error mapping at the boundary. | Filesystem/network/business logic in the command wrapper. |
| **Frontend core/shared** | Code is generic across features: API transport, event cleanup, registry primitives, generic UI, pure helpers. | No concrete feature imports; no product-specific workflow; stable typed API. | A shared component that knows one feature's business rules. |
| **Frontend feature module** | The code provides UI, local state, contribution declarations, and typed API use for one capability. | Owns its state; imports only frontend core/shared and its own files; exports only the module surface needed by the app composition root. | Importing another feature module, including its public entry; sharing a store; or treating a feature public entry as a cross-feature API. |

If a responsibility has no clear owner, stop and define the boundary before writing code. Do not place it in `App.tsx`, `lib.rs`, a provider implementation, or a generic "common" folder by default.

## Contract-change gate

Any change that adds, removes, renames, or changes the meaning of an externally visible command, argument, result field, event, enum value, or persisted settings field is a contract change.

A contract change requires all of the following in the same scoped work:

- [ ] An update to the affected authoritative contract for every public behavior change. Update both RPC contract files only when the changed shared model, command, or event surface is documented in both.
- [ ] Matching Rust DTO/request/response definitions with the required serde casing.
- [ ] Matching TypeScript types.
- [ ] Matching command/event client wrappers and call sites.
- [ ] Backward-compatible loading/default behavior where persistence is involved.
- [ ] A migration or compatibility note for existing user data and commands.
- [ ] Tests or CI checks covering both serialization shape and behavioral behavior.
- [ ] Review of secret exposure, error text, and command/event logging.

Internal refactors must preserve the public contract. A passing internal compilation is not permission to rename an external field or event. "Public behavior changed" always requires the affected authoritative contract to be updated; adding an unrelated second contract file is not required when the surface belongs to only one contract.

## Adding a feature or module

Current status: the typed mod and loader provider contracts/APIs, two extracted settings/saves feature crates, two extracted mod implementation crates, six extracted loader implementation crates, two host composition registries, and generic Minecraft core helpers are implemented at milestone level. Loader host adoption builds typed contexts through the sole `LoaderKind` adapter in `src-tauri/src/loaders.rs`; full provider runtime lifecycle, extraction of the remaining five feature crates, full typed-port/Cargo-metadata checker acceptance, and Phase 5 remain pending. New mod/loader provider work belongs in the extracted API/provider crates; payload mapping and safe I/O stay there. Frontend pages, navigation, settings sections, and instance actions belong in feature modules and the explicit `src/app/composition.ts` registry, filtered through the validated availability snapshot. Use the `flint-feature-api` descriptor/registration/initializer contracts for subsequent host feature work.

1. **Define ownership.** Choose one cohesive responsibility and record the intended boundary: core service, feature module, provider, adapter, or shared utility. Do not use a new crate or folder as a substitute for an ownership decision.
2. **Declare the descriptor.** For a Phase 4 Rust feature, use the canonical Rust feature contract: stable ID, `feature_version`, `api_version`, `startup_policy`, required features, versioned provider requirements, one initialization entry point, and backend contributions. For the implemented Phase 3 frontend contribution registry, use the current frontend `FeatureDescriptor` contract. Registration data is side-effect free.
3. **Initialize safely.** Validate prerequisites before side effects; receive dependencies through typed contexts; start no owned worker until initialization succeeds. On failure, clean every partial resource before returning the error. Follow the canonical startup/runtime states and reverse shutdown order.
4. **Register contributions.** Use the approved registry slot, stable feature-prefixed IDs, declared requirements, and deterministic order. Reject duplicates instead of overriding and do not mutate a frozen registry. After the relevant registry phase, do not add a central hard-coded feature/provider switch.
5. **Wire boundaries.** Add thin host/command/API adapters where public behavior changes, explicitly register commands in the compile-time handler, and add frontend contributions through the composition root. No feature imports another frontend feature module or its public entry; cross-feature use goes through typed core/shell contracts or contribution slots.
6. **Update the contract.** Update the affected authoritative contract for public behavior. Update both RPC contract files only for a shared model, command, or event surface.
7. **Verify the edges.** Add CI-covered coverage for descriptor/provider compatibility, deterministic selection/provider-ID order, feature dependency cycles, duplicate/missing/ambiguous cases, startup/runtime failures, serialization/events, security limits, and any UI availability rules applicable to the phase.

## Provider implementation rules

[`ARCHITECTURE.md`](ARCHITECTURE.md) is canonical for the detailed provider descriptor, compatibility/selection predicate, context ports, lifecycle, and port-enforcement mechanics. The following implementation rules are mandatory summaries.

- The typed mod and loader provider APIs, all eight provider implementation-crate extractions, and the settings/saves feature crates are implemented at milestone level; typed host loader adoption, one immutable mod registry, and one immutable loader registry are implemented. Full provider runtime lifecycle, extraction of the remaining five feature crates, full typed-port/Cargo-metadata checker acceptance, and Phase 5 remain pending. New mod/loader provider work starts from the implemented contracts and extracted `providers/flint-mod-provider-*` and `providers/flint-loader-provider-*` crates. Do not reintroduce hard-coded provider branches or parallel dispatch.
- Define provider behavior in the provider contract. The generic registry and typed mod/loader contracts live in `src-tauri/crates/flint-provider-api`, with all eight implementations in the extracted provider crates. Features depend on the traits, not concrete providers. `src-tauri/src/loaders.rs` is only the sole host `LoaderKind` adapter/orchestrator, not a provider implementation.
- Neither mod nor loader provider crates use transitional host parent-helper imports or waivers. The six loader leaves use shared core ports, and the Phase 3 checker fails closed on legacy loader adapter/API alternatives, concrete provider imports, raw I/O, and missing exact six-leaf/descriptor/dependency boundaries. Full Phase 4 Cargo-metadata and Phase 5 runtime/provider-port acceptance remain later gates.
- Provider descriptors expose `provider_version`, `provider_api`, and capabilities. Each capability binds `{ id, api_version, selection_keys }`; selection keys are not a separate provider-level list.
- Providers are independent leaves. Do not declare provider-to-provider dependencies, build a provider dependency graph, or run provider-cycle detection. Cross-provider coordination belongs in core services or feature orchestration.
- Provider registries sort and validate provider leaves deterministically by provider ID after descriptor/capability validation. Full provider runtime lifecycle/initialization state remains pending Phase 4; do not use `HashMap` iteration, filesystem discovery, or registration order.
- Select providers by the full feature requirement: host provider-API compatibility, capability ID and capability-API compatibility, provider-version constraint, optional selection key bound to that capability, and active provider state. Exactly one provider must match; zero is unavailable and multiple is ambiguous. No fallback. Mod and loader selection each reuse one immutable registry built at host composition and injected through managed state. The feature host catalog tracks provider availability/unavailability metadata for feature startup/gating, while full mutable provider health/runtime lifecycle remains pending Phase 4.
- Map public enum values such as `ModSource` and `LoaderKind` to stable selection keys in one adapter location. For the implemented selection milestones, do not add call-site `match` branches in commands, UI-facing services, or shared helpers.
- Keep provider payloads private and normalize to public DTOs at the boundary. Provider IDs, versions, capabilities, and raw payloads do not leak into public DTOs unless a contract change introduces them.
- Mod and loader providers receive typed contexts with `NetworkPort`, safe workspace/path, Java-runner where applicable, progress/logging, timeout, deadline, and cancellation values; mod providers additionally receive an optional redacted scoped credential. The six loader leaves and host list/install contexts are implemented at milestone level. Neither API context exposes a Tauri handle, raw settings path, unrestricted filesystem root, or unrestricted process handle.
- The active `scripts/check-architecture.mjs` at `--phase 3` enforces frontend manifest/composition/import boundaries plus module/provider import and selection boundaries, explicitly forbidden std/process/fs/net/reqwest/Tauri patterns in all extracted provider leaves, exact mod and loader provider manifests/dependency/source/composition checks, exact six loader descriptors/requirements, two immutable composition registries only in `provider_composition.rs`, the sole `LoaderKind` adapter in `src-tauri/src/loaders.rs`, one `ModSource` binding, and exact allowlists. Legacy loader adapter/API alternatives and waivers fail closed. Mod and loader providers use the shared typed network/path/process/progress/logging/deadline/cancellation contracts. The Phase 4 Cargo-metadata gate and Phase 5 remain later work; temporary exceptions must be explicit, owned, scoped, and phase-expiring.
- Mod providers never execute a downloaded mod archive. Loader providers run only trusted installer flows with bounded processes, captured/redacted output, timeouts, and safe archive policy.
- Do not fall back to another provider silently. Provider-specific credentials, headers, and error bodies must not leak into logs, DTOs, events, or notices.
- Provider progress publication uses the shared progress port. Command results and committed durable state are authoritative; default progress events are best-effort. If a contract marks an event as required protocol, delivery failure must return an error or typed partial-success without rolling back committed state.
- Provider behavior must remain deterministic for the same inputs except for explicitly documented external-service variability. Do not make the registry a generic reflection or runtime plugin mechanism.

## Frontend contribution and state rules

[`ARCHITECTURE.md`](ARCHITECTURE.md) is canonical for contribution slots, registry validation, state ownership, and static/lazy loading mechanics. These are the mandatory implementation summaries.

### Contributions

- Phase 3 contribution registry and explicit static composition root are implemented. Pages, navigation, settings sections, and instance actions render through the compiled feature set, and the Phase 4 milestone now filters them from the host availability snapshot. The settings feature mounts five sections; the launch feature mounts a launch-navigation action. Unready/unavailable features are hidden and command execution is gated by the owning feature ID.
- New feature UI must use typed contributions from the explicit composition root; the shell must not keep a second hard-coded page/navigation/settings/instance-action list. Settings sections use the shared draft and validators; instance actions use the typed action context.
- Frontend contribution IDs are feature-prefixed and globally unique across all frontend slots. A page contribution ID is canonical contract data; navigation uses `targetPageId`.
- The runtime registry rejects duplicate contribution IDs across any slot and unknown/ambiguous `targetPageId` targets before rendering. Order by explicit `order`, then stable ID.
- A frontend feature may import only its own subtree plus frontend core/shared contracts. It must not import another feature module, including that feature public entry. Cross-feature composition/state/actions use typed core contracts, app-shell services/contexts, or contribution slots.
- Static imports and one-string-literal local `lazy(() => import("./literal-path"))` are allowed only inside the importing feature subtree or an explicitly allowlisted local chunk. Variable/template specifiers, absolute paths, file/HTTP URLs, runtime string loaders, remote code, `eval`, `new Function`, and module federation are prohibited.
- The implemented TypeScript AST checker rules enforce the strict target import policy: a feature may import only its own subtree plus frontend core/shared, and all cross-feature imports including public entries are violations. These Phase 3 checks are active in the release workflow `prepare` gate at `--phase 3`.
- Contribution registration is pure declaration. Do not perform I/O, subscriptions, timers, or store creation at module evaluation. Freeze the registry before render.
- Page contributions and mounted settings sections are wrapped by the shared core contribution error boundary. Contribution loaders are component/lazy-function references; the registry cannot runtime-prove an arbitrary string path.

### State ownership
Current Phase 3 ownership: `src/app/App.tsx` owns shared settings/account/instance/query/action state and exposes `AppPageContext`, including frozen contribution lists, to page modules. Feature modules own local presentation state; registered settings sections consume the shared settings draft and callbacks, and registered instance actions consume the typed action context. The rules below are the target ownership model.

- App/shell state is limited to cross-feature presentation and selection: active page contribution ID, selected instance, effective game directory, global notices, registry, and typed shell services.
- Each feature owns its query results, non-shared drafts, progress, modals, and action busy/error state.
- The settings owner owns the single saved/draft settings workflow and persistence. The accounts owner owns account state/mutations. Other features use published core/shell contracts, never another feature module.
- No feature may import another feature module, its store, or its public entry. Cross-feature state/actions move through typed core contracts or app-shell contexts.
- Do not introduce a mutable global singleton to avoid explicit dependency passing.

### API and event use
Current Phase 3 still has reviewed transitional direct Tauri event/dialog/opener imports in the shell, accounts, saves, and versions/downloads modules, tracked by Phase 5 checker waivers. The active release workflow `prepare` gate is `--phase 3`; do not claim full typed API/event adapter migration. The target rules below remain mandatory for new work and later cleanup.

- Call commands through the shared typed API client; keep TypeScript request/response types identical to the serialized contract.
- Type event payloads, keep literal names contract-compatible, and clean up every listener on rejected and resolved registration.
- Use request IDs or equivalent guards so stale completions cannot overwrite newer state.
- Command results and committed durable state are authoritative. Default progress events are best-effort; delivery failure is logged and does not fail an otherwise successful command or roll back committed state. If a contract marks an event as required protocol, delivery failure returns an error or typed partial success without rolling back committed state.
- Starting in Phase 4, contributions from non-ready features are not mounted and dependent commands return unavailable-feature errors instead of falling back. Phase 3 renders the static compiled set.

## Rust conventions

- Use `snake_case` for modules, functions, methods, variables, and crate-private helpers.
- Use `UpperCamelCase` for types and traits; use `SCREAMING_SNAKE_CASE` for constants.
- Use lowercase stable identifiers for feature/provider IDs and contract enum values.
- Keep DTO fields in the casing required by serde. Public serialized DTOs should use `#[serde(rename_all = "camelCase")]` where the contract requires it.
- Keep transport DTOs separate from rich domain/runtime types when their invariants differ. Map explicitly at the adapter boundary.
- Keep `pub` minimal. Prefer private items and narrow `pub(crate)` exposure; expose only what the crate boundary contract requires.
- Do not expose internal provider payloads, mutable registry internals, or Tauri types from a reusable crate.
- Return `Result` at fallible boundaries. Do not use `unwrap`, `expect`, unchecked indexing, or `unreachable!` on untrusted network, filesystem, archive, process, or frontend data.
- `unwrap`/`expect` in tests or for a genuinely proven invariant must be accompanied by a clear justification and must not become a shortcut around user-controlled failure.
- Add context to errors: operation, safe resource identifier, and cause. Do not include secrets or full sensitive payloads.
- Use `std::fmt::Display` / typed internal errors where they improve mapping; keep the public command error contract consistent with the documentation.
- Do not create background threads with ambiguous ownership. A feature that owns a worker must also own its stop/shutdown semantics.
- Do not hold synchronization guards across blocking calls, process waits, or await points.
- Treat filesystem and archive operations as security-sensitive. Validate components before joining, reject escapes/symlinks where required, and use atomic replacement for persisted state.
- From Phase 1 onward, new provider implementations must not import Tauri or receive `AppHandle`; provider contexts carry framework-neutral ports. The extracted `flint-core`, `flint-feature-api`, `flint-provider-api`, `flint-mod-provider-curseforge`, and `flint-mod-provider-modrinth` crates are framework-neutral, and remaining feature/provider implementation crates must not depend on or import Tauri when extracted. Existing host feature modules may retain reviewed current Tauri adapter calls, but new domain/business behavior must remain outside the command wrapper and the phase matrix governs the transition.
- Do not use `#[allow(dead_code)]` to keep abandoned migration code. Remove it or document a concrete follow-up with an owner.

## TypeScript conventions

- Use `PascalCase` for components, types, interfaces, and enums; use `camelCase` for functions, variables, properties, and hooks.
- Serialized DTO properties must match the camelCase wire shape exactly. Do not translate field names ad hoc in components.
- Keep strict typing. Do not introduce `any`; use `unknown` and narrow it at the boundary.
- Prefer `import type` for type-only imports.
- All API calls go through the typed API client. All events use the shared event helpers/constants where available.
- Handle promise rejection explicitly. Do not leave floating promises in effects or handlers unless the rejection is deliberately contained and reported.
- Effects that subscribe, schedule timers, or issue requests must clean up listeners/timers and protect against stale completions.
- Do not import another feature module, including its public entry, or pass another feature internal setters/store across module boundaries. Use typed core contracts or app-shell services/contexts.
- Frontend contribution IDs are stable, feature-prefixed, and globally unique across all slots; page contribution IDs are canonical, and navigation uses `targetPageId`. IDs are never generated from labels, random values, or array positions.
- UI controls must remain keyboard reachable and use deliberate accessible names/roles; feature work must not regress existing accessibility behavior.
- No remote UI execution and no arbitrary dynamic imports. Only one string-literal, relative, bundler-resolvable specifier inside the feature subtree is allowed for local lazy loading; contribution loaders are component/lazy-function references, never runtime string paths or URLs.

## Naming and contract consistency

- Existing command names, event names, DTO field names, enum string values, canonical page contribution IDs, and established section/page behavior remain stable unless a contract change is explicitly approved.
- Current Tauri commands use `snake_case`; frontend wrappers use typed camelCase methods. Do not rename one side independently.
- Event names remain namespaced with the established `flint://` convention unless a contract migration says otherwise. New events need an exact name, payload type, emitter, and cleanup path.
- Feature/provider/internal IDs use the stable lowercase identifier rules; they are not equivalent to display labels or file paths. Feature descriptors use `feature_version`, while provider descriptors use `provider_version` and `provider_api`; capability IDs carry their own API version.
- All Cargo workspace packages share the generated date-based release version. `src-tauri/Cargo.toml` keeps the host `[package] version` equal to `[workspace.package] version`; every non-root member declares `version.workspace = true`; release tooling updates the `Cargo.lock` entry for every local workspace package. Member manifests are not independently versioned, and no workspace package may use an independent `0.x` release version.
- Rust enum values that cross the frontend boundary must serialize exactly as documented. Add a serialization test whenever the enum or serde attributes change.
- Rust DTOs and TypeScript interfaces must change in the same commit/change set. A field rename or public behavior change requires the affected authoritative contract update; update both RPC contracts only when a shared model/command/event surface is affected.
- Additive DTO fields should be optional or defaulted when older persisted data may lack them. Never assume an older settings file contains a new field.
- Keep public API surfaces minimal; every export is a maintenance and versioning promise. Navigation contribution fields use `targetPageId`; do not introduce a second route identifier for the same page.

## Error handling and recovery

- Treat malformed local metadata, provider responses, downloaded archives, and user-selected paths as expected failure modes, not panics.
- Validate early and fail with a user-actionable message. Preserve enough technical context for safe logs.
- Classify failures: invalid input, unsupported capability, unavailable external service, permission/path error, malformed data, timeout, partial operation, and internal defect. Map them consistently at the boundary.
- The framework-neutral Rust feature lifecycle API/registry/coordinator is implemented in `flint-feature-api`, and the Tauri host composes/starts it through `feature_host.rs` with managed state, `get_host_availability`, 28-command gating, and exit shutdown. The canonical target summary remains: failure propagates to the failed feature plus all transitive dependents; any affected `Required` feature takes `StartupFailed` with feature/provider/core cleanup or `RuntimeFailed` -> `Stopping` -> `Stopped`/exit; an optional-only affected set may leave the host `Degraded` only while every `Required` feature remains `Ready`; a `Required` feature must never remain `Ready` with a non-ready dependency/provider. There is no feature-level `Degraded`. The mod provider registry is immutable and built once at host composition, while the loader adapter registry remains operation-scoped; the feature host catalog reuses their real provider metadata while full typed provider runtime lifecycle remains pending.
- A command owned by or dependent on a non-ready feature returns a concise unavailable-feature error. It must not fall back, perform partial work, or access I/O directly.
- Initialization failure is atomic: the feature cleans its own partial resources before returning the error. Successful features shut down in reverse initialization order; shutdown is idempotent and errors are logged without hiding the primary failure.
- Command results and committed durable state are authoritative. Default progress-event delivery is best-effort and does not fail a successful command or roll back committed state. If a contract marks an event as a required protocol step, delivery failure returns an error or typed partial-success result and is never reported as success.
- Do not convert an internal failure into an empty successful result. Empty lists are valid only when the operation legitimately found nothing.
- Do not retry non-idempotent writes blindly. Retry only bounded, idempotent network/download steps under the shared policy.
- Describe partial success explicitly when an operation can leave completed files. Never report transactional atomicity that was not implemented.
- Preserve the primary error when cleanup or shutdown also fails; append cleanup diagnostics without masking the cause.
- Error text must not contain authorization headers, tokens, API keys, passwords, or unredacted launch commands.

## Async, concurrency, and blocking work

- Blocking network, zip/NBT parsing, hashing, process waits, and large file I/O must not run on the Tauri async runtime thread.
- Schedule blocking work through the approved runner at the host boundary. Feature/provider APIs should make blocking behavior explicit.
- Use timeouts and size/depth/count limits for all external and archive operations.
- Do not hold locks across I/O or await. Prefer immutable startup registries and narrowly scoped feature state.
- Cancellation is not automatic. If an operation can be abandoned, define what is cancelled, what remains on disk, and how the UI observes the final state.
- Prevent stale request races with request IDs/generation counters and cleanup guards.
- Do not start background work in a descriptor or module import.
- Feature/provider contexts must carry deadline and cooperative cancellation where operations can be long-running; cancellation must not leave a false success state or silently roll back committed durable state.
- The host adapter alone owns planned Tauri `RunEvent::Exit` and `RuntimeFailed` shutdown coordination; feature/provider code does not import Tauri run events or hold `AppHandle`. States and ordering are canonical in [`ARCHITECTURE.md`](ARCHITECTURE.md), and shutdown must not hold the UI thread indefinitely.

## Security, filesystem, and network safety

### Input and path safety

- Never trust frontend-provided paths, file names, provider IDs, versions, URLs, archive member names, or metadata sizes.
- Normalize paths before validation; reject absolute/traversal components where a relative component is required.
- Join caller-controlled names only through the shared safe-join policy and verify the resolved path remains inside the intended root.
- Reject symlink escapes, duplicate/unsafe archive names, and writes to reserved/temporary paths as required by the operation.
- Do not derive a write path from a display name, provider-provided filename, or project title without sanitization and validation.
- Destructive operations need explicit confirmation at the UI layer and revalidation at the backend. UI confirmation does not replace backend validation.

### Archives and processes

- Enforce archive entry count, per-file size, total uncompressed size, compression ratio, and depth limits.
- Reject path traversal and absolute paths during collection and again while writing.
- Validate archive type before extraction. Do not execute a downloaded mod archive.
- Loader installer execution must use the supplied/validated Java executable, bounded arguments, captured output, timeout, and a log path with redaction.
- Treat subprocess output as untrusted text. Never evaluate or render it as HTML.

### Network

- Require HTTPS for external endpoints and redirects. Validate the final URL/protocol before use.
- Apply connection/read timeouts and bounded body sizes.
- Do not log credentials or full raw responses. Include safe service/status context.
- Provider-specific headers/keys must be attached only to the intended request.
- Do not bypass a failed provider download or safety check with a shell command, browser fallback, or unchecked URL.

### Secrets and the Tauri boundary

- Tokens, refresh tokens, API keys, passwords, and authorization headers must never appear in command results, events, notices, logs, or launch command strings.
- Persist secrets only in the documented local store and with the existing platform permission behavior.
- Validate every command argument again on the backend, regardless of frontend validation.
- Do not expand Tauri capabilities, window permissions, or content-security settings as an unrequested side effect. Treat such changes as security review items.
- Runtime plugin loading, dynamic native execution, remote UI, and dynamic command registration are not permitted without a separate accepted decision and threat model.

## Documentation and comments

- Documentation must clearly label **Implemented**, **Partially implemented**, **Planned**, and **Deferred** behavior.
- Never describe remaining target-only crates, full provider runtime lifecycle, full 19-crate extraction, the Phase 4 Cargo-metadata gate, or later-phase checker gates as implemented/active. Describe the typed mod/loader provider APIs, all eight extracted implementation crates, two immutable composition registries, typed host loader adoption, the Phase 1/2 registry/selection milestones, shared core policy/typed-port contracts, the feature lifecycle API crate, the implemented host lifecycle/availability IPC/gating/filtering milestone, the implemented Phase 3 frontend contribution slots, and the active Phase 3 gate accurately.
- Update the affected authoritative contract for every public behavior change. Update both RPC contract files only when the affected shared model, command, or event surface appears in both. Update architecture/standards when a boundary or phase rule changes.
- Use only relative Markdown links that resolve inside `docs/`. Do not link to another checkout, an external site, or a file outside the documentation tree.
- Keep headings concrete and stable. Avoid marketing language, invented claims, and comparisons to unrelated products.
- Comments should explain why a non-obvious invariant, safety check, compatibility path, or workaround exists. Do not narrate obvious assignments.
- Document security limits and failure behavior next to the code that enforces them when the contract alone is not enough.
- Remove stale comments and migration markers when the migration completes.

### Originality requirement

**Do not introduce names, branding, identifiers, copied text, or copied code from other projects.** External references may inform expected behavior only. The implementation, naming, structure, comments, tests, fixtures, and documentation must be original.

Do not mention unrelated products anywhere in code, identifiers, comments, docs, fixtures, or test data. Framework and platform names already required by this project may appear only where technically necessary. If a reference implementation is consulted, do not copy its API names, prose, file layout, screenshots, test cases, or distinctive identifiers; derive an original implementation from the behavioral requirement.

## Maintainability guidance

- Split by responsibility and change reason, not by arbitrary line count.
- Avoid god modules such as a single command file, a single state file, or a single provider file that owns unrelated capabilities.
- Avoid premature abstractions. A shared module is justified by stable repeated behavior, not by two superficially similar code fragments.
- Keep public APIs small and intentionally documented. A broad barrel export is not a substitute for ownership.
- Prefer explicit composition and typed dependency injection over global registries, ambient mutable state, or service locators.
- After the relevant provider/frontend registry phase, keep feature-specific branches out of shared code. If shared code needs to know a concrete feature/provider, the boundary is wrong. Before that phase, add only the single explicit dispatch path allowed by the compliant pre-phase matrix.
- Keep provider-specific payload mapping inside the provider and public DTO mapping at the boundary.
- Do not introduce dynamic plugin machinery before its separate security and ABI decision.
- Delete superseded migration shims when the new path is proven; do not keep two dispatch systems indefinitely.
- Make errors deterministic and testable. Avoid hidden I/O in constructors and descriptor registration.
- Favor clear ownership of async work, timeouts, cleanup, and partial-failure reporting.

## New-module checklist
Current status: the typed mod and loader provider APIs, two extracted mod implementation crates, six extracted loader implementation crates, two host-composed immutable registries, and generic Minecraft core helpers are implemented at milestone level. Loader host adoption builds typed contexts through the sole `LoaderKind` adapter in `loaders.rs`; full provider runtime lifecycle, extraction of the remaining five feature crates, full typed-port/Cargo-metadata checker acceptance, and Phase 5 remain pending. New providers extend the extracted contracts/crates. The active Phase 3 gate enforces exact mod/loader provider source, manifest, dependency, composition, and selection-adapter boundaries. Phase 3 frontend modules use the contribution registry. Phase 4 has the fourteen-crate workspace/core/feature-API/provider-API/two-settings-saves/two-mod/six-loader foundation and host lifecycle/availability snapshot/gating milestone implemented.

Use this checklist when adding a module, feature, provider, or major service boundary in the phase that introduces it. Before that phase, apply the phase applicability matrix and mark target-only checks `N/A` with the phase basis.

- [ ] One clear responsibility and owner are defined in a sentence.
- [ ] The placement decision is core, API, feature, provider, adapter, or shared utility, with a reason.
- [ ] Stable feature/provider/contribution IDs follow the identifier rules and are not display labels. Frontend contribution IDs are globally unique across all slots.
- [ ] `feature_version`, `api_version`, and `startup_policy` are set and compatible with the host.
- [ ] Feature dependencies are acyclic. Versioned provider capability requirements are complete and satisfiable through the full exact-one compatibility/selection predicate, including any optional per-capability selection key.
- [ ] The descriptor has one initialization entry point and no side effects at registration time.
- [ ] Initialization validates prerequisites before side effects.
- [ ] Initialization cleans its own partial resources on failure; shutdown is idempotent, reverse-order, and releases every owned resource; planned Tauri `RunEvent::Exit` integration owns final teardown.
- [ ] Health/readiness state is explicit and does not report success prematurely.
- [ ] Failure propagation covers the failed feature plus all transitive dependents. Effective criticality makes any affected `Required` feature fatal (`StartupFailed` or `RuntimeFailed`), `Degraded` is allowed only while every `Required` remains `Ready`, no `Required` remains `Ready` with a non-ready dependency/provider, and diagnostic `StartupFailed` cleanup covers initialized providers and core.
- [ ] Every frontend contribution has a globally unique feature-prefixed ID across all slots, an explicit slot/order, and declared requirements; navigation identifies its page with `targetPageId`.
- [ ] After the relevant registry phase, no central hard-coded feature/provider branch was added.
- [ ] The implemented checker passes its active release workflow `prepare` gate at `--phase 3` for frontend manifest/composition and provider boundaries. Where later crates/workspace exist, the Phase 4 Cargo metadata/allowlist gate must be activated before claiming full layer enforcement.
- [ ] Frontend imports are limited to the feature own subtree plus core/shared; no cross-feature imports exist, including public feature entries. Cross-feature composition uses typed core/shell contracts or contribution slots.
- [ ] Tauri types and command macros remain in the host/adapters layer.
- [ ] New commands are thin, explicitly registered in the compile-time handler, and documented.
- [ ] Frontend API wrappers, Rust DTOs, TypeScript DTOs, event payloads, and contract docs agree.
- [ ] Navigation/page/settings/instance-action contributions, if any, are registered through the frontend registry; non-ready features are not mounted and are shown only through diagnostics.
- [ ] State ownership is documented; no feature writes another feature's state or the settings file directly.
- [ ] Frontend loading uses local static/literal lazy imports only; contribution loaders are component/lazy-function references, never runtime string paths, URLs, or remote code.
- [ ] Untrusted paths, archive members, provider payloads, URLs, filenames, and process output are validated with limits.
- [ ] Secrets are absent from DTOs, events, errors, logs, and command display strings.
- [ ] Blocking work is scheduled safely and has timeouts/bounds where applicable.
- [ ] Stale request and cleanup behavior is handled on the frontend.
- [ ] Tests cover descriptor/provider version compatibility, per-capability key bindings, deterministic selection and provider-ID order, feature dependency cycles, duplicate/missing/ambiguous cases, affected-set failure propagation, the every-Required-Ready degraded invariant, StartupFailed/RuntimeFailed transitions, diagnostic cleanup, event semantics, boundary mapping, and security-relevant edge cases.
- [ ] Required CI checks, including the phase-current `scripts/check-architecture.mjs` gate once available, are identified and passing before completion.
- [ ] Documentation describes implemented behavior separately from planned migration work.
- [ ] No names, branding, identifiers, prose, or code were copied from another project.

## Code-review checklist

The reviewer is independent of the implementer, reviews logic and current-phase boundary correctness, and does not execute builds/tests. Apply the phase applicability matrix first: target-only checks are `N/A` before their phase, while mandatory current-phase checks still apply. Reviewing is not a substitute for CI execution. Current review baseline: typed mod and loader provider APIs, two extracted mod implementation crates, six extracted loader implementation crates, two extracted settings/saves feature crates, two immutable provider registries, and typed host loader adoption are implemented at milestone level; Phase 3 frontend contribution slots implemented; Phase 4 fourteen-crate workspace/core/feature-API/provider-API/provider foundations and host lifecycle/availability IPC/gating/filtering milestone implemented; and the release workflow `prepare` gate active at `--phase 3` for frontend manifest/composition/import, provider import/selection, exact mod/loader provider dependency/source/composition boundaries, exact six loader leaves/descriptors/requirements, forbidden-pattern boundaries, exact allowlists, static workspace shape, and the 30-command/28-feature-command gate map (not full typed ports or Cargo metadata). Extraction of the remaining five feature crates (settings and saves are implemented), full 19-crate extraction, full provider runtime lifecycle, the Phase 4 Cargo-metadata gate, and Phase 5 remain pending.

### 1. Correctness and logic

- [ ] The change does what the task and contract require, including error and edge paths.
- [ ] State transitions are valid: no false-ready state, silent fallback, or success after a failed write.
- [ ] Duplicate/missing/incompatible provider, capability, contribution, and feature cases are handled before side effects. Feature dependency cycles are detected separately; providers are independent leaves. Zero-match and multi-match provider selection are unavailable/ambiguous without map-order dependence.
- [ ] Initialization order and shutdown order are correct and deterministic; failed init cleans its own partial resources; successful features shut down in reverse order, including through the planned Tauri `RunEvent::Exit` adapter.
- [ ] Concurrent/stale requests cannot overwrite newer state.
- [ ] Failure propagation includes all transitive dependents; any affected `Required` feature makes startup/runtime fatal; `Degraded` is used only when every `Required` feature remains `Ready`; no `Required` feature remains `Ready` with a non-ready dependency/provider; no feature-level `Degraded` exists.
- [ ] Command results/durable state are authoritative; default progress-event failures are logged best-effort, while an explicitly required protocol event failure returns an error or typed partial success without rolling back committed state.
- [ ] No panic, unbounded loop, recursion, allocation, or timeout risk exists on untrusted input.

### 2. Dependency direction and boundaries

- [ ] The current-phase dependency allowlist is satisfied. At full extraction, core has no host, feature-module, provider-implementation, frontend, or feature-specific imports.
- [ ] Modules do not import the host or another module's private files.
- [ ] Providers are independent leaves: they do not import host, feature internals, any other provider, or Tauri, and declare no provider-to-provider dependencies.
- [ ] Tauri types, `AppHandle`, event emission, and command macros remain in host/adapters.
- [ ] When the Rust composition root exists, it is the only place that instantiates concrete feature/provider implementations. `src-tauri/src/provider_composition.rs` is the sole concrete mod/loader provider composition root and owns both immutable registries; `src-tauri/src/loaders.rs` contains only the sole `LoaderKind` adapter and orchestration. No concrete provider construction is allowed in host commands or feature modules.
- [ ] The frontend composition root is the only place that imports concrete feature modules for assembly. Every feature import stays within its own subtree plus frontend core/shared; no cross-feature import, including a public entry, is allowed.
- [ ] No new Cargo or TypeScript dependency creates an avoidable cycle or expands a public layer unnecessarily. Cargo/TypeScript module systems alone do not enforce arbitrary allowlists; the implemented checker supplies the active release workflow `prepare` Rust/TypeScript gate at `--phase 3`, while Phase 4+ Cargo/full-AST gates must be advanced explicitly before they are claimed active.
- [ ] Tauri command handlers are thin and delegate behavior to feature/application services.

### 3. Contracts and models

- [ ] Command names, arguments, results, events, enum strings, and behavioral rules match the contract documents.
- [ ] Rust serde casing matches TypeScript properties and persisted JSON expectations.
- [ ] New persisted fields are optional/defaulted for older data.
- [ ] Internal types are mapped deliberately at the adapter boundary.
- [ ] Frontend contribution IDs are globally unique across all slots, feature-prefixed, and page targets use canonical `targetPageId` references. The runtime registry rejects duplicates and unknown targets.
- [ ] The affected authoritative contract is updated for public behavior; both RPC contract files are updated only for shared model, command, or event surfaces. Architecture status and implementation agree.

### 4. Security

- [ ] Caller paths and filenames are revalidated against traversal, absolute paths, symlinks, and root escapes.
- [ ] Archive entries and decompression limits are enforced.
- [ ] Network requests use HTTPS, bounded sizes/redirects, and timeouts.
- [ ] Downloaded mod archives are never executed.
- [ ] Loader process execution uses the bounded/approved path and safe logging.
- [ ] Secrets are absent from logs, events, errors, notices, launch command strings, and DTOs.
- [ ] Downloaded or provider-supplied content is never treated as executable UI or trusted HTML. Contribution loaders are never runtime string paths, and dynamic imports use only literal local specifiers.
- [ ] Capability/content-security changes are explicit and justified.

### 5. Maintainability

- [ ] The split follows responsibility, not arbitrary line count.
- [ ] Public exports are minimal and intentional.
- [ ] No god module, hidden global state, service locator, or duplicate dispatch path was introduced.
- [ ] After the relevant provider phase, provider-specific logic is not spread back into shared code; any pre-phase extension uses the single migration path in the matrix.
- [ ] Naming is clear, consistent, and original.
- [ ] Comments explain non-obvious invariants and safety decisions rather than restating code.
- [ ] Dead migration code and temporary compatibility shims have an owner and removal point, or were removed.
- [ ] The change does not add premature runtime plugin machinery, and target-only features are not presented as implemented before their phase.

### 6. Documentation and evidence

- [ ] Implemented and planned behavior are labeled correctly.
- [ ] Markdown links are relative and resolve within `docs/`.
- [ ] Documentation does not contain unrelated product names or copied external identifiers/prose.
- [ ] The completion report lists files, behavior/contract impact, risks, and the CI verification evidence.
- [ ] The reviewer did not claim or substitute local build/test execution for CI.

## Team delivery workflow

### Implementer

1. Read the task, contracts, architecture, standards, and phase applicability matrix. Inspect the actual repository before deciding that a boundary or target structure exists.
2. Define the write scope and ownership boundary in the task or handoff notes.
3. Implement the smallest complete change that satisfies the task. Keep unrelated refactors out of scope.
4. Update tests, the affected authoritative contract, DTOs, client wrappers, events, and docs required by the change. Update both RPC contract files only when a shared surface is affected.
5. Hand off to an independent reviewer with a concise logic summary, changed-file list, risks, and the intended CI evidence.
6. Address reviewer findings in the same scoped change.
7. Use CI as the build/test authority. Do not report a check as passed unless CI actually ran it successfully or the task defines an equivalent authoritative result.
8. Commit completed work locally and do not push unless explicitly requested. A task that explicitly says not to commit is governed by its task instruction.
9. Report the local commit or uncommitted status, files changed, contract impact, CI status, and residual risks.

### Independent reviewer

- Review the logic, current-phase dependency direction, provider/feature lifecycle semantics, contract consistency, security behavior, and maintainability of the proposed change.
- Do not run local builds, tests, typechecks, formatters, linters, package commands, or `scripts/check-architecture.mjs`. Do not use local command output as review evidence.
- Do not rewrite the implementation unless the task explicitly reassigns implementation work.
- Report findings with file/line references, the violated rule, the failure scenario, and the expected behavior.
- Separate blocking defects from optional suggestions.
- Consult CI results for executable verification. If CI does not cover a required check, state that the check is unverified or pending rather than substituting a local command.
- Approve only when logic and boundaries are acceptable, target-only checks are correctly marked `N/A` before their phase, and the applicable contract/docs are synchronized.

### CI authority and completion

- CI is the build/test authority for the change. The `flint-release.yml` release workflow `prepare` job installs frontend dependencies, runs `node --test scripts/test-architecture-checker.mjs`, then runs `node scripts/check-architecture.mjs --phase 3`; all build-matrix jobs depend on `prepare`. The gate covers frontend manifest/composition/import boundaries, module/provider import and selection boundaries, forbidden std/process/fs/net/reqwest/Tauri patterns in all extracted provider leaves, exact mod/loader provider dependency/source/composition edges, exact six loader leaves/descriptors/requirements, both immutable composition registries, the sole `LoaderKind` adapter, exact allowlists, static exact fourteen-member workspace membership, and the host 30-command/28-feature-command gating map. Mod and loader typed-port adoption is enforced at milestone level; extraction of the remaining five feature crates, full provider runtime lifecycle, the Phase 4 Cargo metadata gate, and Phase 5 remain pending advancement. A green local build or informal review does not replace CI evidence.
- CI failures go back to the implementer. The reviewer does not rerun the build, test suite, or architecture checker locally to diagnose them.
- Do not mark work complete while required CI checks are failing or uncollected. If the repository has no CI coverage for a required check, record that gap explicitly and add or schedule verification through the appropriate scoped work.
- Do not push, publish, tag, or release unless the task explicitly requests it.
- Keep the documentation index and implementation status current when a migration phase or accepted target changes.
