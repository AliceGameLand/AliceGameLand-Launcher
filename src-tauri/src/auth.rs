use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::{Client, RequestBuilder, Url};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tokio::sync::oneshot;

const IDENTITY_ORIGIN: &str = "https://login.microsoftonline.com";
const REDIRECT: &str = "https://login.microsoftonline.com/common/oauth2/nativeclient";
const TOKEN_ENDPOINT: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const SCOPE: &str = "XboxLive.signin offline_access";
const WINDOW_LABEL: &str = "microsoft-login";
const CREDENTIAL_SERVICE: &str = "kr.dreamcraft.alicegameland.launcher";
const CREDENTIAL_USER: &str = "microsoft-tauri";
const CLIENT_CONFIG: &str = include_str!("../../app/assets/config/auth.json");
static LOGIN: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

type Result<T> = std::result::Result<T, String>;
type LoginCallback = Arc<Mutex<Option<oneshot::Sender<Result<String>>>>>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientConfig {
    azure_client_id: String,
}

#[derive(Deserialize, Serialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub(crate) skins: Vec<ProfileSkin>,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct ProfileSkin {
    state: String,
    url: String,
}

pub struct GameSession {
    pub profile: Profile,
    pub access_token: String,
    pub client_id: String,
    pub xuid: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct SavedAccount {
    profile: Profile,
    refresh_token: String,
}

#[derive(Deserialize)]
struct MicrosoftToken {
    access_token: String,
    refresh_token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XboxToken {
    token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XstsToken {
    token: String,
    display_claims: DisplayClaims,
}

#[derive(Deserialize)]
struct DisplayClaims {
    xui: Vec<UserClaim>,
}

#[derive(Deserialize)]
struct UserClaim {
    uhs: String,
    xid: Option<String>,
}

#[derive(Deserialize)]
struct MinecraftToken {
    access_token: String,
}

fn random_parameter() -> Result<String> {
    let mut random = [0; 32];
    getrandom::fill(&mut random).map_err(|_| "로그인 보안 값을 생성하지 못했어요.")?;
    Ok(URL_SAFE_NO_PAD.encode(random))
}

fn handle_redirect(url: &Url, expected_state: &str) -> Option<Result<String>> {
    if url.origin().ascii_serialization() != IDENTITY_ORIGIN
        || url.path() != "/common/oauth2/nativeclient"
    {
        return None;
    }

    let mut state = None;
    let mut code = None;
    for (key, value) in url.query_pairs() {
        let destination = match key.as_ref() {
            "state" => &mut state,
            "code" => &mut code,
            _ => continue,
        };
        if destination.replace(value.into_owned()).is_some() {
            return Some(Err("로그인 응답이 올바르지 않아요.".into()));
        }
    }

    if state.as_deref() != Some(expected_state) {
        return Some(Err("로그인 요청을 확인하지 못했어요.".into()));
    }
    Some(
        code.filter(|value| !value.is_empty())
            .ok_or_else(|| "Microsoft 로그인이 완료되지 않았어요.".into()),
    )
}

fn send_once(callback: &LoginCallback, result: Result<String>) {
    if let Ok(mut pending) = callback.lock() {
        if let Some(sender) = pending.take() {
            let _ = sender.send(result);
        }
    }
}

async fn authorize(app: &AppHandle, client_id: &str) -> Result<(String, String)> {
    let state = random_parameter()?;
    let verifier = random_parameter()?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut url = Url::parse("https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize")
        .map_err(|_| "로그인 주소가 올바르지 않아요.")?;
    url.query_pairs_mut().extend_pairs([
        ("client_id", client_id),
        ("response_type", "code"),
        ("response_mode", "query"),
        ("redirect_uri", REDIRECT),
        ("scope", SCOPE),
        ("prompt", "select_account"),
        ("state", &state),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
    ]);

    let (sender, receiver) = oneshot::channel();
    let callback = Arc::new(Mutex::new(Some(sender)));
    let navigation = Arc::clone(&callback);
    // This remote window has no launcher capability or access to account storage.
    let mut window = WebviewWindowBuilder::new(app, WINDOW_LABEL, WebviewUrl::External(url))
        .title("Microsoft 로그인")
        .inner_size(520.0, 600.0)
        .incognito(true)
        .on_navigation(move |url| {
            if let Some(result) = handle_redirect(url, &state) {
                send_once(&navigation, result);
                return false;
            }
            url.scheme() == "https"
        });
    let test_page_loaded = if app.state::<crate::Launcher>().verify_oauth {
        let loaded = Arc::new(tokio::sync::Notify::new());
        let page = Arc::clone(&loaded);
        window = window.on_page_load(move |_, event| {
            if event.event() == tauri::webview::PageLoadEvent::Finished
                && event.url().scheme() == "https"
                && event.url().host_str() == Some("login.live.com")
            {
                page.notify_one();
            }
        });
        Some(loaded)
    } else {
        None
    };
    let window = window
        .build()
        .map_err(|_| "Microsoft 로그인 창을 열지 못했어요.")?;
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            send_once(&callback, Err("Microsoft 로그인 창이 닫혔어요.".into()));
        }
    });
    if let Some(loaded) = test_page_loaded {
        let result = tokio::time::timeout(Duration::from_secs(30), loaded.notified()).await;
        let _ = window.destroy();
        result.map_err(|_| "Microsoft 로그인 페이지를 확인하지 못했어요.")?;
    }
    let response = tokio::time::timeout(Duration::from_secs(600), receiver).await;
    if app.get_webview_window(WINDOW_LABEL).is_some() {
        let _ = window.destroy();
    }
    let code = response
        .map_err(|_| "Microsoft 로그인 시간이 초과됐어요.")?
        .map_err(|_| "Microsoft 로그인이 중단됐어요.")??;
    Ok((code, verifier))
}

