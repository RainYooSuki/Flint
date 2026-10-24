import type { FeatureDescriptor, PageComponentProps } from "../../core/contributions";
import { AccountDialog } from "./AccountDialog";

const FEATURE_ID = "flint.accounts";
const SETTINGS_PAGE_ID = "flint.settings.page.settings";

export function AccountDialogSurface({ context }: PageComponentProps) {
  return (
    <AccountDialog
      accounts={context.accounts}
      accountsLoading={context.accountsLoading}
      accountsError={context.accountsError}
      microsoftClientId={context.settings?.microsoftClientId ?? ""}
      onClose={context.onCloseAccountDialog}
      onOpenSettings={() => {
        context.onCloseAccountDialog();
        context.navigateToPage(SETTINGS_PAGE_ID);
      }}
      onAccountsChanged={context.onAccountsChanged}
      onRefreshAccounts={context.onRefreshAccounts}
      onNotify={context.onNotify}
    />
  );
}

export const accountsFeature: FeatureDescriptor = {
  id: FEATURE_ID,
  featureVersion: "1.0.0",
  backendRequires: ["flint.accounts"],
  requires: ["flint.settings"],
  contributions: {
    pages: [],
    navigation: [],
    settingsSections: [],
    instanceActions: []
  }
};

export default accountsFeature;
