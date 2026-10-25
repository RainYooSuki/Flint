# Development Standards

## Status and scope

These standards are mandatory for changes to the implemented architecture. The migration is complete through Phase 5 in the working tree, subject to GitHub Actions verification. The strict rules apply to the current implementation rather than a future migration target: all nineteen Cargo packages, typed provider contracts/ports, shared actual-registry provider lifecycle state, the Rust feature lifecycle/availability coordinator, the five-slot frontend contribution registry, typed frontend host services, and the strict `--phase 5` checker are present in the repository.

The architecture remains a compile-time **modular monolith**. There is no runtime plugin discovery, arbitrary native loading, remote UI loading, or dynamic command registration in the approved scope.

### Current implementation status

The canonical phase-status table is [`ARCHITECTURE.md`](ARCHITECTURE.md#canonical-migration-phase-status). This document owns process, checklists, and concise mandatory summaries; it does not replace the canonical status table.

Current summary:

- The nineteen-package Cargo workspace contains `flint-core`, `flint-feature-api`, `flint-provider-api`, seven feature crates, two mod provider crates, and six loader provider crates. `ProviderRuntimeState` owns the two actual immutable provider registries and implements the feature `ProviderCatalog`; provider-backed commands and lifecycle availability consume that same state.
- `flint-feature-api` provides validated feature descriptors, dependency ordering, the lifecycle state machine, failure propagation, command gating, reverse/idempotent shutdown, and the availability DTO. `src-tauri/src/feature_host.rs` composes seven explicit features and starts the coordinator; `lib.rs` publishes availability, gates all 28 feature commands, and invokes shutdown on `RunEvent::Exit`.
- The frontend registry supports pages, navigation, settings sections, instance actions, and overlays. Tauri package access is confined to the exact five reviewed adapters for invoke, events, dialogs, opener, and window. `main.tsx` creates `HostServices`; `AppPageContext` injects it; features do not import Tauri directly.
- The release `prepare` gate runs the architecture and frontend contract tests and then `node scripts/check-architecture.mjs --phase 5`. The checker is strict, exception-free, and fail-closed for the production tree.
- GitHub Actions is the final verification authority. This working-tree documentation change does not claim a green hosted run.

Future extensions such as runtime plugins require a separate accepted Architecture Decision Record and threat model. They do not reopen the completed migration phases.

## Sources of truth and precedence

1. An explicit task instruction governs the immediate work scope, including when a task prohibits commits or limits writable files.
2. The applicable sections of [`API_CONTRACT.md`](API_CONTRACT.md) and [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md) govern externally visible frontend/backend commands, arguments, DTO fields, enum strings, events, and behavioral rules. Update both only when a shared model, command, or event surface is affected.
3. [`ARCHITECTURE.md`](ARCHITECTURE.md) is the canonical detailed source for implemented boundaries, dependency direction, provider/registry and lifecycle semantics, frontend contributions/adapters, the strict Phase 5 checker, and acceptance gates. This document provides process, checklists, and concise summaries with links to that detail.
4. This document governs implementation process, quality bars, review, and delivery.
5. [`architecture-decision-records/0001-modular-core-and-feature-modules.md`](architecture-decision-records/0001-modular-core-and-feature-modules.md) records why the architecture decision was made. It does not override the contracts or detailed architecture.
6. [`INTEGRATIONS.md`](INTEGRATIONS.md) records current integration setup and local-storage guidance.
7. [`CODE_REVIEW.md`](CODE_REVIEW.md) is a point-in-time quality and readiness snapshot, not standing proof that a later change has passed anything.

If two sources conflict about a public command, event, DTO, or user-visible rule, stop and resolve the conflict explicitly before implementing. Do not silently choose one and leave the others stale.

## Completed phase matrix and current rules

All migration phases are implemented in the working tree. The strict Phase 5 rules are mandatory now; earlier phase rules remain binding wherever they identify ownership, provider, contribution, lifecycle, or contract behavior.

| Rule group | Phase 1: ModProvider | Phase 2: LoaderProvider | Phase 3: Frontend registry | Phase 4: Workspace/modules | Phase 5: Hardening |
| --- | --- | --- | --- | --- | --- |
| Contracts, documentation, security, error/file/network rules | Implemented | Implemented | Implemented | Implemented | **Mandatory:** affected authoritative contract updated for public behavior; both RPC contracts only for a shared model/command/event surface. |
| Dependency direction and cycle hygiene | Typed ModProvider and extracted leaves | Typed loader leaves and host adoption | Feature-local UI imports only | Exact nineteen-package Cargo/source gate | **Strict:** exact crate/source/dependency allowlists, no cycles, no production migration exception. |
| Provider API, versioned capabilities, registry, typed contexts | Typed mod contracts and two leaves | Typed loader contracts, six leaves, 75/600/600 split | Shared typed contracts; no frontend provider branching | Typed provider ports and injected registries | **Strict:** one shared actual-registry `ProviderRuntimeState`, provider lifecycle/loss/shutdown propagation, no raw provider I/O. |
| FeatureDescriptor, lifecycle, failure propagation, and Rust composition root | Mod/provider requirements consumed | Loader/provider requirements consumed | Frontend descriptor/registry validation | Seven-feature host composition and availability IPC | **Strict:** Required/optional affected-set semantics, RuntimeFailed/Stopping/Stopped, unavailable gating, reverse/idempotent shutdown. |
| Frontend contribution registry and composition root | No provider-specific frontend dispatch | No provider-specific frontend dispatch | Pages/navigation/settings/instance actions | Availability filtering for all original slots | **Strict:** typed overlay slot, frozen global IDs, exact host-service injection, availability filtering for all five slots. |
| Tauri boundary and lifecycle | Thin mod command adapters | Thin loader command adapters | Statically rendered contributions | Managed availability and 28 command gates | **Strict:** exact five frontend Tauri adapters/importers, thin host event adapters, `RunEvent::Exit` shutdown, no feature/provider Tauri imports. |
| Architecture checker and CI gate | Provider rules retained | Provider rules retained | Frontend registry rules retained | Cargo/source/typed-port rules retained | **Active:** architecture/frontend contract tests plus `node scripts/check-architecture.mjs --phase 5` in `prepare`; CI failure blocks completion. |

### Mandatory implementation rules after Phase 5

The following are mandatory for every change:

- Preserve public command names, DTO fields, enum serialization, event names/payload shapes, settings behavior, and user-visible behavior unless an intentional contract change is documented and coordinated.
- Update the affected authoritative contract for every public behavior change. Update both [`API_CONTRACT.md`](API_CONTRACT.md) and [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md) only when the shared model, command, or event surface is affected.
- Apply security, path/archive/network safety, secret redaction, bounded I/O, stale-request handling, and no-panic rules to changed code.
- Keep feature code inside its owning crate/module and host logic inside thin adapters. Do not add business logic to Tauri command wrappers.
- Use the shared provider registries/lifecycle state. Do not reintroduce provider-specific call-site branches, metadata-only provider catalogs, or a second provider lifecycle truth.
- Use the typed frontend contribution registry and injected `HostServices`. Do not import Tauri packages outside the exact five adapters or add a second page/navigation/settings/action/overlay dispatch path.
- Do not add copied names, branding, identifiers, prose, code, or unrelated product references.
- Treat GitHub Actions as the build/test authority. Local reviewer commands are not CI evidence.

### Current implementation path (Phase 5 strict gate)

A change in the implemented architecture is compliant when it:

1. Preserves current contracts and places behavior in the existing owner crate/module with the narrowest necessary public surface.
2. Uses the shared `ProviderRuntimeState` and typed provider contexts for mod/loader changes; no provider-specific call-site branch or parallel provider-state catalog is added.
3. Uses the frontend contribution registry for pages, navigation, settings sections, instance actions, and overlays, and filters all slots through the validated host availability snapshot.
4. Injects `HostServices` through `AppPageContext`; only the exact five adapter files import Tauri packages, and only `main.tsx` creates the concrete host-service implementation.
5. Uses `useRequestGeneration` or the equivalent reviewed generation guard for asynchronous UI work and `useHostEvent` cleanup semantics for event listeners. Stale completion, unmount, or late registration must not mutate UI state after disposal.
6. Enforces archive entry limits and preflight validation before extraction; keeps actual-copy size, symlink, duplicate/case-alias, and cleanup defenses.
7. Uses sanitized/bounded progress text and redacted launch-command display paths. Never expose credentials, provider IDs, URLs, unsafe paths, or control characters through events, diagnostics, logs, or UI notices.
8. Runs and observes the release-workflow tests through `prepare` at `--phase 5`. Do not add phase-expiring production exceptions; an allowlist change is an architecture change requiring explicit review.
9. Records hosted CI as the verification authority; a local command or documentation edit is not proof that CI is green.

## Before-work checklist

- [ ] Confirm the task targets the correct repository and checkout. Do not modify another checkout or carry assumptions from a different working tree.
- [ ] Read the relevant contract sections, the architecture boundary rules, and the completed phase matrix before editing.
- [ ] Inspect the actual current implementation and file graph before deciding that a boundary exists.
- [ ] Write down the user-visible behavior, compatibility constraints, and non-goals of the change.
- [ ] Classify the work and identify its phase: core service, provider, feature module, adapter, shared utility, or documentation-only.
- [ ] Identify the exact writable scope and list files/directories expected to change.
- [ ] Identify every contract, DTO, command, event, settings field, or UI contribution affected, and which authoritative contract must change.
- [ ] Draw the intended dependency direction and check it against the implemented layer rules, exact allowlists, and forbidden/cyclic imports.
- [ ] Identify filesystem, network, archive, process, secret, and path-safety risks.
- [ ] Decide lifecycle/failure behavior: affected-set propagation, effective criticality (any affected Required is fatal), Required-Ready degraded invariant, dependency skipping, unavailable commands, diagnostic StartupFailed cleanup, partial success, cleanup/shutdown, event semantics, retry, cancellation, and stale UI requests.
- [ ] Identify the CI checks needed for the change, including the architecture/frontend contracts and the active `--phase 5` checker. Do not substitute reviewer-run local commands for CI.
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

Current status: the nineteen-crate workspace, typed mod/loader provider contracts and all eight provider leaves, seven feature crates, shared actual-registry provider lifecycle state, framework-neutral feature lifecycle/availability coordinator, typed frontend host services, and five-slot contribution registry are implemented. New mod/loader provider work belongs in the extracted API/provider crates; payload mapping and safe I/O stay there. Frontend pages, navigation, settings sections, instance actions, and overlays belong in feature modules and the explicit `src/app/composition.ts` registry, filtered through the validated availability snapshot.

1. **Define ownership.** Choose one cohesive responsibility and record whether it is a core service, feature module, provider, adapter, or shared utility. Do not use a new crate or folder as a substitute for an ownership decision.
2. **Declare the descriptor.** For a Rust feature, use the canonical descriptor contract: stable ID, `feature_version`, `api_version`, `startup_policy`, required features, provider requirements, one initialization entry point, and backend contributions. For frontend UI, extend the current typed `FeatureDescriptor` with the appropriate reviewed slot. Registration data is side-effect free.
3. **Initialize safely.** Validate prerequisites before side effects; receive dependencies through typed contexts; start no owned worker until initialization succeeds. On failure, clean every partial resource before returning the error. Follow the canonical startup/runtime states and reverse shutdown order.
4. **Register contributions.** Use an approved registry slot, stable feature-prefixed IDs, declared requirements, and deterministic order. Reject duplicates instead of overriding, and never mutate a frozen registry.
5. **Wire boundaries.** Add thin host/command/API adapters for public behavior, explicitly register commands in the compile-time handler, and add frontend contributions through the composition root. No feature imports another frontend feature module or public entry; cross-feature use goes through typed core/shell contracts or contribution slots.
6. **Update the contract.** Update the affected authoritative contract for public behavior. Update both RPC contract files only for a shared model, command, or event surface.
7. **Verify the edges.** Add CI-covered coverage for descriptor/provider compatibility, deterministic provider-ID order, feature cycles, duplicate/missing/ambiguous cases, startup/runtime failure and shutdown, serialization/events, security limits, stale UI requests, and UI availability rules. Use the strict Phase 5 gate and hosted CI as evidence.

## Provider implementation rules

[`ARCHITECTURE.md`](ARCHITECTURE.md) is canonical for descriptor, compatibility/selection predicate, typed context ports, lifecycle, and port-enforcement mechanics.

- The typed mod and loader provider APIs, all eight provider implementation crates, and all seven feature crates are implemented. `ProviderRuntimeState` owns one immutable mod registry and one immutable loader registry and is the sole feature provider catalog. New provider work starts from `src-tauri/crates/flint-provider-api` and the corresponding extracted `providers/*` crate. Do not add hard-coded provider branches, a parallel dispatch path, or a second provider lifecycle state.
- Define behavior in the provider contract. Features depend on provider traits, not concrete implementations. `src-tauri/modules/flint-feature-loaders/src/lib.rs` owns the sole `LoaderKind` binding/orchestration; the host `src-tauri/src/loaders.rs` adapter only translates progress events/sinks.
- Provider crates do not import host/private modules, Tauri, another provider, feature internals, raw HTTP clients, direct filesystem/process/network APIs, or concrete core adapters. They use the typed network/path/archive/Java/progress/logging/deadline/cancellation contexts and, for mods, a redacted scoped credential.
- Provider descriptors expose `provider_version`, `provider_api`, and capabilities. Each capability binds `{ id, api_version, selection_keys }`; selection keys are not a provider-level list. Duplicate provider/capability IDs and duplicate capability/key bindings fail during validation.
- Providers are independent leaves. Do not declare provider-to-provider dependencies, build a provider dependency graph, or run provider-cycle detection.
- Provider initialization/shutdown and runtime loss update `ProviderRuntimeState`. A failed or stopped provider is unavailable for selection and propagates through feature Required/optional semantics. Do not use `HashMap` iteration, filesystem discovery, or registration order for selection.
- Select providers by the full feature requirement: host provider-API compatibility, capability ID/API compatibility, semantic provider version, optional capability-scoped selection key, and active provider state. Exactly one provider must match; zero is unavailable and multiple is ambiguous. No fallback.
- Map serialized `ModSource`/`LoaderKind` values to stable selection keys in the reviewed single adapter locations. Do not add call-site enum matching in commands or shared services.
- Keep provider payloads private and normalize to contract DTOs at the boundary. Provider IDs, versions, capabilities, raw payloads, credentials, and error bodies must not leak into public DTOs, logs, events, diagnostics, or notices unless a contract explicitly introduces them.
- Mod providers never execute a downloaded mod archive. Loader providers run only trusted installer flows with bounded processes, captured/redacted output, timeouts, and safe archive policy.
- Provider progress text is credential/URL-redacted, control-normalized, and bounded before publication. Command results and committed durable state remain authoritative; default progress events are best-effort.
- The strict checker at `--phase 5` enforces exact provider source/dependency/composition boundaries, one shared actual-registry `ProviderRuntimeState`, typed context-port use, forbidden raw APIs, exact six loader contracts/constructors, one `ModSource` binding, one `LoaderKind` binding, and exact allowlists. No temporary provider exception remains.

## Frontend contribution and state rules

[`ARCHITECTURE.md`](ARCHITECTURE.md) is canonical for contribution slots, registry validation, host-service injection, state ownership, and loading mechanics.

### Contributions

- Pages, navigation, settings sections, instance actions, and overlays render only through the compiled, validated registry. The settings feature mounts five sections; launch mounts an instance action; accounts mounts the account-dialog overlay. Unready/unavailable features are hidden and their commands are gated by backend feature ID.
- New UI uses typed contributions from the explicit composition root. The shell must not keep a second hard-coded page/navigation/settings/action/overlay list. Settings sections use the shared draft/validators; instance actions use the typed action context.
- Contribution IDs are feature-prefixed and globally unique across all five slots. A page contribution ID is canonical contract data; navigation uses `targetPageId`.
- The registry rejects duplicate IDs, unknown/ambiguous navigation targets, invalid owners/components/selectors, and feature cycles before rendering. Order by explicit `order`, then stable ID; freeze before render.
- A frontend feature may import only its own subtree plus frontend core/shared contracts. It must not import another feature module, including its public entry.
- Static imports and one-string-literal local `lazy(() => import("./literal-path"))` are allowed only inside the importing feature subtree or an explicitly reviewed local chunk. Variable/template specifiers, absolute/URL imports, runtime string loaders, remote code, `eval`, `new Function`, and module federation are prohibited.
- Tauri package imports are limited to the exact files and specifiers enforced by the strict checker. Features consume `HostServices` through `AppPageContext`; they do not import concrete Tauri adapters.
- Contribution registration is pure declaration. Do not perform I/O, subscriptions, timers, or store creation at module evaluation.

### State ownership

- App/shell state is limited to cross-feature presentation and selection: active page ID, selected instance, effective game directory, global notices, availability snapshot, registry, and injected host services.
- Each feature owns its query results, non-shared drafts, progress, modals/overlays, and action busy/error state.
- The settings owner owns the single saved/draft settings workflow and persistence. The accounts owner owns account state/mutations. Other features use published core/shell contracts, never another feature module.
- Cross-feature state/actions move through typed core contracts or app-shell contexts. Do not introduce a mutable global singleton to avoid explicit dependency passing.
- Use `useRequestGeneration` or an equivalent reviewed generation guard so stale success/error completion cannot overwrite newer state or clear a newer loading state. Clean up listeners/timers on unmount and when dependent context changes.

### API and event use

- Call commands through the shared typed API client; keep TypeScript request/response types identical to the serialized contract. The client reaches IPC only through `src/core/api/tauriInvoke.ts`.
- Type event payloads, keep literal names contract-compatible, and clean up every listener on rejected and resolved registration. Use `useHostEvent`/`createHostEventSubscriber` semantics so late registration is disposed immediately.
- Command results and committed durable state are authoritative. Default progress events are best-effort; delivery failure is logged and does not fail an otherwise successful command or roll back committed state. If a contract marks an event as required protocol, delivery failure returns an error or typed partial success without rolling back committed state.
- Provider-influenced loader/mod progress messages are sanitized, control-normalized, and bounded before events are published. Launch-command display is redacted; tokens, credentials, provider IDs, URLs, unsafe paths, and unredacted commands must not appear in events, errors, diagnostics, logs, or notices.
- Contributions from non-ready features are not mounted, and dependent commands return unavailable-feature errors instead of falling back.

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
- Provider implementations must not import Tauri or receive `AppHandle`; provider contexts carry framework-neutral ports. `flint-core`, `flint-feature-api`, `flint-provider-api`, and all eight extracted provider crates are framework-neutral. No feature/provider crate may depend on or import Tauri. Domain/business behavior stays outside command wrappers.
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
- The framework-neutral Rust feature lifecycle API/registry/coordinator is implemented in `flint-feature-api`, and the Tauri host composes/starts it through `feature_host.rs` with managed state, `get_host_availability`, 28-command gating, and exit shutdown. The canonical target summary remains: failure propagates to the failed feature plus all transitive dependents; any affected `Required` feature takes `StartupFailed` with feature/provider/core cleanup or `RuntimeFailed` -> `Stopping` -> `Stopped`/exit; an optional-only affected set may leave the host `Degraded` only while every `Required` feature remains `Ready`; a `Required` feature must never remain `Ready` with a non-ready dependency/provider. There is no feature-level `Degraded`. Both mod and loader provider registries are immutable and owned by the shared `ProviderRuntimeState`; provider-backed commands and the feature provider catalog consume the same lifecycle truth, including initialization, loss, and shutdown.
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
- The host adapter alone owns Tauri `RunEvent::Exit` and `RuntimeFailed` shutdown coordination; feature/provider code does not import Tauri run events or hold `AppHandle`. States and ordering are canonical in [`ARCHITECTURE.md`](ARCHITECTURE.md), and shutdown must not hold the UI thread indefinitely.

## Security, filesystem, and network safety

### Input and path safety

- Never trust frontend-provided paths, file names, provider IDs, versions, URLs, archive member names, or metadata sizes.
- Normalize paths before validation; reject absolute/traversal components where a relative component is required.
- Join caller-controlled names only through the shared safe-join policy and verify the resolved path remains inside the intended root.
- Reject symlink escapes, duplicate/unsafe archive names, and writes to reserved/temporary paths as required by the operation.
- Do not derive a write path from a display name, provider-provided filename, or project title without sanitization and validation.
- Destructive operations need explicit confirmation at the UI layer and revalidation at the backend. UI confirmation does not replace backend validation.

### Archives and processes

- Enforce archive entry count before per-entry parsing, then per-file size, total uncompressed size, compression ratio/actual-copy limits, path length, and depth limits.
- Preflight every entry through the shared safe-archive policy: reject traversal, absolute/unsafe paths, symlinks, duplicates/case aliases, reserved names, and oversized entries before creating output. Revalidate while writing and clean partial extraction output on failure.
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

- Tokens, refresh tokens, API keys, passwords, provider URLs, authorization headers, and unredacted launch commands must never appear in command results, events, notices, logs, diagnostics, or UI strings. Provider-influenced progress text is sanitized, control-normalized, and bounded before event publication.
- Persist secrets only in the documented local store and with the existing platform permission behavior.
- Validate every command argument again on the backend, regardless of frontend validation.
- Do not expand Tauri capabilities, window permissions, or content-security settings as an unrequested side effect. Treat such changes as security review items.
- Runtime plugin loading, dynamic native execution, remote UI, and dynamic command registration are not permitted without a separate accepted decision and threat model.

## Documentation and comments

- Documentation must label current behavior **Implemented** and reserve **Planned** or **Deferred** for genuinely future extensions.
- Describe the current tree accurately: nineteen Cargo packages, typed mod/loader provider APIs and all eight implementation crates, one shared actual-registry `ProviderRuntimeState`, seven extracted feature crates, the feature lifecycle/availability IPC/gating/filtering system, the five-slot frontend contribution registry, exact typed host services, and the strict `--phase 5` checker gate. Do not label completed migration structures as pending.
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
- Keep feature-specific branches out of shared code. If shared code needs to know a concrete feature/provider, the boundary is wrong. Do not add a parallel provider or frontend dispatch path.
- Keep provider-specific payload mapping inside the provider and public DTO mapping at the boundary.
- Do not introduce dynamic plugin machinery before its separate security and ABI decision.
- Delete superseded migration shims when the new path is proven; do not keep two dispatch systems indefinitely.
- Make errors deterministic and testable. Avoid hidden I/O in constructors and descriptor registration.
- Favor clear ownership of async work, timeouts, cleanup, and partial-failure reporting.

## New-module checklist
Current status: the typed mod and loader provider APIs, two extracted mod implementation crates, six extracted loader implementation crates, one shared actual-registry `ProviderRuntimeState`, and generic Minecraft core helpers are implemented. The loader feature crate builds typed contexts through its sole `LoaderKind` binding; the host loader module only adapts progress events. New providers extend the extracted contracts/crates. The strict Phase 5 gate enforces exact Cargo metadata/source roots, provider source/manifests/dependencies/composition/typed ports/selection adapters, the five-slot frontend contribution registry, typed host-service injection, stale-request guards, and shim bans.

Use this checklist when adding a module, feature, provider, or major service boundary to the implemented architecture. All items below apply unless explicitly marked as future extension work.

- [ ] One clear responsibility and owner are defined in a sentence.
- [ ] The placement decision is core, API, feature, provider, adapter, or shared utility, with a reason.
- [ ] Stable feature/provider/contribution IDs follow the identifier rules and are not display labels. Frontend contribution IDs are globally unique across all slots.
- [ ] `feature_version`, `api_version`, and `startup_policy` are set and compatible with the host.
- [ ] Feature dependencies are acyclic. Versioned provider capability requirements are complete and satisfiable through the full exact-one compatibility/selection predicate, including any optional per-capability selection key.
- [ ] The descriptor has one initialization entry point and no side effects at registration time.
- [ ] Initialization validates prerequisites before side effects.
- [ ] Initialization cleans its own partial resources on failure; shutdown is idempotent, reverse-order, and releases every owned resource; the implemented Tauri `RunEvent::Exit` bridge owns final teardown.
- [ ] Health/readiness state is explicit and does not report success prematurely.
- [ ] Failure propagation covers the failed feature plus all transitive dependents. Effective criticality makes any affected `Required` feature fatal (`StartupFailed` or `RuntimeFailed`), `Degraded` is allowed only while every `Required` remains `Ready`, no `Required` remains `Ready` with a non-ready dependency/provider, and diagnostic `StartupFailed` cleanup covers initialized providers and core.
- [ ] Every frontend contribution has a globally unique feature-prefixed ID across all slots, an explicit slot/order, and declared requirements; navigation identifies its page with `targetPageId`.
- [ ] After the relevant registry phase, no central hard-coded feature/provider branch was added.
- [ ] The strict checker passes its release workflow `prepare` gate at `--phase 5` for exact Tauri adapters/importers, overlays, stale-request rules, Cargo metadata/dependency/source boundaries, workspace roots, cycles, shared provider runtime state, and typed provider ports.
- [ ] Frontend imports are limited to the feature own subtree plus core/shared; no cross-feature imports exist, including public feature entries. Cross-feature composition uses typed core/shell contracts or contribution slots.
- [ ] Tauri types and command macros remain in the host/adapters layer.
- [ ] New commands are thin, explicitly registered in the compile-time handler, and documented.
- [ ] Frontend API wrappers, Rust DTOs, TypeScript DTOs, event payloads, and contract docs agree.
- [ ] Navigation/page/settings-section/instance-action/overlay contributions, if any, are registered through the frontend registry; non-ready features are not mounted and are shown only through diagnostics.
- [ ] State ownership is documented; no feature writes another feature's state or the settings file directly.
- [ ] Frontend loading uses local static/literal lazy imports only; contribution loaders are component/lazy-function references, never runtime string paths, URLs, or remote code.
- [ ] Untrusted paths, archive members, provider payloads, URLs, filenames, and process output are validated with limits.
- [ ] Secrets are absent from DTOs, events, errors, logs, and command display strings.
- [ ] Blocking work is scheduled safely and has timeouts/bounds where applicable.
- [ ] Stale request and cleanup behavior is handled on the frontend.
- [ ] Tests cover descriptor/provider version compatibility, per-capability key bindings, deterministic selection and provider-ID order, feature dependency cycles, duplicate/missing/ambiguous cases, affected-set failure propagation, the every-Required-Ready degraded invariant, StartupFailed/RuntimeFailed transitions, diagnostic cleanup, event semantics, boundary mapping, and security-relevant edge cases.
- [ ] Required CI checks, including architecture/frontend contract tests and `scripts/check-architecture.mjs --phase 5`, are identified and passing before completion.
- [ ] Documentation describes implemented behavior separately from genuinely deferred future extensions.
- [ ] No names, branding, identifiers, prose, or code were copied from another project.

## Code-review checklist

The reviewer is independent of the implementer, reviews logic and boundary correctness, and does not execute builds/tests. Reviewing is not a substitute for CI execution. Current review baseline: nineteen Cargo packages, typed mod/loader provider APIs and all eight implementation crates, one shared actual-registry `ProviderRuntimeState`, seven feature crates, the feature lifecycle/availability coordinator, five frontend contribution slots with typed host services, and the strict Phase 5 checker gate. The release workflow `prepare` job runs architecture/frontend contract tests and `node scripts/check-architecture.mjs --phase 5`; exact adapter/importer, shim, Cargo metadata/external dependency/cycle, all-workspace source, provider port, loader composition, allowlist, and 30-command/28-feature-command rules apply.

### 1. Correctness and logic

- [ ] The change does what the task and contract require, including error and edge paths.
- [ ] State transitions are valid: no false-ready state, silent fallback, or success after a failed write.
- [ ] Duplicate/missing/incompatible provider, capability, contribution, and feature cases are handled before side effects. Feature dependency cycles are detected separately; providers are independent leaves. Zero-match and multi-match provider selection are unavailable/ambiguous without map-order dependence.
- [ ] Initialization order and shutdown order are correct and deterministic; failed init cleans its own partial resources; successful features shut down in reverse order, including through the implemented Tauri `RunEvent::Exit` adapter.
- [ ] Concurrent/stale requests cannot overwrite newer state.
- [ ] Failure propagation includes all transitive dependents; any affected `Required` feature makes startup/runtime fatal; `Degraded` is used only when every `Required` feature remains `Ready`; no `Required` feature remains `Ready` with a non-ready dependency/provider; no feature-level `Degraded` exists.
- [ ] Command results/durable state are authoritative; default progress-event failures are logged best-effort, while an explicitly required protocol event failure returns an error or typed partial success without rolling back committed state.
- [ ] No panic, unbounded loop, recursion, allocation, or timeout risk exists on untrusted input.

### 2. Dependency direction and boundaries

- [ ] The strict dependency allowlist is satisfied. Core has no host, feature-module, provider-implementation, frontend, or feature-specific imports.
- [ ] Modules do not import the host or another module's private files.
- [ ] Providers are independent leaves: they do not import host, feature internals, any other provider, or Tauri, and declare no provider-to-provider dependencies.
- [ ] Tauri types, `AppHandle`, event emission, and command macros remain in host/adapters.
- [ ] `src-tauri/src/provider_composition.rs` is the sole concrete mod/loader provider composition root; its shared `ProviderRuntimeState` owns both immutable registries and implements the feature provider catalog. The sole `LoaderKind` binding/orchestration lives in `src-tauri/modules/flint-feature-loaders/src/lib.rs`; `src-tauri/src/loaders.rs` is a thin progress-event adapter. No concrete provider construction is allowed in host commands or feature modules.
- [ ] The frontend composition root is the only place that imports concrete feature modules for assembly. Every feature import stays within its own subtree plus frontend core/shared; no cross-feature import, including a public entry, is allowed.
- [ ] No new Cargo or TypeScript dependency creates an avoidable cycle or expands a public layer unnecessarily. Cargo/TypeScript module systems alone do not enforce arbitrary allowlists; the strict Phase 5 `prepare` gate combines Cargo metadata, all-workspace source checks, typed provider ports, exact frontend adapters/importers, shim bans, and overlay/stale-request contracts.
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
- [ ] Provider-specific logic stays in provider/owning feature code and is not spread back into shared or host command code.
- [ ] Naming is clear, consistent, and original.
- [ ] Comments explain non-obvious invariants and safety decisions rather than restating code.
- [ ] Dead migration code and compatibility shims are absent; no duplicate old/new dispatch path remains.
- [ ] The change does not add premature runtime plugin machinery or present deferred future extensions as implemented.

### 6. Documentation and evidence

- [ ] Implemented and deferred future behavior are labeled correctly.
- [ ] Markdown links are relative and resolve within `docs/`.
- [ ] Documentation does not contain unrelated product names or copied external identifiers/prose.
- [ ] The completion report lists files, behavior/contract impact, risks, and the CI verification evidence.
- [ ] The reviewer did not claim or substitute local build/test execution for CI.

## Team delivery workflow

### Implementer

1. Read the task, contracts, architecture, standards, and completed phase matrix. Inspect the actual repository before deciding that a boundary exists.
2. Define the write scope and ownership boundary in the task or handoff notes.
3. Implement the smallest complete change that satisfies the task. Keep unrelated refactors out of scope.
4. Update tests, the affected authoritative contract, DTOs, client wrappers, events, and docs required by the change. Update both RPC contract files only when a shared surface is affected.
5. Hand off to an independent reviewer with a concise logic summary, changed-file list, risks, and the intended CI evidence.
6. Address reviewer findings in the same scoped change.
7. Use CI as the build/test authority. Do not report a check as passed unless CI actually ran it successfully or the task defines an equivalent authoritative result.
8. Commit completed work locally and do not push unless explicitly requested. A task that explicitly says not to commit is governed by its task instruction.
9. Report the local commit or uncommitted status, files changed, contract impact, CI status, and residual risks.

### Independent reviewer

- Review the logic, dependency direction, provider/feature lifecycle semantics, contract consistency, security behavior, and maintainability of the proposed change.
- Do not run local builds, tests, typechecks, formatters, linters, package commands, or `scripts/check-architecture.mjs`. Do not use local command output as review evidence.
- Do not rewrite the implementation unless the task explicitly reassigns implementation work.
- Report findings with file/line references, the violated rule, the failure scenario, and the expected behavior.
- Separate blocking defects from optional suggestions.
- Consult CI results for executable verification. If CI does not cover a required check, state that the check is unverified or pending rather than substituting a local command.
- Approve only when logic and boundaries are acceptable, deferred future work is clearly separated, and applicable contracts/docs are synchronized.

### CI authority and completion

- CI is the build/test authority for the change. The `flint-release.yml` release workflow `prepare` job installs frontend dependencies, runs `node --test scripts/test-architecture-checker.mjs scripts/test-frontend-contracts.mjs`, then runs `node scripts/check-architecture.mjs --phase 5`; all build-matrix jobs depend on `prepare`. The gate covers exact Tauri adapters/importers, overlay/host-service/stale-request/shim rules, exact nineteen-package Cargo metadata/internal+external dependency allowlists/cycles, all nineteen production source roots, typed `LoaderProvider`/`ModProvider` context ports and forbidden raw APIs, one shared actual-registry `ProviderRuntimeState`, exact provider source/composition edges, the sole `ModSource`/`LoaderKind` bindings, exact allowlists, version/lockfile coherence, and the host 30-command/28-feature-command gating map. A green local build or informal review does not replace CI evidence.
- CI failures go back to the implementer. The reviewer does not rerun the build, test suite, or architecture checker locally to diagnose them.
- Do not mark work complete while required CI checks are failing or uncollected. If the repository has no CI coverage for a required check, record that gap explicitly and add or schedule verification through the appropriate scoped work.
- Do not push, publish, tag, or release unless the task explicitly requests it.
- Keep the documentation index and implementation status current when a migration phase or accepted target changes.
