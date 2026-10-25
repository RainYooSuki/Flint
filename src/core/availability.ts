import type {
  FeatureAvailability,
  FeatureLifecycleState,
  HostLifecycleState
} from "../types";
import type {
  FeatureDescriptor,
  FrontendContributionRegistry,
  PageContribution
} from "./contributions";

const STABLE_FEATURE_ID_PATTERN = /^[a-z][a-z0-9-]*(?:\.[a-z][a-z0-9-]*)+$/;
const HOST_LIFECYCLE_STATES: readonly HostLifecycleState[] = [
  "starting",
  "ready",
  "degraded",
  "startup_failed",
  "runtime_failed",
  "stopping",
  "stopped"
];
const FEATURE_LIFECYCLE_STATES: readonly FeatureLifecycleState[] = [
  "starting",
  "ready",
  "failed",
  "blocked_dependency",
  "stopped"
];
const MAX_REASON_LENGTH = 240;
const KNOWN_PROVIDER_IDS_PATTERN = /\b(?:curseforge|modrinth|forge|neoforge|fabric|quilt|liteloader|optifine)\b/gi;
const GENERAL_SCHEME_URL_PATTERN = /\b[a-z][a-z0-9+.-]*:\/\/\S+/gi;
const SINGLE_SLASH_FILE_PATH_PATTERN = /(^|[\s("'=])[^\s/\\]+[\\/][^\s/\\]*\.[a-z0-9]{1,8}(?=$|[\s)"',;])/gi;
const KNOWN_RELATIVE_PATH_PREFIX_PATTERN = /(^|[\s("'=])(?:saves|mods|versions|logs|backups|assets|libraries|instances|config|java|bin|home|users|tmp|var|etc|opt|usr|resources|run)[\\/][^\s)"',;]*/gi;

export interface AvailabilityEvaluation {
  readonly valid: boolean;
  readonly revision: number | null;
  readonly hostState: HostLifecycleState | null;
  readonly hostOperational: boolean;
  readonly lastFailure: string | null;
  readonly features: readonly FeatureAvailability[];
  readonly featureStates: Readonly<Record<string, FeatureLifecycleState | undefined>>;
}

export interface AvailabilityDiagnostic {
  readonly featureId: string;
  readonly reason: string | null;
}

interface OrderedContribution {
  readonly id: string;
  readonly order: number;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isStableFeatureId(value: unknown): value is string {
  return typeof value === "string" && STABLE_FEATURE_ID_PATTERN.test(value);
}

function isHostLifecycleState(value: unknown): value is HostLifecycleState {
  return HOST_LIFECYCLE_STATES.includes(value as HostLifecycleState);
}

function isFeatureLifecycleState(value: unknown): value is FeatureLifecycleState {
  return FEATURE_LIFECYCLE_STATES.includes(value as FeatureLifecycleState);
}

function emptyFeatureStates(): Readonly<Record<string, FeatureLifecycleState | undefined>> {
  return Object.freeze(Object.create(null) as Record<string, FeatureLifecycleState | undefined>);
}

function invalidAvailability(): AvailabilityEvaluation {
  return Object.freeze({
    valid: false,
    revision: null,
    hostState: null,
    hostOperational: false,
    lastFailure: null,
    features: Object.freeze([]),
    featureStates: emptyFeatureStates()
  });
}

export function sanitizeAvailabilityReason(reason: string | null): string | null {
  if (reason === null) return null;
  let text = reason.replace(/[\u0000-\u001f\u007f]+/g, " ").trim();
  if (!text) return null;

  text = text.replace(
    /\b(access[_-]?token|refresh[_-]?token|authorization|api[_-]?key|token)\b\s*[:=]\s*(?:"[^"]*"|'[^']*'|[^\s,;]+)/gi,
    (_match, label) => `${label}=[redacted]`
  );
  text = text.replace(/\bBearer\s+[A-Za-z0-9._~+/=-]+/gi, "Bearer [redacted]");
  text = text.replace(/\bprovider(?:\s+id)?\s*[:=]\s*['"`]?[a-z][a-z0-9._:-]*['"`]?/gi, "provider [redacted]");
  text = text.replace(/\bprovider\s+id\s+['"`]?[a-z][a-z0-9._:-]*['"`]?/gi, "provider [redacted]");
  text = text.replace(/\bprovider\s+['"`]?[a-z][a-z0-9-]*(?:[._:-][a-z0-9-]+)+['"`]?/gi, "provider [redacted]");
  text = text.replace(KNOWN_PROVIDER_IDS_PATTERN, "[provider]");
  text = text.replace(GENERAL_SCHEME_URL_PATTERN, "[url]");
  text = text.replace(/[A-Za-z]:\\[^\s)"',;]*/g, "[path]");
  text = text.replace(/[A-Za-z]:\/(?:[^\s/]+\/)*[^\s)"',;]*/g, "[path]");
  text = text.replace(/\\\\[^\\\s]+(?:\\[^\\\s]*)+/g, "[path]");
  text = text.replace(
    /(^|[\s("'=])\/(?:[^\s/]+\/)*[^\s)"',;]+/g,
    (_match, prefix) => `${prefix}[path]`
  );
  text = text.replace(
    /(^|[\s("'=])(?:[A-Za-z0-9._-]+[\\/]){2,}[^\s)"',;]*/g,
    (_match, prefix) => `${prefix}[path]`
  );
  text = text.replace(
    /(^|[\s("'=])(?:\.{1,2}|~)[\\/][^\s)"',;]*/g,
    (_match, prefix) => `${prefix}[path]`
  );
  text = text.replace(
    KNOWN_RELATIVE_PATH_PREFIX_PATTERN,
    (_match, prefix) => `${prefix}[path]`
  );
  text = text.replace(
    SINGLE_SLASH_FILE_PATH_PATTERN,
    (_match, prefix) => `${prefix}[path]`
  );

  const normalized = text.replace(/\s+/g, " ").trim();
  if (!normalized) return null;
  return normalized.length > MAX_REASON_LENGTH
    ? `${normalized.slice(0, MAX_REASON_LENGTH - 3)}...`
    : normalized;
}

/**
 * Validates an IPC snapshot defensively. A malformed or duplicate-ID snapshot
 * fails closed and grants no feature availability.
 */
export function evaluateAvailability(snapshot: unknown): AvailabilityEvaluation {
  if (!isRecord(snapshot)) return invalidAvailability();

  const revision = snapshot.revision;
  const hostState = snapshot.hostState;
  const lastFailure = snapshot.lastFailure;
  const featureEntries = snapshot.features;
  if (typeof revision !== "number" || !Number.isSafeInteger(revision) || revision < 0) return invalidAvailability();
  if (!isHostLifecycleState(hostState)) return invalidAvailability();
  if (lastFailure !== null && typeof lastFailure !== "string") return invalidAvailability();
  if (!Array.isArray(featureEntries)) return invalidAvailability();

  const featuresById = new Map<string, FeatureAvailability>();
  for (const entry of featureEntries) {
    if (!isRecord(entry)) return invalidAvailability();
    const featureId = entry.featureId;
    const state = entry.state;
    const reason = entry.reason;
    if (!isStableFeatureId(featureId) || !isFeatureLifecycleState(state) || (reason !== null && typeof reason !== "string")) {
      return invalidAvailability();
    }
    if (featuresById.has(featureId)) return invalidAvailability();
    featuresById.set(featureId, Object.freeze({ featureId, state, reason }) as FeatureAvailability);
  }

  const features = Object.freeze(
    [...featuresById.values()].sort((left, right) => {
      if (left.featureId < right.featureId) return -1;
      if (left.featureId > right.featureId) return 1;
      return 0;
    })
  );
  const featureStates = Object.create(null) as Record<string, FeatureLifecycleState | undefined>;
  for (const feature of features) featureStates[feature.featureId] = feature.state;

  return Object.freeze({
    valid: true,
    revision,
    hostState,
    hostOperational: hostState === "ready" || hostState === "degraded",
    lastFailure: sanitizeAvailabilityReason(lastFailure),
    features,
    featureStates: Object.freeze(featureStates)
  });
}

export function isBackendFeatureReady(evaluation: AvailabilityEvaluation, featureId: string): boolean {
  return evaluation.valid &&
    evaluation.hostOperational &&
    evaluation.featureStates[featureId] === "ready";
}

export function isFrontendFeatureAvailable(
  feature: FeatureDescriptor,
  evaluation: AvailabilityEvaluation
): boolean {
  if (!evaluation.valid || !evaluation.hostOperational || !Array.isArray(feature.backendRequires)) return false;
  return feature.backendRequires.every((featureId) => evaluation.featureStates[featureId] === "ready");
}

function compareContributionOrder(left: OrderedContribution, right: OrderedContribution): number {
  if (left.order < right.order) return -1;
  if (left.order > right.order) return 1;
  if (left.id < right.id) return -1;
  if (left.id > right.id) return 1;
  return 0;
}

function freezeOrderedContributions<T extends OrderedContribution>(items: readonly T[]): readonly T[] {
  return Object.freeze(
    [...items].sort(compareContributionOrder).map((item) => Object.freeze({ ...item }) as T)
  );
}

/**
 * Produces a deterministic, frozen availability view of the static registry.
 * Navigation is retained only when both its owner and target page are present.
 */
export function filterContributionRegistry(
  registry: FrontendContributionRegistry,
  evaluation: AvailabilityEvaluation
): FrontendContributionRegistry {
  const features = Object.freeze(
    registry.features
      .filter((feature) => isFrontendFeatureAvailable(feature, evaluation))
      .sort((left, right) => {
        if (left.id < right.id) return -1;
        if (left.id > right.id) return 1;
        return 0;
      })
      .map((feature) => Object.freeze({ ...feature }) as FeatureDescriptor)
  );
  const activeFeatureIds = new Set(features.map((feature) => feature.id));
  const pages = freezeOrderedContributions(
    registry.pages.filter((page) => activeFeatureIds.has(page.featureId))
  );
  const pageIds = new Set(pages.map((page) => page.id));
  const navigation = freezeOrderedContributions(
    registry.navigation.filter(
      (item) => activeFeatureIds.has(item.featureId) && pageIds.has(item.targetPageId)
    )
  );
  const settingsSections = freezeOrderedContributions(
    registry.settingsSections.filter((section) => activeFeatureIds.has(section.featureId))
  );
  const instanceActions = freezeOrderedContributions(
    registry.instanceActions.filter((action) => activeFeatureIds.has(action.featureId))
  );
  const overlays = freezeOrderedContributions(
    registry.overlays.filter((overlay) => activeFeatureIds.has(overlay.featureId))
  );

  const pageById = Object.create(null) as Record<string, PageContribution | undefined>;
  for (const page of pages) pageById[page.id] = page;

  return Object.freeze({
    features,
    pages,
    navigation,
    settingsSections,
    instanceActions,
    overlays,
    pageById: Object.freeze(pageById),
    defaultPageId: pages[0]?.id ?? null
  });
}

export function unavailableFeatureDiagnostics(
  evaluation: AvailabilityEvaluation,
  frontendFeatures: readonly FeatureDescriptor[] = []
): readonly AvailabilityDiagnostic[] {
  if (!evaluation.valid) return Object.freeze([]);
  const diagnostics = new Map<string, AvailabilityDiagnostic>();
  for (const feature of evaluation.features) {
    if (feature.state === "ready") continue;
    diagnostics.set(feature.featureId, Object.freeze({
      featureId: feature.featureId,
      reason: sanitizeAvailabilityReason(feature.reason)
    }) as AvailabilityDiagnostic);
  }
  for (const feature of frontendFeatures) {
    if (!Array.isArray(feature.backendRequires)) continue;
    for (const featureId of feature.backendRequires) {
      if (evaluation.featureStates[featureId] !== "ready" && !diagnostics.has(featureId)) {
        diagnostics.set(featureId, Object.freeze({ featureId, reason: null }) as AvailabilityDiagnostic);
      }
    }
  }
  return Object.freeze(
    [...diagnostics.values()].sort((left, right) => {
      if (left.featureId < right.featureId) return -1;
      if (left.featureId > right.featureId) return 1;
      return 0;
    })
  );
}
