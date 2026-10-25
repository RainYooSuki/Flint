use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    Offline,
    Microsoft,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LoginStatus {
    Pending,
    Success,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub id: String,
    pub username: String,
    pub uuid: Option<String>,
    pub account_type: AccountType,
    pub active: bool,
    pub skin_url: Option<String>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MicrosoftDeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MicrosoftLoginPoll {
    pub status: LoginStatus,
    pub message: String,
    pub account: Option<AccountInfo>,
    pub slow_down: bool,
}

use flint_core::{net, paths};
use flint_feature_settings as settings;
use reqwest::{blocking::Client, StatusCode};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBOX_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MINECRAFT_LOGIN_URL: &str =
    "https://api.minecraftservices.com/authentication/login_with_xbox";
const MINECRAFT_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const OAUTH_SCOPE: &str = "XboxLive.signin offline_access";
const DEVICE_CODE_MAX_BODY: u64 = 1024 * 1024;
const AUTH_MAX_BODY: u64 = 2 * 1024 * 1024;
const TOKEN_REFRESH_SKEW_SECONDS: i64 = 60;

#[derive(Clone, Serialize, Deserialize)]
struct StoredMicrosoftAccount {
    id: String,
    username: String,
    uuid: String,
    #[serde(default)]
    skin_url: Option<String>,
    expires_at: i64,
    refresh_token: String,
    minecraft_access_token: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct AccountStore {
    #[serde(default = "offline_account_id")]
    active_account_id: String,
    #[serde(default)]
    accounts: Vec<StoredMicrosoftAccount>,
}

impl Default for AccountStore {
    fn default() -> Self {
        Self {
            active_account_id: offline_account_id(),
            accounts: Vec::new(),
        }
    }
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    #[serde(default)]
    verification_uri: String,
    #[serde(default)]
    verification_url: String,
    expires_in: u64,
    #[serde(default)]
    interval: Option<u64>,
    #[serde(default)]
    message: String,
}

#[derive(Deserialize)]
struct OAuthTokenResponse {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct XboxResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims", default)]
    display_claims: XboxDisplayClaims,
}

#[derive(Deserialize, Default)]
struct XboxDisplayClaims {
    #[serde(default)]
    xui: Vec<XboxUserClaim>,
}

#[derive(Deserialize)]
struct XboxUserClaim {
    #[serde(default)]
    uhs: String,
}

#[derive(Deserialize)]
struct MinecraftLoginResponse {
    access_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
}

#[derive(Deserialize)]
struct MinecraftProfileResponse {
    id: String,
    name: String,
    #[serde(default)]
    skins: Vec<MinecraftSkin>,
}

#[derive(Deserialize)]
struct MinecraftSkin {
    #[serde(default)]
    url: String,
}

struct MinecraftSession {
    access_token: String,
    expires_at: i64,
    username: String,
    uuid: String,
    skin_url: Option<String>,
}

pub enum LaunchIdentity {
    Offline {
        username: String,
    },
    Microsoft {
        username: String,
        uuid: String,
        access_token: String,
    },
}

fn offline_account_id() -> String {
    "offline".to_string()
}

fn account_store_path() -> PathBuf {
    paths::data_dir().join("accounts.json")
}

pub fn list_accounts() -> Result<Vec<AccountInfo>, String> {
    let settings = settings::load_settings()?;
    if !account_store_path().exists() {
        save_store(&AccountStore::default())?;
    }
    let mut store = load_store()?;
    store.ensure_valid_active();
    Ok(public_accounts(&store, &settings.username))
}

pub fn begin_microsoft_login() -> Result<MicrosoftDeviceCode, String> {
    let settings = settings::load_settings()?;
    let client_id = settings.microsoft_client_id.trim();
    if client_id.is_empty() {
        return Err(
            "Microsoft login requires a Microsoft application (client) ID in Settings.".to_string(),
        );
    }

    let client = auth_client()?;
    let (status, body) = post_form(
        &client,
        DEVICE_CODE_URL,
        &[("client_id", client_id), ("scope", OAUTH_SCOPE)],
    )?;
    if !status.is_success() {
        return Err(oauth_status_error(status, &body));
    }
    let response: DeviceCodeResponse = serde_json::from_slice(&body)
        .map_err(|error| format!("Microsoft returned an invalid device-code response: {error}"))?;
    if response.device_code.is_empty() || response.user_code.is_empty() {
        return Err("Microsoft returned an incomplete device-code response.".to_string());
    }
    let verification_uri = if !response.verification_uri.is_empty() {
        response.verification_uri
    } else {
        response.verification_url
    };
    if verification_uri.is_empty() {
        return Err("Microsoft did not return a verification URL.".to_string());
    }
    net::validate_https_url(&verification_uri)
        .map_err(|_| "Microsoft returned a non-HTTPS verification URL.".to_string())?;

    Ok(MicrosoftDeviceCode {
        device_code: response.device_code,
        user_code: response.user_code,
        verification_uri,
        expires_in: response.expires_in,
        interval: response.interval.unwrap_or(5).max(1),
        message: if response.message.is_empty() {
            "Open the verification URL and enter the displayed code.".to_string()
        } else {
            response.message
        },
    })
}

pub fn poll_microsoft_login(device_code: &str) -> Result<MicrosoftLoginPoll, String> {
    let device_code = device_code.trim();
    if device_code.is_empty() {
        return Err("The Microsoft device code is missing.".to_string());
    }
    let settings = settings::load_settings()?;
    let client_id = settings.microsoft_client_id.trim();
    if client_id.is_empty() {
        return Err(
            "Microsoft login requires a Microsoft application (client) ID in Settings.".to_string(),
        );
    }

    let client = match auth_client() {
        Ok(value) => value,
        Err(error) => return Ok(error_poll(error)),
    };
    let (status, body) = match post_form(
        &client,
        TOKEN_URL,
        &[
            ("client_id", client_id),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", device_code),
        ],
    ) {
        Ok(value) => value,
        Err(error) => return Ok(error_poll(error)),
    };
    let value: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => {
            return Ok(error_poll(
                "Microsoft returned an unreadable login response. Try again.".to_string(),
            ));
        }
    };

    if !status.is_success() {
        let code = value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown_error");
        return Ok(match code {
            "authorization_pending" => pending_poll("Waiting for Microsoft sign-in.", false),
            "slow_down" => pending_poll(
                "Microsoft asked the launcher to slow down. Keep waiting and try again shortly.",
                true,
            ),
            _ => error_poll(poll_error_message(code)),
        });
    }

    let token: OAuthTokenResponse = match serde_json::from_value(value) {
        Ok(value) => value,
        Err(_) => {
            return Ok(error_poll(
                "Microsoft returned an incomplete token response.".to_string(),
            ));
        }
    };
    if token.access_token.is_empty() {
        return Ok(error_poll(
            "Microsoft did not return an access token.".to_string(),
        ));
    }
    let Some(refresh_token) = token
        .refresh_token
        .clone()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(error_poll(
            "Microsoft did not return a refresh token. Please sign in again.".to_string(),
        ));
    };

    let session = match exchange_minecraft_session(&client, &token.access_token) {
        Ok(value) => value,
        Err(error) => return Ok(error_poll(error)),
    };
    let mut store = match load_store() {
        Ok(value) => value,
        Err(error) => return Ok(error_poll(error)),
    };
    store.upsert(StoredMicrosoftAccount {
        id: session.uuid.clone(),
        username: session.username.clone(),
        uuid: session.uuid.clone(),
        skin_url: session.skin_url.clone(),
        expires_at: session.expires_at,
        refresh_token,
        minecraft_access_token: session.access_token,
    });
    store.active_account_id = session.uuid.clone();
    if let Err(error) = save_store(&store) {
        return Ok(error_poll(format!(
            "Microsoft sign-in succeeded, but the account could not be saved: {error}"
        )));
    }

    Ok(MicrosoftLoginPoll {
        status: LoginStatus::Success,
        message: format!("Signed in as {}.", session.username),
        account: Some(account_info(
            &store,
            store
                .accounts
                .iter()
                .find(|account| account.id.eq_ignore_ascii_case(&session.uuid)),
        )),
        slow_down: false,
    })
}

