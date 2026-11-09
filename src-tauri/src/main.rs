#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod artifacts;
mod auth;
mod distribution;
mod java;
mod minecraft;
mod preferences;

use std::{path::PathBuf, sync::Mutex};

use distribution::{Distribution, Server};
use preferences::Preferences;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

static PREPARATION: Mutex<()> = Mutex::new(());

struct Launcher {
    servers: Vec<Server>,
    preferences_path: PathBuf,
    preferences: Mutex<Preferences>,
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

#[derive(Serialize)]
struct Prepared {
    verified_files: usize,
    downloaded_files: usize,
}

#[tauri::command]
async fn prepare_server(
    server_id: String,
    launcher: State<'_, Launcher>,
) -> Result<Prepared, String> {
    if !launcher.servers.iter().any(|server| server.id == server_id) {
        return Err("Unknown server".into());
    }
    let profile = launcher
        .preferences_path
        .parent()
        .ok_or("Launcher profile directory is missing")?
        .to_path_buf();
    let root = launcher
        .preferences
        .lock()
        .map_err(|error| error.to_string())?
        .game_data_dir()
        .ok_or("Game data directory is missing")?
        .to_path_buf();
    tauri::async_runtime::spawn_blocking(move || -> artifacts::Result<_> {
        let _guard = PREPARATION
            .try_lock()
            .map_err(|_| "Server files are already being prepared")?;
        if !root.is_absolute() {
            return Err("Game data directory must be absolute".into());
        }
        let client = artifacts::client()?;
        let distribution = Distribution::refresh(&client, &profile.join("distribution.json"))?;
        let server = distribution
            .servers
            .iter()
            .find(|server| server.id == server_id)
            .ok_or("Server is no longer in the distribution")?;
        let files = server.files()?;
        java::prepare(&profile.join("java"))?;
        std::fs::create_dir_all(&root)?;
        let root = root.canonicalize()?;
        let downloaded = artifacts::ensure_all(&client, &root, &files)?;
        let vanilla = minecraft::files(&client, &root, &server.minecraft_version)?;
        let vanilla_downloaded = artifacts::ensure_all(&client, &root, &vanilla)?;
        Ok(Prepared {
            verified_files: files.len() + vanilla.len(),
            downloaded_files: downloaded + vanilla_downloaded,
        })
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())
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
            let test_profile = test_profile()?;
            let (profile, data_root) = match test_profile {
                Some(path) => (path.clone(), path),
                None => (app.path().app_data_dir()?, app.path().data_dir()?),
            };
            let preferences_path = profile.join("preferences.json");
            let distribution = Distribution::local(&profile.join("distribution.json"))
                .map_err(std::io::Error::other)?;
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
            prepare_server,
            auth::account_profile,
            auth::microsoft_sign_in
        ])
        .run(tauri::generate_context!())
        .expect("failed to start AliceGameLand");
}
