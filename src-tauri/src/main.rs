#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod java;
mod preferences;

use std::{path::PathBuf, sync::Mutex};

use preferences::Preferences;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

const DISTRIBUTION: &str = include_str!("../../app/assets/distribution.json");

struct Launcher {
    servers: Vec<Server>,
    preferences_path: PathBuf,
    preferences: Mutex<Preferences>,
}

#[derive(Deserialize)]
struct Distribution {
    servers: Vec<Server>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Server {
    id: String,
    name: String,
    minecraft_version: String,
    #[serde(default)]
    main_server: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    version: String,
    servers: Vec<Server>,
    selected_server: Option<String>,
}

#[tauri::command]
fn launcher_snapshot(app: AppHandle, launcher: State<'_, Launcher>) -> Result<Snapshot, String> {
    let preferences = launcher
        .preferences
        .lock()
        .map_err(|error| error.to_string())?;
    Ok(Snapshot {
        version: app.package_info().version.to_string(),
        servers: launcher.servers.clone(),
        selected_server: preferences.selected_server.clone(),
    })
}

#[tauri::command]
fn select_server(server_id: String, launcher: State<'_, Launcher>) -> Result<(), String> {
    if !launcher.servers.iter().any(|server| server.id == server_id) {
        return Err("Unknown server".into());
    }
    let mut preferences = launcher
        .preferences
        .lock()
        .map_err(|error| error.to_string())?;
    let previous = preferences.selected_server.replace(server_id);
    if let Err(error) = preferences.save(&launcher.preferences_path) {
        preferences.selected_server = previous;
        return Err(error.to_string());
    }
    Ok(())
}

#[tauri::command]
async fn prepare_java(launcher: State<'_, Launcher>) -> Result<PathBuf, String> {
    let base = launcher
        .preferences_path
        .parent()
        .ok_or("Launcher profile directory is missing")?
        .join("java");
    tauri::async_runtime::spawn_blocking(move || {
        java::prepare(&base).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

fn test_profile() -> std::io::Result<Option<PathBuf>> {
    let Some(path) = std::env::var_os("ALICE_TAURI_TEST_PROFILE").map(PathBuf::from) else {
        return Ok(None);
    };
    if path.is_absolute() && path.is_dir() {
        Ok(Some(path))
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Test profile must be an existing absolute directory",
        ))
    }
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let distribution: Distribution = serde_json::from_str(DISTRIBUTION)?;
            let test_profile = test_profile()?;
            let (profile, data_root) = match test_profile {
                Some(path) => (path.clone(), path),
                None => (app.path().app_data_dir()?, app.path().data_dir()?),
            };
            let preferences_path = profile.join("preferences.json");
            let preferences = Preferences::load(&preferences_path, &data_root)?;
            app.manage(Launcher {
                servers: distribution.servers,
                preferences_path,
                preferences: Mutex::new(preferences),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            launcher_snapshot,
            select_server,
            prepare_java,
            auth::account_profile,
            auth::microsoft_sign_in
        ])
        .run(tauri::generate_context!())
        .expect("failed to start AliceGameLand");
}
