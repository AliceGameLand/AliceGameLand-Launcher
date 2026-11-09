#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod artifacts;
mod auth;
mod distribution;
mod game;
mod java;
mod minecraft;
mod preferences;

use std::{path::PathBuf, sync::Mutex};

use distribution::{Distribution, Server};
use preferences::Preferences;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

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

#[tauri::command]
async fn play_server(
    server_id: String,
    progress: tauri::ipc::Channel<&'static str>,
    app: AppHandle,
    launcher: State<'_, Launcher>,
) -> Result<u32, String> {
    if !launcher.servers.iter().any(|server| server.id == server_id) {
        return Err("서버를 찾지 못했어요.".into());
    }
    let launch = {
        let profile = launcher
            .preferences_path
            .parent()
            .ok_or("런처 프로필 경로를 찾지 못했어요.")?
            .to_path_buf();
        let preferences = launcher
            .preferences
            .lock()
            .map_err(|_| "런처 설정을 읽지 못했어요.")?;
        let root = preferences
            .game_data_dir()
            .ok_or("게임 데이터 폴더를 찾지 못했어요.")?
            .to_path_buf();
        let (settings, java_options) = preferences.launch_settings(&server_id);
        game::Launch {
            profile,
            root,
            server_id,
            launcher_version: app.package_info().version.to_string(),
            settings,
            java_options,
        }
    };
    let _ = progress.send("Minecraft 계정 확인 중");
    let session = auth::game_session().await?;
    tauri::async_runtime::spawn_blocking(move || game::play(launch, session, progress))
        .await
        .map_err(|_| "게임 준비가 중단됐어요.")?
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
            play_server,
            auth::account_profile,
            auth::microsoft_sign_in
        ])
        .run(tauri::generate_context!())
        .expect("failed to start AliceGameLand");
}