pub fn select_account(account_id: &str) -> Result<Vec<AccountInfo>, String> {
    let account_id = account_id.trim();
    if account_id.is_empty() {
        return Err("Select a valid account.".to_string());
    }
    let settings = settings::load_settings()?;
    let mut store = load_store()?;
    if !account_id.eq_ignore_ascii_case("offline")
        && !store
            .accounts
            .iter()
            .any(|account| account.id.eq_ignore_ascii_case(account_id))
    {
        return Err("The selected Microsoft account no longer exists.".to_string());
    }
    store.active_account_id = if account_id.eq_ignore_ascii_case("offline") {
        offline_account_id()
    } else {
        store
            .accounts
            .iter()
            .find(|account| account.id.eq_ignore_ascii_case(account_id))
            .map(|account| account.id.clone())
            .ok_or_else(|| "The selected Microsoft account no longer exists.".to_string())?
    };
    save_store(&store)?;
    Ok(public_accounts(&store, &settings.username))
}

pub fn remove_account(account_id: &str) -> Result<Vec<AccountInfo>, String> {
    let account_id = account_id.trim();
    if account_id.eq_ignore_ascii_case("offline") {
        return Err("The built-in offline account cannot be removed.".to_string());
    }
    let settings = settings::load_settings()?;
    let mut store = load_store()?;
    let removed_active = store.active_account_id.eq_ignore_ascii_case(account_id);
    let original_len = store.accounts.len();
    store
        .accounts
        .retain(|account| !account.id.eq_ignore_ascii_case(account_id));
    if store.accounts.len() == original_len {
        return Err("The requested Microsoft account no longer exists.".to_string());
    }
    if removed_active {
        store.active_account_id = offline_account_id();
    }
    save_store(&store)?;
    Ok(public_accounts(&store, &settings.username))
}

