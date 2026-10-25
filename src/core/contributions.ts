import type { ComponentType } from "react";
import type { api } from "../api";
import type { IconName } from "../components/Icon";
import type { InstanceInfo, LauncherSettings, NoticeKind } from "../types";
import type { AppPageContext } from "./app-context";

const STABLE_ID_PATTERN = /^[a-z][a-z0-9-]*(?:\.[a-z][a-z0-9-]*)+$/;
const SEMANTIC_VERSION_PATTERN = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/;

export interface PageComponentProps {
  readonly context: AppPageContext;
}

export type PageComponent = ComponentType<PageComponentProps>;
export type SettingsSectionComponent = ComponentType<PageComponentProps>;

export interface NavigationContext {
  readonly settingsDirty: boolean;
  readonly coreReady: boolean;
  readonly selectedInstance: InstanceInfo | null;
}

export interface NavigationIndicator {
  readonly className: string;
  readonly title: string;
  readonly screenReaderLabel: string;
}

export interface PageContribution {
  readonly id: string;
  readonly featureId: string;
  readonly title: string;
  readonly order: number;
  readonly component: PageComponent;
}

export interface NavigationContribution {
  readonly id: string;
  readonly featureId: string;
  readonly label: string;
  readonly icon: IconName;
  readonly targetPageId: string;
  readonly order: number;
  readonly visible?: (context: NavigationContext) => boolean;
  readonly indicator?: (context: NavigationContext) => NavigationIndicator | null;
}

export interface SettingsSectionContribution {
  readonly id: string;
  readonly featureId: string;
  readonly title: string;
  readonly order: number;
  readonly component: SettingsSectionComponent;
  readonly settingsNamespace: string;
  readonly validate?: (settings: LauncherSettings) => readonly string[];
}

export interface InstanceActionContext {
  readonly selectedInstance: InstanceInfo | null;
  readonly gameDir: string;
  readonly navigateToPage: (targetPageId: string) => void;
  readonly notify: (kind: NoticeKind, message: string) => void;
  readonly api: typeof api;
}

export interface InstanceActionContribution {
  readonly id: string;
  readonly featureId: string;
  readonly label: string;
  readonly icon: IconName;
  readonly order: number;
  readonly visible?: (context: InstanceActionContext) => boolean;
  readonly enabled?: (context: InstanceActionContext) => boolean;
  readonly handler: (context: InstanceActionContext) => void | Promise<void>;
}

export interface OverlayContribution {
  readonly id: string;
  readonly featureId: string;
  readonly order: number;
  readonly component: PageComponent;
  readonly visible?: (context: AppPageContext) => boolean;
}

export interface FeatureContributions {
  readonly pages: readonly PageContribution[];
  readonly navigation: readonly NavigationContribution[];
  readonly settingsSections: readonly SettingsSectionContribution[];
  readonly instanceActions: readonly InstanceActionContribution[];
  readonly overlays: readonly OverlayContribution[];
}

export interface FeatureDescriptor {
  readonly id: string;
  readonly featureVersion: string;
  readonly requires: readonly string[];
  readonly backendRequires: readonly string[];
  readonly contributions: FeatureContributions;
}

export interface FrontendContributionRegistry {
  readonly features: readonly FeatureDescriptor[];
  readonly pages: readonly PageContribution[];
  readonly navigation: readonly NavigationContribution[];
  readonly settingsSections: readonly SettingsSectionContribution[];
  readonly instanceActions: readonly InstanceActionContribution[];
  readonly overlays: readonly OverlayContribution[];
  readonly pageById: Readonly<Record<string, PageContribution | undefined>>;
  readonly defaultPageId: string | null;
}

interface OrderedContribution {
  readonly id: string;
  readonly order: number;
}

interface IdentifiedContribution extends OrderedContribution {
  readonly featureId: string;
}

function validationError(message: string): never {
  throw new Error(`Invalid frontend contributions: ${message}`);
}

