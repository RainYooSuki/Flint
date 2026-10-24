import { buildContributionRegistry } from "../core/contributions";
import { aboutFeature } from "../modules/about";
import { accountsFeature } from "../modules/accounts";
import { launchFeature } from "../modules/launch";
import { savesFeature } from "../modules/saves";
import { settingsFeature } from "../modules/settings";
import { versionsFeature } from "../modules/versions";

export { AccountDialogSurface } from "../modules/accounts";

/** The single explicit static composition root for frontend features. */
export const frontendRegistry = Object.freeze(buildContributionRegistry([
  launchFeature,
  versionsFeature,
  savesFeature,
  settingsFeature,
  aboutFeature,
  accountsFeature
]));