pub fn refresh_active_account() -> Result<AccountInfo, String> {
    let settings = settings::load_settings()?;
    let mut store = load_store()?;
    store.ensure_valid_active();
    if store.active_account_id.eq_ignore_ascii_case("offline") {
        return Ok(offline_account_info(&store, &settings.username));
    }
    let client_id = settings.microsoft_client_id.trim();
    if client_id.is_empty() {
        return Err(
            "Microsoft account refresh requires a Microsoft application (client) ID in Settings."
                .to_string(),
        );
    }
    let index = store
        .accounts
        .iter()
        .position(|account| account.id.eq_ignore_ascii_case(&store.active_account_id))
        .ok_or_else(|| "The active Microsoft account could not be found.".to_string())?;
    let client = auth_client()?;
    let refreshed = refresh_microsoft_account(&client, client_id, &store.accounts[index])?;
    store.accounts[index] = refreshed;
    let active_id = store.active_account_id.clone();
    save_store(&store)?;
    let account = store
        .accounts
        .iter()
        .find(|account| account.id.eq_ignore_ascii_case(&active_id));
    Ok(account_info(&store, account))
}

pub fn resolve_launch_identity(offline_username: &str) -> Result<LaunchIdentity, String> {
    let mut store = load_store()?;
    store.ensure_valid_active();
    if store.active_account_id.eq_ignore_ascii_case("offline") {
        return Ok(LaunchIdentity::Offline {
            username: offline_username.to_string(),
        });
    }

    let index = store
        .accounts
        .iter()
        .position(|account| account.id.eq_ignore_ascii_case(&store.active_account_id))
        .ok_or_else(|| {
            "The active Microsoft account is missing. Select an account and try again.".to_string()
        })?;
    if now_unix() + TOKEN_REFRESH_SKEW_SECONDS >= store.accounts[index].expires_at {
        let settings = settings::load_settings()?;
        let client_id = settings.microsoft_client_id.trim();
        if client_id.is_empty() {
            return Err(
                "The active Microsoft session expired and no Microsoft application ID is configured. Re-authenticate the account."
                    .to_string(),
            );
        }
        let client = auth_client()?;
        let refreshed = refresh_microsoft_account(&client, client_id, &store.accounts[index])?;
        store.accounts[index] = refreshed;
        save_store(&store)?;
    }

    let account = &store.accounts[index];
    Ok(LaunchIdentity::Microsoft {
        username: account.username.clone(),
        uuid: account.uuid.clone(),
        access_token: account.minecraft_access_token.clone(),
    })
}