function assertStableId(value: unknown, label: string): asserts value is string {
  if (typeof value !== "string" || !STABLE_ID_PATTERN.test(value)) {
    validationError(`${label} must be a lowercase dot-separated stable ID.`);
  }
}

function assertNonEmptyString(value: unknown, label: string): asserts value is string {
  if (typeof value !== "string" || !value.trim()) {
    validationError(`${label} must be a non-empty string.`);
  }
}

function assertSemanticVersion(value: unknown, label: string): asserts value is string {
  if (typeof value !== "string" || !SEMANTIC_VERSION_PATTERN.test(value)) {
    validationError(`${label} must be a semantic version (for example 1.0.0).`);
  }
}

function assertFiniteOrder(value: unknown, label: string): asserts value is number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    validationError(`${label} must be a finite, non-negative number.`);
  }
}

function assertFunction(value: unknown, label: string): void {
  if (typeof value !== "function") {
    validationError(`${label} must be a function reference.`);
  }
}

function compareContributionOrder(left: OrderedContribution, right: OrderedContribution): number {
  if (left.order < right.order) return -1;
  if (left.order > right.order) return 1;
  if (left.id < right.id) return -1;
  if (left.id > right.id) return 1;
  return 0;
}

function compareStableId(left: { readonly id: string }, right: { readonly id: string }): number {
  if (left.id < right.id) return -1;
  if (left.id > right.id) return 1;
  return 0;
}

function assertContributionBase(
  feature: FeatureDescriptor,
  contribution: IdentifiedContribution,
  slot: string,
  contributionIds: ReadonlySet<string>
): void {
  assertStableId(contribution.id, `${slot} contribution ID`);
  if (contribution.featureId !== feature.id) {
    validationError(`${slot} contribution ${contribution.id} has featureId ${contribution.featureId}, expected ${feature.id}.`);
  }
  if (!contribution.id.startsWith(`${feature.id}.`)) {
    validationError(`${slot} contribution ${contribution.id} must be prefixed by its feature ID ${feature.id}.`);
  }
  assertFiniteOrder(contribution.order, `${slot} contribution ${contribution.id} order`);
  if (contributionIds.has(contribution.id)) {
    validationError(`contribution ID ${contribution.id} is duplicated across frontend slots.`);
  }
}

function assertFeatureContributionsShape(feature: FeatureDescriptor): void {
  if (!feature.contributions || typeof feature.contributions !== "object") {
    validationError(`feature ${feature.id} must declare a contributions object.`);
  }
  if (!Array.isArray(feature.contributions.pages)) {
    validationError(`feature ${feature.id} contributions.pages must be an array.`);
  }
  if (!Array.isArray(feature.contributions.navigation)) {
    validationError(`feature ${feature.id} contributions.navigation must be an array.`);
  }
  if (!Array.isArray(feature.contributions.settingsSections)) {
    validationError(`feature ${feature.id} contributions.settingsSections must be an array.`);
  }
  if (!Array.isArray(feature.contributions.instanceActions)) {
    validationError(`feature ${feature.id} contributions.instanceActions must be an array.`);
  }
  if (!Array.isArray(feature.contributions.overlays)) {
    validationError(`feature ${feature.id} contributions.overlays must be an array.`);
  }
}

/**
 * Validates descriptors before any registry is exposed to React.
 * IDs are global across every contribution slot, not just within one slot.
 */
