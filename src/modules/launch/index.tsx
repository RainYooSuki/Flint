import type { FeatureDescriptor, InstanceActionContext, PageComponentProps } from "../../core/contributions";
import { LaunchView } from "./LaunchView";

const FEATURE_ID = "flint.launch";
const PAGE_ID = "flint.launch.page.launch";

function LaunchPage({ context }: PageComponentProps) {
  return (
    <LaunchView
      info={context.info}
      settings={context.settings}
      selectedInstance={context.selectedInstance}
      javas={context.javas}
      activeAccount={context.activeAccount}
      instancesLoading={context.instancesLoading}
      instancesError={context.instancesError}
      launching={context.launching}
      launchResult={context.launchResult}
      launchStartedAt={context.launchStartedAt}
      onLaunch={context.onLaunch}
      onGoVersions={() => context.navigateToPage("flint.versions.page.versions")}
      onGoSettings={() => context.navigateToPage("flint.settings.page.settings")}
      onGoTools={() => context.navigateToPage("flint.about.page.about")}
      onRefreshInstances={context.onRefreshInstances}
      onOpenPath={context.onOpenPath}
    />
  );
}

export const launchFeature: FeatureDescriptor = {
  id: FEATURE_ID,
  featureVersion: "1.0.0",
  requires: ["flint.accounts", "flint.settings"],
  contributions: {
    pages: [
      {
        id: PAGE_ID,
        featureId: FEATURE_ID,
        title: "启动",
        order: 10,
        component: LaunchPage
      }
    ],
    navigation: [
      {
        id: "flint.launch.navigation.launch",
        featureId: FEATURE_ID,
        label: "启动",
        icon: "play",
        targetPageId: PAGE_ID,
        order: 10
      }
    ],
    settingsSections: [],
    instanceActions: [
      {
        id: "flint.launch.instance-action.launch",
        featureId: FEATURE_ID,
        label: "启动所选版本",
        icon: "play",
        order: 10,
        visible: (context: InstanceActionContext) => context.selectedInstance !== null,
        handler: (context: InstanceActionContext) => {
          if (!context.selectedInstance) return;
          context.navigateToPage(PAGE_ID);
        }
      }
    ]
  }
};

export default launchFeature;