pub fn redact_secret(value: &str, secret: &str) -> String {
    if secret.is_empty() || secret == "0" {
        return value.to_string();
    }
    value.replace(secret, "[REDACTED]")
}

fn auth_client() -> Result<Client, String> {
    net::http_client(Duration::from_secs(60))
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn pending_poll(message: impl Into<String>, slow_down: bool) -> MicrosoftLoginPoll {
    MicrosoftLoginPoll {
        status: LoginStatus::Pending,
        message: message.into(),
        account: None,
        slow_down,
    }
}

fn error_poll(message: impl Into<String>) -> MicrosoftLoginPoll {
    MicrosoftLoginPoll {
        status: LoginStatus::Error,
        message: message.into(),
        account: None,
        slow_down: false,
    }
}

fn poll_error_message(code: &str) -> String {
    match code {
        "authorization_declined" => "Microsoft sign-in was declined.".to_string(),
        "expired_token" | "code_expired" => {
            "The Microsoft sign-in code expired. Start sign-in again.".to_string()
        }
        "bad_verification_code" | "invalid_grant" => {
            "Microsoft rejected the sign-in code. Start sign-in again.".to_string()
        }
        "access_denied" => "Microsoft access was denied.".to_string(),
        "invalid_client" => {
            "Microsoft rejected the configured application ID. Check Settings.".to_string()
        }
        other => format!("Microsoft sign-in failed ({other}). Please try again."),
    }
}

fn oauth_status_error(status: StatusCode, body: &[u8]) -> String {
    let value = serde_json::from_slice::<Value>(body).unwrap_or(Value::Null);
    let code = value
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("authentication_error");
    let message = poll_error_message(code);
    format!(
        "Microsoft authentication failed with HTTP {}: {message}",
        status.as_u16()
    )
}

fn post_form(
    client: &Client,
    url: &str,
    form: &[(&str, &str)],
) -> Result<(StatusCode, Vec<u8>), String> {
    let response = client
        .post(url)
        .form(form)
        .send()
        .map_err(|error| format!("Cannot contact Microsoft authentication: {error}"))?;
    let status = response.status();
    let body = net::read_body_limited(response, DEVICE_CODE_MAX_BODY)?;
    Ok((status, body))
}

fn post_json(client: &Client, url: &str, body: &Value) -> Result<(StatusCode, Value), String> {
    let encoded = serde_json::to_vec(body)
        .map_err(|error| format!("Cannot encode authentication request: {error}"))?;
    let response = client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(encoded)
        .send()
        .map_err(|error| format!("Cannot contact authentication service: {error}"))?;
    let status = response.status();
    let bytes = net::read_body_limited(response, AUTH_MAX_BODY)?;
    let value = serde_json::from_slice::<Value>(&bytes).unwrap_or_else(|_| json!({}));
    Ok((status, value))
}

fn exchange_minecraft_session(
    client: &Client,
    microsoft_token: &str,
) -> Result<MinecraftSession, String> {
    if microsoft_token.trim().is_empty() {
        return Err("Microsoft returned an empty access token.".to_string());
    }

    let (status, xbox) = post_json(
        client,
        XBOX_AUTH_URL,
        &json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={microsoft_token}")
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }),
    )?;
    if !status.is_success() {
        return Err("Xbox Live authentication failed. Try signing in again.".to_string());
    }
    let xbox: XboxResponse = serde_json::from_value(xbox)
        .map_err(|_| "Xbox Live returned an invalid authentication response.".to_string())?;
    let uhs = xbox
        .display_claims
        .xui
        .first()
        .map(|claim| claim.uhs.clone())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Xbox Live did not return a user identifier.".to_string())?;

    let (status, xsts_value) = post_json(
        client,
        XSTS_URL,
        &json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbox.token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }),
    )?;
    if !status.is_success() {
        let xerr = xsts_value
            .get("XErr")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        return Err(xsts_error_message(xerr));
    }
    let xsts: XboxResponse = serde_json::from_value(xsts_value)
        .map_err(|_| "Xbox security token service returned invalid JSON.".to_string())?;

    let (status, minecraft_value) = post_json(
        client,
        MINECRAFT_LOGIN_URL,
        &json!({
            "identityToken": format!("XBL3.0 x={uhs};{}", xsts.token)
        }),
    )?;
    if !status.is_success() {
        return Err(
            "Minecraft Services rejected the Xbox identity. Verify that this account owns Minecraft: Java Edition."
                .to_string(),
        );
    }
    let minecraft: MinecraftLoginResponse = serde_json::from_value(minecraft_value)
        .map_err(|_| "Minecraft Services returned an invalid login response.".to_string())?;
    if minecraft.access_token.is_empty() {
        return Err("Minecraft Services did not return an access token.".to_string());
    }

    let profile_response = client
        .get(MINECRAFT_PROFILE_URL)
        .bearer_auth(&minecraft.access_token)
        .send()
        .map_err(|error| format!("Cannot retrieve Minecraft profile: {error}"))?;
    let status = profile_response.status();
    let profile_body = net::read_body_limited(profile_response, AUTH_MAX_BODY)?;
    if status == StatusCode::NOT_FOUND {
        return Err("This Microsoft account does not own Minecraft: Java Edition.".to_string());
    }
    if !status.is_success() {
        return Err(format!(
            "Minecraft profile request failed with HTTP {}.",
            status.as_u16()
        ));
    }
    let profile: MinecraftProfileResponse = serde_json::from_slice(&profile_body)
        .map_err(|_| "Minecraft returned an invalid profile response.".to_string())?;
    if profile.id.len() != 32 || !profile.id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Minecraft returned an invalid profile UUID.".to_string());
    }

    Ok(MinecraftSession {
        access_token: minecraft.access_token,
        expires_at: now_unix() + minecraft.expires_in.unwrap_or(86_400) as i64,
        username: profile.name,
        uuid: profile.id.to_ascii_lowercase(),
        skin_url: profile
            .skins
            .into_iter()
            .map(|skin| skin.url)
            .find(|url| net::validate_https_url(url).is_ok()),
    })
}