export function validateFeatureDescriptors(features: readonly FeatureDescriptor[]): void {
  if (!Array.isArray(features)) {
    validationError("feature descriptor list must be an array.");
  }

  const featureIds = new Set<string>();
  const featuresById = new Map<string, FeatureDescriptor>();

  for (const feature of features) {
    if (!feature || typeof feature !== "object") {
      validationError("each feature descriptor must be an object.");
    }
    assertStableId(feature.id, "feature ID");
    if (featureIds.has(feature.id)) {
      validationError(`feature ID ${feature.id} is duplicated.`);
    }
    featureIds.add(feature.id);
    featuresById.set(feature.id, feature);
    assertSemanticVersion(feature.featureVersion, `feature ${feature.id} featureVersion`);
    assertFeatureContributionsShape(feature);

    if (!Array.isArray(feature.backendRequires)) {
      validationError(`feature ${feature.id} backendRequires must be an array.`);
    }
    const backendRequirements = new Set<string>();
    for (const requiredBackendId of feature.backendRequires) {
      assertStableId(requiredBackendId, `feature ${feature.id} backend requirement`);
      if (backendRequirements.has(requiredBackendId)) {
        validationError(`feature ${feature.id} declares duplicate backend requirement ${requiredBackendId}.`);
      }
      backendRequirements.add(requiredBackendId);
    }

    if (!Array.isArray(feature.requires)) {
      validationError(`feature ${feature.id} requires must be an array.`);
    }
    const requirements = new Set<string>();
    for (const requiredFeatureId of feature.requires) {
      assertStableId(requiredFeatureId, `feature ${feature.id} requirement`);
      if (requiredFeatureId === feature.id) {
        validationError(`feature ${feature.id} cannot require itself.`);
      }
      if (requirements.has(requiredFeatureId)) {
        validationError(`feature ${feature.id} declares duplicate requirement ${requiredFeatureId}.`);
      }
      requirements.add(requiredFeatureId);
    }
  }

  for (const feature of features) {
    for (const requiredFeatureId of feature.requires) {
      if (!featureIds.has(requiredFeatureId)) {
        validationError(`feature ${feature.id} requires unknown feature ${requiredFeatureId}.`);
      }
    }
  }

  const visiting = new Set<string>();
  const visited = new Set<string>();
  const visit = (featureId: string): void => {
    if (visited.has(featureId)) return;
    if (visiting.has(featureId)) {
      validationError(`feature requirement cycle detected at ${featureId}.`);
    }
    const feature = featuresById.get(featureId);
    if (!feature) {
      validationError(`feature requirement references unknown feature ${featureId}.`);
    }
    visiting.add(featureId);
    for (const requiredFeatureId of feature.requires) visit(requiredFeatureId);
    visiting.delete(featureId);
    visited.add(featureId);
  };
  for (const featureId of featureIds) visit(featureId);

  const contributionIds = new Set<string>();
  const pageIds = new Set<string>();

  for (const feature of features) {
    for (const page of feature.contributions.pages) {
      assertStableId(page.id, `page contribution ID owned by ${feature.id}`);
      if (pageIds.has(page.id)) {
        validationError(`page ID ${page.id} is duplicated.`);
      }
      pageIds.add(page.id);
      assertContributionBase(feature, page, "page", contributionIds);
      contributionIds.add(page.id);
      assertNonEmptyString(page.title, `page ${page.id} title`);
      assertFunction(page.component, `page ${page.id} component`);
    }

    for (const navigation of feature.contributions.navigation) {
      assertContributionBase(feature, navigation, "navigation", contributionIds);
      contributionIds.add(navigation.id);
      assertNonEmptyString(navigation.label, `navigation ${navigation.id} label`);
      assertNonEmptyString(navigation.icon, `navigation ${navigation.id} icon`);
      assertNonEmptyString(navigation.targetPageId, `navigation ${navigation.id} targetPageId`);
      if (navigation.visible !== undefined) {
        assertFunction(navigation.visible, `navigation ${navigation.id} visible selector`);
      }
      if (navigation.indicator !== undefined) {
        assertFunction(navigation.indicator, `navigation ${navigation.id} indicator selector`);
      }
    }

    for (const section of feature.contributions.settingsSections) {
      assertContributionBase(feature, section, "settings section", contributionIds);
      contributionIds.add(section.id);
      assertNonEmptyString(section.title, `settings section ${section.id} title`);
      assertNonEmptyString(section.settingsNamespace, `settings section ${section.id} settingsNamespace`);
      assertFunction(section.component, `settings section ${section.id} component`);
      if (section.validate !== undefined) {
        assertFunction(section.validate, `settings section ${section.id} validate hook`);
      }
    }

    for (const action of feature.contributions.instanceActions) {
      assertContributionBase(feature, action, "instance action", contributionIds);
      contributionIds.add(action.id);
      assertNonEmptyString(action.label, `instance action ${action.id} label`);
      assertNonEmptyString(action.icon, `instance action ${action.id} icon`);
      assertFunction(action.handler, `instance action ${action.id} handler`);
      if (action.visible !== undefined) {
        assertFunction(action.visible, `instance action ${action.id} visible selector`);
      }
      if (action.enabled !== undefined) {
        assertFunction(action.enabled, `instance action ${action.id} enabled selector`);
      }
    }

    for (const overlay of feature.contributions.overlays) {
      assertContributionBase(feature, overlay, "overlay", contributionIds);
      contributionIds.add(overlay.id);
      assertFunction(overlay.component, `overlay ${overlay.id} component`);
      if (overlay.visible !== undefined) {
        assertFunction(overlay.visible, `overlay ${overlay.id} visible selector`);
      }
    }
  }

  for (const feature of features) {
    for (const navigation of feature.contributions.navigation) {
      assertStableId(navigation.targetPageId, `navigation ${navigation.id} targetPageId`);
      if (!pageIds.has(navigation.targetPageId)) {
        validationError(`navigation ${navigation.id} targets unknown page ${navigation.targetPageId}.`);
      }
    }
  }
}

