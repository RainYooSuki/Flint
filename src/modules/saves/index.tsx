import type { FeatureDescriptor, PageComponentProps } from "../../core/contributions";
import { SaveManagerView } from "./SaveManagerView";

const FEATURE_ID = "flint.saves";
const PAGE_ID = "flint.saves.page.saves";

function SavesPage({ context }: PageComponentProps) {
  return (
    <SaveManagerView
      gameDir={context.settings?.gameDir ?? ""}
      gameDirPath={context.gameDirPath}
      gameDirLabel={context.gameDirLabel}
      selectedInstance={context.selectedInstance}
      onOpenPath={context.onOpenPath}
      onNotify={context.onNotify}
    />
  );
}

export const savesFeature: FeatureDescriptor = {
  id: FEATURE_ID,
  featureVersion: "1.0.0",
  backendRequires: ["flint.saves"],
  requires: ["flint.settings"],
  contributions: {
    pages: [
      {
        id: PAGE_ID,
        featureId: FEATURE_ID,
        title: "存档",
        order: 30,
        component: SavesPage
      }
    ],
    navigation: [
      {
        id: "flint.saves.navigation.saves",
        featureId: FEATURE_ID,
        label: "存档",
        icon: "save",
        targetPageId: PAGE_ID,
        order: 30
      }
    ],
    settingsSections: [],
    instanceActions: []
  }
};

export default savesFeature;