fn refresh_microsoft_account(
    client: &Client,
    client_id: &str,
    account: &StoredMicrosoftAccount,
) -> Result<StoredMicrosoftAccount, String> {
    if account.refresh_token.trim().is_empty() {
        return Err(
            "The Microsoft account has no refresh token. Please sign in again.".to_string(),
        );
    }
    let (status, body) = post_form(
        client,
        TOKEN_URL,
        &[
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", &account.refresh_token),
            ("scope", OAUTH_SCOPE),
        ],
    )?;
    if !status.is_success() {
        let value = serde_json::from_slice::<Value>(&body).unwrap_or(Value::Null);
        let code = value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("refresh_failed");
        return Err(format!(
            "Microsoft session refresh failed ({}). Sign in again if the problem continues.",
            poll_error_message(code)
        ));
    }
    let token: OAuthTokenResponse = serde_json::from_slice(&body)
        .map_err(|_| "Microsoft returned an invalid refresh response.".to_string())?;
    if token.access_token.is_empty() {
        return Err("Microsoft did not return a refreshed access token.".to_string());
    }
    let session = exchange_minecraft_session(client, &token.access_token)?;
    Ok(StoredMicrosoftAccount {
        id: session.uuid.clone(),
        username: session.username,
        uuid: session.uuid,
        skin_url: session.skin_url,
        expires_at: session.expires_at,
        refresh_token: token
            .refresh_token
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| account.refresh_token.clone()),
        minecraft_access_token: session.access_token,
    })
}

fn xsts_error_message(error: u64) -> String {
    match error {
        2_148_916_233 => {
            "This Microsoft account has no Xbox profile. Create one at xbox.com and try again."
                .to_string()
        }
        2_148_916_235 => {
            "Xbox Live is not available in this account's country or region.".to_string()
        }
        2_148_916_238 => {
            "This Xbox account is managed by a child/family account and cannot sign in here."
                .to_string()
        }
        _ => "Xbox Live authorization was denied. Verify the account and try again.".to_string(),
    }
}

impl AccountStore {
    fn normalize(&mut self) {
        let mut seen = HashSet::new();
        self.accounts
            .retain(|account| seen.insert(account.id.to_ascii_lowercase()));
        self.ensure_valid_active();
    }