function freezeOrderedContributions<T extends OrderedContribution>(items: readonly T[]): readonly T[] {
  const ordered = [...items].sort(compareContributionOrder);
  return Object.freeze(ordered.map((item) => Object.freeze({ ...item }) as T));
}

function freezeFeature(feature: FeatureDescriptor): FeatureDescriptor {
  const contributions: FeatureContributions = Object.freeze({
    pages: freezeOrderedContributions(feature.contributions.pages),
    navigation: freezeOrderedContributions(feature.contributions.navigation),
    settingsSections: freezeOrderedContributions(feature.contributions.settingsSections),
    instanceActions: freezeOrderedContributions(feature.contributions.instanceActions),
    overlays: freezeOrderedContributions(feature.contributions.overlays)
  });

  return Object.freeze({
    id: feature.id,
    featureVersion: feature.featureVersion,
    requires: Object.freeze([...feature.requires]),
    backendRequires: Object.freeze([...feature.backendRequires]),
    contributions
  });
}

/**
 * Builds a deeply frozen registry after validating duplicate IDs, page
 * targets, descriptor references, and ordering.
 */
export function buildContributionRegistry(features: readonly FeatureDescriptor[]): FrontendContributionRegistry {
  validateFeatureDescriptors(features);

  const orderedFeatures = Object.freeze([...features].sort(compareStableId).map(freezeFeature));
  const pages = freezeOrderedContributions(orderedFeatures.flatMap((feature) => feature.contributions.pages));
  const navigation = freezeOrderedContributions(orderedFeatures.flatMap((feature) => feature.contributions.navigation));
  const settingsSections = freezeOrderedContributions(orderedFeatures.flatMap((feature) => feature.contributions.settingsSections));
  const instanceActions = freezeOrderedContributions(orderedFeatures.flatMap((feature) => feature.contributions.instanceActions));
  const overlays = freezeOrderedContributions(orderedFeatures.flatMap((feature) => feature.contributions.overlays));

  const pageById = Object.create(null) as Record<string, PageContribution | undefined>;
  for (const page of pages) pageById[page.id] = page;

  return Object.freeze({
    features: orderedFeatures,
    pages,
    navigation,
    settingsSections,
    instanceActions,
    overlays,
    pageById: Object.freeze(pageById),
    defaultPageId: pages[0]?.id ?? null
  });
}
