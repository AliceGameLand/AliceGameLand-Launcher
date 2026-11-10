use std::time::Duration;

use reqwest::{redirect::Policy, Url};
use semver::Version;
use serde::Serialize;
use tauri::{ipc::Channel, AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::Mutex;

use crate::{game, Launcher};

const PUBLIC_KEY: Option<&str> = option_env!("ALICE_TAURI_UPDATE_PUBLIC_KEY");
const STABLE_FEED: &str = "https://github.com/AliceGameLand/AliceGameLand-Launcher/releases/download/tauri-stable-feed/tauri-update.json";
const PREVIEW_FEED: &str = "https://github.com/AliceGameLand/AliceGameLand-Launcher/releases/download/tauri-preview-feed/tauri-update.json";
const MAX_PACKAGE: usize = 256 * 1024 * 1024;
const UNAVAILABLE: &str = "이 빌드에서는 자동 업데이트를 사용할 수 없어요.";
const CHECK_FAILED: &str = "업데이트를 확인하지 못했어요. 잠시 후 다시 시도해 주세요.";
const DOWNLOAD_FAILED: &str = "업데이트 파일을 받지 못했어요. 잠시 후 다시 시도해 주세요.";

#[derive(Default)]
pub struct Updates(Mutex<Option<Update>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    version: String,
    notes: Option<String>,
}

#[derive(Serialize)]
pub struct Progress {
    received: u64,
    total: Option<u64>,
}

pub fn available() -> bool {
    if PUBLIC_KEY.is_none_or(str::is_empty) {
        return false;
    }
    #[cfg(windows)]
    return crate::wizard::update_scope_for_running_install().is_some();
    #[cfg(target_os = "macos")]
    return true;
    #[cfg(not(any(windows, target_os = "macos")))]
    false
}

fn is_newer(current: &Version, release: &Version, prerelease: bool) -> bool {
    release.cmp_precedence(current).is_gt() && (prerelease || release.pre.is_empty())
}

fn allows_preview(app: &AppHandle, launcher: &Launcher) -> Result<bool, String> {
    Ok(!app.package_info().version.pre.is_empty()
        || launcher
            .preferences
            .lock()
            .map_err(|_| CHECK_FAILED)?
            .allow_prerelease())
}

fn trusted_transfer(url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}

fn trusted_package(url: &Url) -> bool {
    trusted_transfer(url)
        && url.host_str() == Some("github.com")
        && url
            .path()
            .starts_with("/AliceGameLand/AliceGameLand-Launcher/releases/download/")
}

async fn check_feed(
    app: &AppHandle,
    feed: &str,
    prerelease: bool,
) -> Result<Option<Update>, String> {
    let updater = app
        .updater_builder()
        .pubkey(PUBLIC_KEY.ok_or(UNAVAILABLE)?)
        .endpoints(vec![Url::parse(feed).map_err(|_| CHECK_FAILED)?])
        .map_err(|_| CHECK_FAILED)?
        .configure_client(|client| client.https_only(true))
        .timeout(Duration::from_secs(15))
        .version_comparator(move |current, remote| is_newer(&current, &remote.version, prerelease))
        .restart_after_install(false)
        .on_before_exit(game::stop_attached);
    #[cfg(windows)]
    let updater = updater.installer_arg(
        crate::wizard::update_scope_for_running_install()
            .ok_or(UNAVAILABLE)?
            .update_flag(),
    );
    let updater = updater.build().map_err(|_| CHECK_FAILED)?;
    let update = updater.check().await.map_err(|_| CHECK_FAILED)?;
    if update
        .as_ref()
        .is_some_and(|update| !trusted_package(&update.download_url))
    {
        return Err("업데이트 파일 주소를 확인하지 못했어요.".into());
    }
    Ok(update)
}

#[tauri::command]
pub async fn check_launcher_update(
    app: AppHandle,
    launcher: State<'_, Launcher>,
    updates: State<'_, Updates>,
) -> Result<Option<AvailableUpdate>, String> {
    if !available() {
        return Err(UNAVAILABLE.into());
    }
    let mut pending = updates
        .0
        .try_lock()
        .map_err(|_| "업데이트 작업이 진행 중이에요.")?;
    *pending = None;
    let prerelease = allows_preview(&app, &launcher)?;
    let stable = check_feed(&app, STABLE_FEED, false);
    let latest = if prerelease {
        let (stable, preview) = tokio::join!(stable, check_feed(&app, PREVIEW_FEED, true));
        match (stable, preview) {
            (Ok(stable), Ok(preview)) => [stable, preview]
                .into_iter()
                .flatten()
                .max_by_key(|update| Version::parse(&update.version).ok()),
            (Ok(Some(update)), Err(_)) | (Err(_), Ok(Some(update))) => Some(update),
            (Ok(None), Err(_)) => None,
            (Err(error), _) => return Err(error),
        }
    } else {
        stable.await?
    };
    let details = latest.as_ref().map(|update| AvailableUpdate {
        version: update.version.clone(),
        notes: update
            .body
            .as_ref()
            .map(|notes| notes.chars().take(16_384).collect()),
    });
    *pending = latest;
    Ok(details)
}

async fn download(update: &Update, progress: Channel<Progress>) -> Result<Vec<u8>, String> {
    let client = reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !trusted_transfer(attempt.url()) {
                attempt.error("Unexpected update redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| DOWNLOAD_FAILED)?;
    read_package(&client, &update.download_url, |event| {
        let _ = progress.send(event);
    })
    .await
}

async fn read_package(
    client: &reqwest::Client,
    url: &Url,
    mut report: impl FnMut(Progress),
) -> Result<Vec<u8>, String> {
    let mut response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|_| DOWNLOAD_FAILED)?
        .error_for_status()
        .map_err(|_| DOWNLOAD_FAILED)?;
    let total = response.content_length();
    if total.is_some_and(|size| size > MAX_PACKAGE as u64) {
        return Err("업데이트 파일이 너무 커요.".into());
    }
    let mut bytes = Vec::new();
    let mut next_report = 0;
    while let Some(chunk) = response.chunk().await.map_err(|_| DOWNLOAD_FAILED)? {
        if bytes.len() + chunk.len() > MAX_PACKAGE {
            return Err("업데이트 파일이 너무 커요.".into());
        }
        bytes.extend_from_slice(&chunk);
        if bytes.len() >= next_report {
            report(Progress {
                received: bytes.len() as u64,
                total,
            });
            next_report = bytes.len() + 256 * 1024;
        }
    }
    report(Progress {
        received: bytes.len() as u64,
        total,
    });
    Ok(bytes)
}

#[tauri::command]
pub async fn install_launcher_update(
    app: AppHandle,
    launcher: State<'_, Launcher>,
    updates: State<'_, Updates>,
    version: String,
    progress: Channel<Progress>,
) -> Result<(), String> {
    if !available() {
        return Err(UNAVAILABLE.into());
    }
    let pending = updates
        .0
        .try_lock()
        .map_err(|_| "업데이트 작업이 진행 중이에요.")?;
    let update = pending
        .as_ref()
        .filter(|update| update.version == version)
        .ok_or("업데이트를 다시 확인해 주세요.")?;
    let release = Version::parse(&version).map_err(|_| CHECK_FAILED)?;
    if !is_newer(
        &app.package_info().version,
        &release,
        allows_preview(&app, &launcher)?,
    ) {
        return Err("업데이트 설정이 바뀌었어요. 새 버전을 다시 확인해 주세요.".into());
    }
    let bytes = download(update, progress).await?;
    if !available() {
        return Err("설치 위치가 바뀌었어요. 런처를 다시 열어 주세요.".into());
    }
    let update = update.clone();
    tauri::async_runtime::spawn_blocking(move || {
        update
            .verify(&bytes)
            .map_err(|_| "업데이트 파일의 서명을 확인하지 못했어요.")?;
        update
            .install(bytes)
            .map_err(|_| "업데이트를 적용하지 못했어요. 설치 위치와 권한을 확인해 주세요.")
    })
    .await
    .map_err(|_| "업데이트 작업이 중단됐어요.")??;
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_offers_newer_releases_on_the_selected_channel() {
        let current = Version::parse("1.0.41").unwrap();
        let preview = Version::parse("1.0.42-beta.1").unwrap();
        assert!(!is_newer(&current, &current, true));
        assert!(!is_newer(
            &current,
            &Version::parse("1.0.40").unwrap(),
            true
        ));
        assert!(!is_newer(
            &current,
            &Version::parse("1.0.41+rebuilt").unwrap(),
            true
        ));
        assert!(!is_newer(&current, &preview, false));
        assert!(is_newer(&current, &preview, true));
        assert!(is_newer(
            &preview,
            &Version::parse("1.0.42").unwrap(),
            false
        ));
    }

    #[test]
    fn accepts_packages_only_from_the_launcher_release_repository() {
        let base = "https://github.com/AliceGameLand/AliceGameLand-Launcher/releases/download/v1.0.42/update.exe";
        assert!(trusted_package(&Url::parse(base).unwrap()));
        for bad in [
            base.replace("https:", "http:"),
            base.replace("github.com", "github.com.example.net"),
            base.replace("github.com", "github.com:444"),
            base.replace("github.com", "user@github.com"),
            base.replace("AliceGameLand-Launcher", "AliceGameLand-Distribution"),
        ] {
            assert!(!trusted_package(&Url::parse(&bad).unwrap()), "{bad}");
        }
    }

    #[test]
    #[ignore = "uses a disposable signing key and built CI package"]
    fn signed_update_accepts_only_matching_package_and_version() {
        use serde::Deserialize;
        use std::{fs, path::PathBuf};
        use tokio::{
            io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
            net::TcpListener,
        };

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Fixture {
            executable: PathBuf,
            #[cfg(target_os = "macos")]
            expected: PathBuf,
            payload: PathBuf,
            manifest: serde_json::Value,
            public_key: String,
        }

        let root = PathBuf::from(std::env::var_os("ALICE_TAURI_TEST_UPDATE").unwrap());
        let mut fixture: Fixture =
            serde_json::from_slice(&fs::read(root.join("fixture.json")).unwrap()).unwrap();
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().plugins.0.insert(
            "updater".into(),
            serde_json::json!({
                "pubkey": fixture.public_key,
                "requireSignedVersion": true,
                "dangerousInsecureTransportProtocol": true
            }),
        );
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_updater::Builder::new().build())
            .build(context)
            .unwrap();
        tauri::async_runtime::block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let origin = format!("http://{}", listener.local_addr().unwrap());
            for target in fixture.manifest["platforms"]
                .as_object_mut()
                .unwrap()
                .values_mut()
            {
                target["url"] = serde_json::json!(format!("{origin}/package"));
            }
            let manifest = serde_json::to_vec(&fixture.manifest).unwrap();
            let package = fs::read(&fixture.payload).unwrap();
            let package_size = package.len();
            let server = tokio::spawn(async move {
                for (route, payload, content_type) in [
                    ("/feed", manifest, "application/json"),
                    ("/package", package, "application/octet-stream"),
                ] {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    let mut reader = BufReader::new(&mut stream);
                    let mut line = String::new();
                    reader.read_line(&mut line).await.unwrap();
                    assert!(line.starts_with(&format!("GET {route} ")));
                    loop {
                        line.clear();
                        reader.read_line(&mut line).await.unwrap();
                        if line == "\r\n" {
                            break;
                        }
                        assert!(!line.is_empty());
                    }
                    let headers = format!("HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n", payload.len());
                    stream.write_all(headers.as_bytes()).await.unwrap();
                    stream.write_all(&payload).await.unwrap();
                }
            });
            let updater = app
                .updater_builder()
                .executable_path(&fixture.executable)
                .endpoints(vec![Url::parse(&format!("{origin}/feed")).unwrap()])
                .unwrap()
                .no_proxy()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap();
            let update = updater.check().await.unwrap().unwrap();
            let client = reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap();
            let mut received = 0;
            let mut bytes = read_package(&client, &update.download_url, |event| {
                received = event.received
            })
            .await
            .unwrap();
            server.await.unwrap();
            assert_eq!(received, package_size as u64);
            update.verify(&bytes).unwrap();
            bytes[0] ^= 1;
            assert!(update.verify(&bytes).is_err());
            bytes[0] ^= 1;
            let mut relabeled = fixture.manifest;
            relabeled["version"] = serde_json::json!("999.0.0");
            let wrong_version = updater.restore_update(relabeled).unwrap().unwrap();
            assert!(wrong_version.verify(&bytes).is_err());
            #[cfg(target_os = "macos")]
            {
                update.install(bytes).unwrap();
                assert_eq!(
                    fs::read(fixture.executable).unwrap(),
                    fs::read(fixture.expected).unwrap()
                );
            }
        });
    }
}