    fn ensure_valid_active(&mut self) {
        let active = self.active_account_id.trim();
        if active.is_empty()
            || (!active.eq_ignore_ascii_case("offline")
                && !self
                    .accounts
                    .iter()
                    .any(|account| account.id.eq_ignore_ascii_case(active)))
        {
            self.active_account_id = offline_account_id();
        } else if !active.eq_ignore_ascii_case("offline") {
            if let Some(account) = self
                .accounts
                .iter()
                .find(|account| account.id.eq_ignore_ascii_case(active))
            {
                self.active_account_id = account.id.clone();
            }
        }
    }

    fn upsert(&mut self, account: StoredMicrosoftAccount) {
        self.accounts
            .retain(|existing| !existing.id.eq_ignore_ascii_case(&account.id));
        self.accounts.push(account);
        self.accounts.sort_by(|left, right| {
            left.username
                .to_ascii_lowercase()
                .cmp(&right.username.to_ascii_lowercase())
        });
    }
}

fn public_accounts(store: &AccountStore, offline_username: &str) -> Vec<AccountInfo> {
    let mut accounts = Vec::with_capacity(store.accounts.len() + 1);
    accounts.push(offline_account_info(store, offline_username));
    for account in &store.accounts {
        accounts.push(account_info(store, Some(account)));
    }
    accounts
}

fn offline_account_info(store: &AccountStore, offline_username: &str) -> AccountInfo {
    AccountInfo {
        id: offline_account_id(),
        username: offline_username.to_string(),
        uuid: None,
        account_type: AccountType::Offline,
        active: store.active_account_id.eq_ignore_ascii_case("offline"),
        skin_url: None,
        expires_at: None,
    }
}

fn account_info(store: &AccountStore, account: Option<&StoredMicrosoftAccount>) -> AccountInfo {
    let Some(account) = account else {
        return AccountInfo {
            id: String::new(),
            username: String::new(),
            uuid: None,
            account_type: AccountType::Microsoft,
            active: false,
            skin_url: None,
            expires_at: None,
        };
    };
    AccountInfo {
        id: account.id.clone(),
        username: account.username.clone(),
        uuid: Some(account.uuid.clone()),
        account_type: AccountType::Microsoft,
        active: store.active_account_id.eq_ignore_ascii_case(&account.id),
        skin_url: account.skin_url.clone(),
        expires_at: Some(account.expires_at),
    }
}

fn load_store() -> Result<AccountStore, String> {
    load_store_from_path(&account_store_path())
}

fn load_store_from_path(path: &Path) -> Result<AccountStore, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    let text = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AccountStore::default());
        }
        Err(error) => {
            return Err(format!(
                "Cannot read the Microsoft account store ({}): {error}",
                path.display()
            ));
        }
    };
    let mut store: AccountStore = serde_json::from_str(&text)
        .map_err(|_| "The Microsoft account store is invalid. Sign in again.".to_string())?;
    store.normalize();
    Ok(store)
}

fn save_store(store: &AccountStore) -> Result<(), String> {
    save_store_to_path(&account_store_path(), store)
}

