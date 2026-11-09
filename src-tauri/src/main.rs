#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const DISTRIBUTION: &str = include_str!("../../app/assets/distribution.json");

#[derive(Deserialize)]
struct Distribution {
    servers: Vec<Server>,
}

#[derive(Deserialize, Serialize)]
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
}

#[tauri::command]
fn launcher_snapshot(app: AppHandle) -> Result<Snapshot, String> {
    let distribution: Distribution =
        serde_json::from_str(DISTRIBUTION).map_err(|error| error.to_string())?;
    Ok(Snapshot {
        version: app.package_info().version.to_string(),
        servers: distribution.servers,
    })
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![launcher_snapshot])
        .run(tauri::generate_context!())
        .expect("failed to start AliceGameLand");
}
