import type { AppPageContext } from "../../core/app-context";
import type { FeatureDescriptor, PageComponentProps } from "../../core/contributions";
import { AccountDialog } from "./AccountDialog";

const FEATURE_ID = "flint.accounts";
const SETTINGS_PAGE_ID = "flint.settings.page.settings";
const ACCOUNT_OVERLAY_ID = "flint.accounts.overlay.account-dialog";

function AccountDialogOverlay({ context }: PageComponentProps) {
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
      opener={context.services.opener}
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
    instanceActions: [],
    overlays: [
      {
        id: ACCOUNT_OVERLAY_ID,
        featureId: FEATURE_ID,
        order: 10,
        component: AccountDialogOverlay,
        visible: (context: AppPageContext) => context.accountDialogOpen
      }
    ]
  }
};

export default accountsFeature;
