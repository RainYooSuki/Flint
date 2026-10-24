# Integration Setup

Flint's Microsoft account and CurseForge integrations are optional. They are configured by the user at runtime and are not compiled into release builds.

## Microsoft account setup

Microsoft sign-in uses OAuth 2.0 Device Code Flow with the consumer Microsoft identity endpoint. Flint requests the `XboxLive.signin` and `offline_access` scopes. A client secret is not used.

### Azure application registration

1. Open the [Microsoft Entra admin center](https://entra.microsoft.com/) or the Azure portal and go to **App registrations**.
2. Create a registration whose supported account types include personal Microsoft accounts. The consumer device-code endpoint cannot be used by an app that only allows organizational accounts.
3. A redirect URI is not required for this device-code flow.
4. In **Authentication**, enable **Allow public client flows**.
5. Copy the **Application (client) ID**. Do not create a client secret for Flint.
6. In Flint, open **Settings > Integration services**, paste the application ID into **Microsoft application ID**, and save the settings.

### Signing in

1. Open the account dialog from the sidebar and choose Microsoft sign-in.
2. Flint displays a short user code and a Microsoft verification URL.
3. Open the URL, enter the code, and approve the request with the Microsoft account that owns Minecraft: Java Edition.
4. Flint polls the authorization endpoint and selects the new account when it succeeds.

The Microsoft account must own Minecraft: Java Edition and must not be blocked from Xbox Live by family, region, or account restrictions. If the access token expires, Flint refreshes it using the stored refresh token. If refresh fails, sign in again.

### Microsoft failures

- **Missing application ID:** Add the Azure public-client application ID in Settings before starting device-code sign-in.
- **Invalid public client:** Confirm the registration allows personal Microsoft accounts and has **Allow public client flows** enabled.
- **Account does not own Minecraft:** Use a Microsoft account with a Java Edition entitlement.
- **Xbox restrictions:** Child/family managed accounts or unavailable Xbox Live regions can prevent sign-in.
- **Expired session:** Sign in again if the refresh token is rejected or the stored account is incomplete.

## CurseForge setup

CurseForge mod search and version operations require a CurseForge API key. Modrinth does not require a key.

1. Sign in to the [CurseForge developer console](https://console.curseforge.com/).
2. Create or open an API key with access to Minecraft mods.
3. In Flint, open **Settings > Integration services**, paste the key into **CurseForge API Key**, and save the settings.
4. Open the Downloads view and select CurseForge as the source.

A missing or invalid key produces an actionable error in the CurseForge panel. Modrinth remains available without a key. API keys are subject to the provider's access, quota, and usage rules; rotate the key in Settings if it is revoked or expired.

## Local storage and privacy

Flint uses the platform application-data directory:

| Platform | Data directory |
| --- | --- |
| Windows | `%APPDATA%\Flint` |
| macOS | `~/Library/Application Support/Flint` |
| Linux | `${XDG_DATA_HOME:-~/.local/share}/Flint` |

Files in that directory:

- `settings.json` contains the selected game directory, Java path, launch settings, Microsoft application ID, and CurseForge API key.
- `accounts.json` contains the Microsoft account store, including access and refresh tokens, the active account, and profile details.

Both files are plain JSON. On Unix, Flint tightens both `accounts.json` and `settings.json` to owner-only (`0600`) permissions when reading or replacing them. Flint does not encrypt these files, so protect the operating-system account and do not share the data directory. Removing `accounts.json` clears the stored accounts; removing `settings.json` resets local settings.

The Microsoft application ID is a public client identifier, not a secret. The CurseForge API key, Microsoft refresh tokens, and Minecraft access tokens are sensitive. Flint sends the client ID and tokens only to the Microsoft/Xbox/Minecraft services needed for authentication, the CurseForge key only to the CurseForge API, and Modrinth requests only to the Modrinth API. No integration secret is embedded in a release binary.

## Current-instance context

### Mods and loaders

The Downloads view uses the currently selected instance and game directory:

- Mod search and version filtering use the selected instance's Minecraft version and loader. The current mod-discovery UI supports Fabric, Forge, NeoForge, and Quilt contexts.
- Mods are installed into `<gameDir>/mods`. Installed mods can be enabled, disabled, or deleted from the same game directory.
- Required dependencies are resolved recursively before install when requested. Optional and incompatible dependencies are reported but are not installed automatically. A required dependency without a safe downloadable file fails the plan.
- Loader installation uses the selected game directory, base version, loader version, and Java executable. If the base version is missing, Flint installs the vanilla metadata, client, libraries, and assets first.
- Forge, NeoForge, and OptiFine use Java installer flows. Fabric and Quilt use upstream profiles and libraries; LiteLoader uses its metadata/profile path. A working Java runtime is still needed to launch any of these instances.

Flint does not launch downloaded mod JARs during installation; trusted loader installer JARs are the only downloaded files run directly by Flint. Minecraft itself may load those mods or libraries later when a modded instance launches. A failed or unavailable upstream download is reported; Flint does not bypass an ad-gated page or execute an HTML response.

### Saves

The save manager also uses the currently selected instance and game directory:

- Saves are discovered under `<gameDir>/saves`.
- Backups are written under `<gameDir>/backups/flint-saves`.
- Export creates a ZIP at a destination chosen by the user.
- Import accepts a ZIP containing one save folder or a top-level `level.dat` and rejects unsafe or duplicate paths.
- Rename changes the save folder name; delete requires confirmation. There is no global save manager separate from the selected game directory.

## Prerequisites and common failures

- **Java is not bundled.** Select a valid Java executable in Settings or use Java discovery before launching or installing Forge, NeoForge, or OptiFine. Missing or invalid Java produces an installer or launch error.
- **Base version missing:** Loader installation tries to install the required vanilla version first. If Mojang services or the game directory are unavailable, retry after checking network access and disk permissions.
- **No matching loader version:** The selected game version may not have a loader build at the upstream service. Choose another loader version or game version.
- **OptiFine is ad-gated:** Upstream may offer only an ad-gated download page rather than a direct JAR. Flint reports the failure instead of passing the page to Java. Download and install OptiFine manually into the same game directory, then refresh local versions.
- **Download URL unavailable:** Provider files can be removed or temporarily unavailable. Retry, choose another version, or use the provider's project page to verify file availability.
- **Missing CurseForge key:** Configure a key in Settings. Modrinth is independent of this setting.
- **Missing Microsoft application ID:** Configure an Azure public-client application ID before Microsoft sign-in. Offline launch remains available.
- **Wrong game version or loader:** Mod and loader operations are tied to the selected instance. Select an instance with the intended Minecraft version and loader before searching or managing files.
- **Save import rejected:** Verify the ZIP contains one safe save folder or a top-level `level.dat`. Back up the current save before importing or deleting.