import type { FeatureDescriptor, NavigationContext, PageComponentProps } from "../../core/contributions";
import { SettingsView } from "./SettingsView";

const FEATURE_ID = "flint.settings";
const PAGE_ID = "flint.settings.page.settings";

function SettingsPage({ context }: PageComponentProps) {
  return (
    <SettingsView
      draft={context.settingsDraft}
      saved={context.settings}
      javas={context.javas}
      settingsLoading={context.settingsLoading}
      javaLoading={context.javaLoading}
      settingsError={context.settingsError}
      javaError={context.javaError}
      saving={context.savingSettings}
      dirty={context.settingsDirty}
      onChange={context.onChangeSettings}
      onSave={context.onSaveSettings}
      onReload={context.onReloadSettings}
      onReset={context.onResetSettings}
      onPickGameDir={context.onPickGameDir}
      onPickJava={context.onPickJava}
      onScanJava={context.onScanJava}
    />
  );
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
    settingsSections: [],
    instanceActions: []
  }
};

export default settingsFeature;
