use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

const REPOSITORY: &str = "https://github.com/AliceGameLand/AliceGameLand-Launcher";
const STATUS_FEED: &str =
    "https://raw.githubusercontent.com/AventiumSoftworks/helios-status-page/master/history/summary.json";
const SERVICES: &[(&str, &str)] = &[
    (
        "mojang-multiplayer-session-service",
        "Multiplayer Session Service",
    ),
    ("minecraft-skins", "Minecraft Skins"),
    ("mojang-s-public-api", "Public API"),
    ("mojang-accounts-website", "Mojang Accounts Website"),
    ("microsoft-o-auth-server", "Microsoft OAuth Server"),
    ("xbox-live-auth-server", "Xbox Live Auth Server"),
    ("xbox-live-gatekeeper", "Xbox Live Gatekeeper"),
    (
        "microsoft-minecraft-api",
        "Minecraft API for Microsoft Accounts",
    ),
    (
        "microsoft-minecraft-profile",
        "Minecraft Profile for Microsoft Accounts",
    ),
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectPage {
    Source,
    Issues,
    Release,
}

#[tauri::command]
pub fn open_project_page(app: AppHandle, page: ProjectPage) -> Result<(), String> {
    let url = match page {
        ProjectPage::Source => REPOSITORY.to_owned(),
        ProjectPage::Issues => format!("{REPOSITORY}/issues"),
        ProjectPage::Release => {
            format!("{REPOSITORY}/releases/tag/v{}", app.package_info().version)
        }
    };
    open::that_detached(url).map_err(|_| "기본 브라우저를 열지 못했어요.".into())
}

#[derive(Deserialize)]
struct FeedEntry {
    slug: String,
    status: String,
}

#[derive(Serialize)]
pub struct ServiceStatus {
    name: &'static str,
    state: &'static str,
}

async fn fetch_status() -> Option<Vec<FeedEntry>> {
    let mut response = reqwest::Client::builder()
        .timeout(Duration::from_millis(2500))
        .build()
        .ok()?
        .get(STATUS_FEED)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if body.len() + chunk.len() > 256 * 1024 {
            return None;
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).ok()
}

#[tauri::command]
pub async fn authentication_status() -> Vec<ServiceStatus> {
    let feed = fetch_status().await.unwrap_or_default();
    SERVICES
        .iter()
        .map(|&(slug, name)| {
            let state = match feed
                .iter()
                .find(|entry| entry.slug == slug)
                .map(|entry| entry.status.as_str())
            {
                Some("up") => "green",
                Some("down") => "red",
                Some("degraded") => "yellow",
                _ => "grey",
            };
            ServiceStatus { name, state }
        })
        .collect()
}