async fn read_response<T: DeserializeOwned>(request: RequestBuilder, stage: &str) -> Result<T> {
    let response = request
        .header("accept", "application/json")
        .send()
        .await
        .map_err(|_| format!("{stage} 요청에 연결하지 못했어요."))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("{stage}에 실패했어요. (HTTP {})", status.as_u16()));
    }
    response
        .json()
        .await
        .map_err(|_| format!("{stage} 응답이 올바르지 않아요."))
}

fn login_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "로그인 네트워크를 준비하지 못했어요.".into())
}

fn client_id() -> Result<String> {
    let config: ClientConfig = serde_json::from_str(CLIENT_CONFIG)
        .map_err(|_| "Microsoft 로그인 설정을 읽지 못했어요.")?;
    Ok(std::env::var("ALICE_MS_CLIENT_ID").unwrap_or(config.azure_client_id))
}

async fn exchange(
    client: &Client,
    client_id: String,
    microsoft_access_token: &str,
) -> Result<GameSession> {
    let xbox: XboxToken = read_response(
        client.post("https://user.auth.xboxlive.com/user/authenticate").json(&json!({
            "Properties": {"AuthMethod":"RPS", "SiteName":"user.auth.xboxlive.com", "RpsTicket":format!("d={microsoft_access_token}")},
            "RelyingParty":"http://auth.xboxlive.com", "TokenType":"JWT"
        })),
        "Xbox Live 인증",
    ).await?;
    let xsts: XstsToken = read_response(
        client
            .post("https://xsts.auth.xboxlive.com/xsts/authorize")
            .json(&json!({
                "Properties": {"SandboxId":"RETAIL", "UserTokens":[xbox.token]},
                "RelyingParty":"rp://api.minecraftservices.com/", "TokenType":"JWT"
            })),
        "Xbox 보안 토큰 확인",
    )
    .await?;
    let claim = xsts
        .display_claims
        .xui
        .first()
        .ok_or("Xbox 사용자 정보를 확인하지 못했어요.")?;
    let minecraft: MinecraftToken = read_response(
        client
            .post("https://api.minecraftservices.com/authentication/login_with_xbox")
            .json(&json!({
                "identityToken":format!("XBL3.0 x={};{}", claim.uhs, xsts.token)
            })),
        "Minecraft 인증",
    )
    .await?;
    let profile = read_response(
        client
            .get("https://api.minecraftservices.com/minecraft/profile")
            .bearer_auth(&minecraft.access_token),
        "Minecraft 프로필 확인",
    )
    .await?;
    Ok(GameSession {
        profile,
        access_token: minecraft.access_token,
        xuid: claim.xid.clone(),
        client_id,
    })
}

fn credential() -> Result<keyring::Entry> {
    let user = match crate::test_profile().map_err(|_| "테스트 프로필을 확인하지 못했어요.")?
    {
        Some(path) => format!(
            "test-{:x}",
            Sha256::digest(path.as_os_str().as_encoded_bytes())
        ),
        None => CREDENTIAL_USER.to_owned(),
    };
    keyring::Entry::new(CREDENTIAL_SERVICE, &user)
        .map_err(|_| "운영체제의 계정 보관함을 열지 못했어요.".into())
}

