import type { FeatureDescriptor, PageComponentProps } from "../../core/contributions";
import { VersionsView } from "./VersionsView";

const FEATURE_ID = "flint.versions";
const PAGE_ID = "flint.versions.page.versions";

function VersionsPage({ context }: PageComponentProps) {
  return (
    <VersionsView
      instances={context.instances}
      selectedId={context.selectedId}
      instancesLoading={context.instancesLoading}
      instancesError={context.instancesError}
      remoteVersions={context.remoteVersions}
      remoteLoading={context.remoteLoading}
      remoteError={context.remoteError}
      remoteFetched={context.remoteFetched}
      installRuntime={context.installRuntime}
      installResult={context.installResult}
      gameDirLabel={context.gameDirLabel}
      gameDirPath={context.gameDirPath}
      settings={context.settings}
      instanceActions={context.instanceActions}
      onRefreshInstances={context.onRefreshInstances}
      onSelectInstance={context.onSelectInstance}
      onFetchRemote={context.onFetchRemote}
      onInstall={context.onInstall}
      onOpenPath={context.onOpenPath}
      onNavigateToPage={context.navigateToPage}
      onGoSettings={() => context.navigateToPage("flint.settings.page.settings")}
      onNotify={context.onNotify}
    />
  );
}

export const versionsFeature: FeatureDescriptor = {
  id: FEATURE_ID,
  featureVersion: "1.0.0",
  requires: ["flint.settings"],
  contributions: {
    pages: [
      {
        id: PAGE_ID,
        featureId: FEATURE_ID,
        title: "下载",
        order: 20,
        component: VersionsPage
      }
    ],
    navigation: [
      {
        id: "flint.versions.navigation.versions",
        featureId: FEATURE_ID,
        label: "下载",
        icon: "download",
        targetPageId: PAGE_ID,
        order: 20
      }
    ],
    settingsSections: [],
    instanceActions: []
  }
};

export default versionsFeature;
