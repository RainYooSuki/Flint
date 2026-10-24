import type { FeatureDescriptor, PageComponentProps } from "../../core/contributions";
import { AboutView } from "./AboutView";

const FEATURE_ID = "flint.about";
const PAGE_ID = "flint.about.page.about";

function AboutPage({ context }: PageComponentProps) {
  return (
    <AboutView
      info={context.info}
      infoLoading={context.infoLoading}
      infoError={context.infoError}
      settings={context.settings}
      instancesCount={context.instances.length}
      javaCount={context.javas.length}
      settingsDirty={context.settingsDirty}
      onOpenPath={context.onOpenPath}
      onRetryInfo={context.onRetryInfo}
      onGoSettings={() => context.navigateToPage("flint.settings.page.settings")}
      onGoVersions={() => context.navigateToPage("flint.versions.page.versions")}
    />
  );
}

export const aboutFeature: FeatureDescriptor = {
  id: FEATURE_ID,
  featureVersion: "1.0.0",
  requires: [],
  contributions: {
    pages: [
      {
        id: PAGE_ID,
        featureId: FEATURE_ID,
        title: "工具",
        order: 50,
        component: AboutPage
      }
    ],
    navigation: [
      {
        id: "flint.about.navigation.about",
        featureId: FEATURE_ID,
        label: "工具",
        icon: "wrench",
        targetPageId: PAGE_ID,
        order: 50
      }
    ],
    settingsSections: [],
    instanceActions: []
  }
};

export default aboutFeature;