async fn load_account() -> Result<Option<SavedAccount>> {
    tauri::async_runtime::spawn_blocking(|| match credential()?.get_password() {
        Ok(saved) => serde_json::from_str(&saved)
            .map(Some)
            .map_err(|_| "저장된 계정 정보를 읽지 못했어요.".into()),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err("운영체제의 계정 보관함을 읽지 못했어요.".into()),
    })
    .await
    .map_err(|_| "계정 정보 확인이 중단됐어요.")?
}

async fn save_account(account: SavedAccount) -> Result<Profile> {
    tauri::async_runtime::spawn_blocking(move || {
        let stored =
            serde_json::to_string(&account).map_err(|_| "계정 정보를 저장하지 못했어요.")?;
        credential()?
            .set_password(&stored)
            .map_err(|_| "운영체제의 계정 보관함에 저장하지 못했어요.")?;
        Ok(account.profile)
    })
    .await
    .map_err(|_| "계정 저장이 중단됐어요.")?
}

#[tauri::command]
pub async fn account_profile() -> Result<Option<Profile>> {
    Ok(load_account().await?.map(|account| account.profile))
}

#[tauri::command]
pub async fn microsoft_sign_in(app: AppHandle) -> Result<Profile> {
    let _login = LOGIN
        .try_lock()
        .map_err(|_| "이미 Microsoft 로그인이 진행 중이에요.")?;
    let client_id = client_id()?;
    let (code, verifier) = authorize(&app, &client_id).await?;
    let client = login_client()?;
    let microsoft: MicrosoftToken = read_response(
        client.post(TOKEN_ENDPOINT).form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id.as_str()),
            ("code", &code),
            ("code_verifier", &verifier),
            ("redirect_uri", REDIRECT),
            ("scope", SCOPE),
        ]),
        "Microsoft 인증",
    )
    .await?;
    let session = exchange(&client, client_id, &microsoft.access_token).await?;
    save_account(SavedAccount {
        profile: session.profile,
        refresh_token: microsoft.refresh_token,
    })
    .await
}

#[tauri::command]
pub async fn microsoft_sign_out() -> Result<()> {
    let _login = LOGIN
        .try_lock()
        .map_err(|_| "이미 Microsoft 로그인이 진행 중이에요.")?;
    tauri::async_runtime::spawn_blocking(|| match credential()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("운영체제의 계정 보관함에서 계정을 삭제하지 못했어요.".into()),
    })
    .await
    .map_err(|_| "로그아웃이 중단됐어요.")?
}
pub async fn game_session() -> Result<GameSession> {
    let _login = LOGIN
        .try_lock()
        .map_err(|_| "이미 Microsoft 로그인이 진행 중이에요.")?;
    let mut account = load_account()
        .await?
        .ok_or("Microsoft 계정에 로그인해 주세요.")?;
    let client_id = client_id()?;
    let client = login_client()?;
    let microsoft: MicrosoftToken = read_response(
        client.post(TOKEN_ENDPOINT).form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id.as_str()),
            ("refresh_token", &account.refresh_token),
            ("scope", SCOPE),
        ]),
        "Microsoft 로그인 갱신",
    )
    .await?;

    // Keep the replacement even if Xbox or Minecraft is temporarily unavailable.
    account.refresh_token = microsoft.refresh_token;
    let profile = save_account(account).await?;
    let session = exchange(&client, client_id, &microsoft.access_token).await?;
    if session.profile.id != profile.id {
        return Err("저장된 Minecraft 계정과 일치하지 않아요. 다시 로그인해 주세요.".into());
    }
    Ok(session)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_registered_origin_path_and_state() {
        let good = Url::parse(&format!("{REDIRECT}?state=match&code=verified")).unwrap();
        assert_eq!(
            handle_redirect(&good, "match").unwrap().unwrap(),
            "verified"
        );
        assert!(handle_redirect(&good, "different").unwrap().is_err());
        for url in [
            "https://login.microsoftonline.com.example.com/common/oauth2/nativeclient?state=match&code=evil",
            "https://login.microsoftonline.com/common/oauth2/nativeclient/extra?state=match&code=evil",
        ] {
            assert!(handle_redirect(&Url::parse(url).unwrap(), "match").is_none());
        }
        let duplicate = Url::parse(&format!("{REDIRECT}?state=match&code=a&code=b")).unwrap();
        assert!(handle_redirect(&duplicate, "match").unwrap().is_err());
    }
}
