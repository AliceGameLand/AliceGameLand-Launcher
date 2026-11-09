#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(all(target_os = "macos", not(target_arch = "aarch64")))]
compile_error!("macOS builds require Apple Silicon (aarch64-apple-darwin).");

mod about;
mod artifacts;
mod auth;
mod distribution;
mod game;
mod java;
mod minecraft;
mod preferences;
mod status;
#[cfg(windows)]
mod wizard;

use std::{path::PathBuf, sync::Mutex};

use distribution::{Distribution, Server};
use preferences::{heap_megabytes, GameSettings, JavaOptions, Preferences, Settings};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

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
async fn server_status(
    server_id: String,
    launcher: State<'_, Launcher>,
) -> Result<Option<status::Players>, String> {
    let server = launcher
        .servers
        .iter()
        .find(|server| server.id == server_id)
        .ok_or("서버를 찾지 못했어요.")?;
    Ok(status::query(&server.address).await)
}

#[tauri::command]
fn game_settings(launcher: State<'_, Launcher>) -> Result<Settings, String> {
    let preferences = launcher
        .preferences
        .lock()
        .map_err(|_| "런처 설정을 읽지 못했어요.")?;
    Ok(preferences.settings().clone())
}

#[tauri::command]
async fn choose_game_folder(
    window: tauri::WebviewWindow,
    launcher: State<'_, Launcher>,
) -> Result<Option<PathBuf>, String> {
    let current = launcher
        .preferences
        .lock()
        .map_err(|_| "런처 설정을 읽지 못했어요.")?
        .game_data_dir()
        .map(std::path::Path::to_path_buf);
    let mut picker = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("게임 폴더 선택");
    if let Some(current) = current.filter(|path| path.is_dir()) {
        picker = picker.set_directory(current);
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    picker.pick_folder(move |choice| {
        let _ = sender.send(choice);
    });
    receiver
        .await
        .map_err(|_| "폴더 선택 창을 열지 못했어요.")?
        .map(|choice| {
            choice
                .into_path()
                .map_err(|_| "게임 폴더 경로를 읽지 못했어요.".into())
        })
        .transpose()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JavaSettings {
    options: JavaOptions,
    executable: Option<PathBuf>,
    minimum_mib: u32,
    recommended_mib: u32,
    memory: Option<MemoryStatus>,
}

#[derive(Serialize)]
struct MemoryStatus {
    total: u64,
    available: u64,
}

#[tauri::command]
fn java_settings(server_id: String, launcher: State<'_, Launcher>) -> Result<JavaSettings, String> {
    let server = launcher
        .servers
        .iter()
        .find(|server| server.id == server_id)
        .ok_or("서버를 찾지 못했어요.")?;
    let (_, options) = launcher
        .preferences
        .lock()
        .map_err(|_| "런처 설정을 읽지 못했어요.")?
        .launch_settings(&server_id);
    let executable = launcher
        .preferences_path
        .parent()
        .and_then(|path| java::cached_executable(&path.join("java")));
    let memory = sysinfo::System::new().ok().and_then(|mut system| {
        system.refresh_memory_specifics(sysinfo::MemoryRefreshKind::nothing().with_ram());
        let total = system.total_memory();
        (total > 0).then_some(MemoryStatus {
            total,
            available: system.available_memory(),
        })
    });
    Ok(JavaSettings {
        options,
        executable,
        minimum_mib: server.minimum_heap(),
        recommended_mib: server.recommended_heap(),
        memory,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JavaSettingsChange {
    server_id: String,
    options: JavaOptions,
}

#[tauri::command]
fn save_game_settings(
    settings: GameSettings,
    java: Option<JavaSettingsChange>,
    data_directory: Option<PathBuf>,
    launcher: State<'_, Launcher>,
) -> Result<(), String> {
    if [settings.res_width, settings.res_height]
        .iter()
        .any(|dimension| !matches!(dimension, Some(1..=65535)))
    {
        return Err("가로와 세로 해상도를 올바르게 입력해 주세요.".into());
    }
    let java = match java {
        Some(change) => {
            let server = launcher
                .servers
                .iter()
                .find(|server| server.id == change.server_id)
                .ok_or("서버를 찾지 못했어요.")?;
            let (min, max) = change.options.effective_heap(server.recommended_heap());
            let (min, max) = (heap_megabytes(&min), heap_megabytes(&max));
            if !matches!((min, max), (Some(min), Some(max)) if min > 0 && max >= min) {
                return Err("Java 최소 메모리가 최대 메모리를 넘지 않게 설정해 주세요.".into());
            }
            if change
                .options
                .jvm_options
                .iter()
                .flatten()
                .any(|arg| arg.is_empty() || arg.contains('\0'))
            {
                return Err("Java 실행 옵션을 확인해 주세요.".into());
            }
            Some((change.server_id, change.options))
        }
        None => None,
    };
    if data_directory
        .as_ref()
        .is_some_and(|path| !path.is_absolute() || !path.is_dir())
    {
        return Err("선택한 게임 폴더에 접근할 수 없어요.".into());
    }
    let mut preferences = launcher
        .preferences
        .lock()
        .map_err(|_| "런처 설정을 읽지 못했어요.")?;
    preferences
        .update_game_settings(settings, java, data_directory, &launcher.preferences_path)
        .map_err(|_| "게임 설정을 저장하지 못했어요.".into())
}

#[tauri::command]
fn open_game_folder(launcher: State<'_, Launcher>) -> Result<(), String> {
    let path = launcher
        .preferences
        .lock()
        .map_err(|_| "런처 설정을 읽지 못했어요.")?
        .game_data_dir()
        .filter(|path| path.is_absolute())
        .ok_or("게임 데이터 폴더를 찾지 못했어요.")?
        .to_path_buf();
    std::fs::create_dir_all(&path).map_err(|_| "게임 데이터 폴더를 만들지 못했어요.")?;
    let opener = if cfg!(windows) {
        "explorer.exe"
    } else {
        "/usr/bin/open"
    };
    let mut process = std::process::Command::new(opener)
        .arg(path)
        .spawn()
        .map_err(|_| "게임 데이터 폴더를 열지 못했어요.")?;
    std::thread::spawn(move || {
        let _ = process.wait();
    });
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
    let context = tauri::generate_context!();
    #[cfg(windows)]
    match wizard::Mode::from_args() {
        Ok(Some(mode)) => return wizard::run(context, mode),
        Ok(None) => {}
        Err(_) => std::process::exit(2),
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            tauri::WebviewWindowBuilder::from_config(app, &app.config().app.windows[0])?.build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            launcher_snapshot,
            select_server,
            server_status,
            about::authentication_status,
            about::open_project_page,
            play_server,
            game_settings,
            choose_game_folder,
            java_settings,
            save_game_settings,
            open_game_folder,
            auth::microsoft_sign_out,
            auth::account_profile,
            auth::microsoft_sign_in
        ])
        .build(context)
        .expect("failed to start AliceGameLand")
        .run(|_, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                game::stop_attached();
            }
        });
}