fn save_store_to_path(path: &Path, store: &AccountStore) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "The account store path has no parent directory.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Cannot create data directory ({}): {error}",
            parent.display()
        )
    })?;
    let bytes = serde_json::to_vec_pretty(store)
        .map_err(|error| format!("Cannot serialize the Microsoft account store: {error}"))?;
    let temp = path.with_file_name(format!(".accounts.{}.{}.tmp", process::id(), net::nonce()));

    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temp)
        .map_err(|error| format!("Cannot create temporary account store: {error}"))?;
    let write_result = (|| -> Result<(), String> {
        file.write_all(&bytes)
            .map_err(|error| format!("Cannot write the account store: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("Cannot flush the account store: {error}"))?;
        Ok(())
    })();
    drop(file);
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&temp, fs::Permissions::from_mode(0o600));
    }

    match fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        Err(first_error) => {
            if !path.exists() {
                let _ = fs::remove_file(&temp);
                return Err(format!(
                    "Cannot save the Microsoft account store: {first_error}"
                ));
            }
            let backup =
                path.with_file_name(format!(".accounts.{}.{}.bak", process::id(), net::nonce()));
            fs::rename(path, &backup).map_err(|error| {
                let _ = fs::remove_file(&temp);
                format!("Cannot replace the Microsoft account store: {error}")
            })?;
            match fs::rename(&temp, path) {
                Ok(()) => {
                    let _ = fs::remove_file(backup);
                    Ok(())
                }
                Err(error) => {
                    let _ = fs::rename(&backup, path);
                    let _ = fs::remove_file(&temp);
                    Err(format!("Cannot save the Microsoft account store: {error}"))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_account_is_first_and_active_by_default() {
        let store = AccountStore::default();
        let accounts = public_accounts(&store, "Alex");
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].id, "offline");
        assert_eq!(accounts[0].account_type, AccountType::Offline);
        assert!(accounts[0].active);
    }

    #[test]
    fn microsoft_account_uuids_are_upserted() {
        let mut store = AccountStore::default();
        let account = StoredMicrosoftAccount {
            id: "0123456789abcdef0123456789abcdef".to_string(),
            username: "Alex".to_string(),
            uuid: "0123456789abcdef0123456789abcdef".to_string(),
            skin_url: None,
            expires_at: 1,
            refresh_token: "refresh".to_string(),
            minecraft_access_token: "minecraft".to_string(),
        };
        store.upsert(account.clone());
        store.upsert(StoredMicrosoftAccount {
            username: "Alex2".to_string(),
            ..account
        });
        assert_eq!(store.accounts.len(), 1);
        assert_eq!(store.accounts[0].username, "Alex2");
    }

    #[test]
    fn duplicate_stored_accounts_are_normalized() {
        let account = StoredMicrosoftAccount {
            id: "0123456789abcdef0123456789abcdef".to_string(),
            username: "Alex".to_string(),
            uuid: "0123456789abcdef0123456789abcdef".to_string(),
            skin_url: None,
            expires_at: 1,
            refresh_token: "refresh".to_string(),
            minecraft_access_token: "minecraft".to_string(),
        };
        let mut store = AccountStore {
            active_account_id: account.id.clone(),
            accounts: vec![account.clone(), account],
        };
        store.normalize();
        assert_eq!(store.accounts.len(), 1);
        assert_eq!(store.active_account_id, store.accounts[0].id);
    }

    #[test]
    fn active_ids_fall_back_to_offline_when_missing() {
        let mut store = AccountStore {
            active_account_id: "missing".to_string(),
            accounts: Vec::new(),
        };
        store.ensure_valid_active();
        assert_eq!(store.active_account_id, "offline");
    }

    #[test]
    fn secrets_are_redacted_from_command_strings() {
        let command = "java --accessToken secret-token --username Alex";
        assert_eq!(
            redact_secret(command, "secret-token"),
            "java --accessToken [REDACTED] --username Alex"
        );
        assert_eq!(redact_secret(command, "0"), command);
    }

    #[test]
    fn slow_down_poll_requests_an_increased_interval() {
        assert!(pending_poll("slow down", true).slow_down);
        assert!(!error_poll("terminal error").slow_down);
    }

    #[test]
    fn account_api_shape_does_not_contain_tokens() {
        let value = serde_json::to_value(AccountInfo {
            id: "id".to_string(),
            username: "Alex".to_string(),
            uuid: Some("uuid".to_string()),
            account_type: AccountType::Microsoft,
            active: true,
            skin_url: None,
            expires_at: Some(1),
        })
        .expect("account serializes");
        assert!(value.get("accountType").is_some());
        assert!(value.get("accessToken").is_none());
        assert!(value.get("refreshToken").is_none());
    }

    #[test]
    fn account_store_round_trips_from_a_temp_file() {
        let directory = std::env::temp_dir().join(format!(
            "flint-auth-test-{}-{}",
            process::id(),
            net::nonce()
        ));
        fs::create_dir_all(&directory).expect("temp dir");
        let path = directory.join("accounts.json");
        let store = AccountStore::default();
        save_store_to_path(&path, &store).expect("save store");
        let loaded = load_store_from_path(&path).expect("load store");
        assert_eq!(loaded.active_account_id, "offline");
        assert!(loaded.accounts.is_empty());
        let _ = fs::remove_dir_all(directory);
    }
}
