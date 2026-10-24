import type { FeatureDescriptor, NavigationContext, PageComponentProps } from "../../core/contributions";
import type { LauncherSettings } from "../../types";
import { validateSettings, validateUsername } from "../../utils";
import { AccountSettingsSection } from "./sections/AccountSettingsSection";
import { IntegrationsSettingsSection } from "./sections/IntegrationsSettingsSection";
import { JavaSettingsSection } from "./sections/JavaSettingsSection";
import { PerformanceSettingsSection } from "./sections/PerformanceSettingsSection";
import { ArgumentsSettingsSection } from "./sections/ArgumentsSettingsSection";
import { SettingsView } from "./SettingsView";

const FEATURE_ID = "flint.settings";
const PAGE_ID = "flint.settings.page.settings";
const ACCOUNT_SECTION_ID = "flint.settings.settings-section.game-account";
const INTEGRATIONS_SECTION_ID = "flint.settings.settings-section.integrations";
const JAVA_SECTION_ID = "flint.settings.settings-section.java";
const PERFORMANCE_SECTION_ID = "flint.settings.settings-section.performance";
const ARGUMENTS_SECTION_ID = "flint.settings.settings-section.arguments";

function validateAccountSettings(settings: LauncherSettings): readonly string[] {
  const message = validateUsername(settings.username);
  return message ? [message] : [];
}

// The central settings validator currently defines no errors for the integrations, Java, and launch-argument sections.
function validateIntegrationsSettings(_settings: LauncherSettings): readonly string[] {
  return [];
}

function validateJavaSettings(_settings: LauncherSettings): readonly string[] {
  return [];
}

function validatePerformanceSettings(settings: LauncherSettings): readonly string[] {
  const errors = validateSettings(settings);
  return [
    errors.minMemoryMb,
    errors.maxMemoryMb,
    errors.width,
    errors.height
  ].filter((message): message is string => Boolean(message));
}

function validateArgumentsSettings(_settings: LauncherSettings): readonly string[] {
  return [];
}

function SettingsPage({ context }: PageComponentProps) {
  return <SettingsView context={context} />;
}

export const settingsFeature: FeatureDescriptor = {
  id: FEATURE_ID,
  featureVersion: "1.0.0",
  requires: [],
  contributions: {
    pages: [
      {
        id: PAGE_ID,
        featureId: FEATURE_ID,
        title: "设置",
        order: 40,
        component: SettingsPage
      }
    ],
    navigation: [
      {
        id: "flint.settings.navigation.settings",
        featureId: FEATURE_ID,
        label: "设置",
        icon: "settings",
        targetPageId: PAGE_ID,
        order: 40,
        indicator: ({ settingsDirty }: NavigationContext) => settingsDirty
          ? {
              className: "nav-dirty",
              title: "设置有未保存更改",
              screenReaderLabel: "设置有未保存更改"
            }
          : null
      }
    ],
    settingsSections: [
      {
        id: ACCOUNT_SECTION_ID,
        featureId: FEATURE_ID,
        title: "游戏与账户",
        order: 10,
        component: AccountSettingsSection,
        settingsNamespace: "flint.settings.game-account",
        validate: validateAccountSettings
      },
      {
        id: INTEGRATIONS_SECTION_ID,
        featureId: FEATURE_ID,
        title: "集成服务",
        order: 20,
        component: IntegrationsSettingsSection,
        settingsNamespace: "flint.settings.integrations",
        validate: validateIntegrationsSettings
      },
      {
        id: JAVA_SECTION_ID,
        featureId: FEATURE_ID,
        title: "Java 环境",
        order: 30,
        component: JavaSettingsSection,
        settingsNamespace: "flint.settings.java",
        validate: validateJavaSettings
      },
      {
        id: PERFORMANCE_SECTION_ID,
        featureId: FEATURE_ID,
        title: "内存与窗口",
        order: 40,
        component: PerformanceSettingsSection,
        settingsNamespace: "flint.settings.performance",
        validate: validatePerformanceSettings
      },
      {
        id: ARGUMENTS_SECTION_ID,
        featureId: FEATURE_ID,
        title: "启动参数",
        order: 50,
        component: ArgumentsSettingsSection,
        settingsNamespace: "flint.settings.arguments",
        validate: validateArgumentsSettings
      }
    ],
    instanceActions: []
  }
};

export default settingsFeature;
