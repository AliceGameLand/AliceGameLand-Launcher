fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "launcher_snapshot",
            "select_server",
            "server_status",
            "authentication_status",
            "open_project_page",
            "play_server",
            "game_settings",
            "choose_game_folder",
            "java_settings",
            "save_game_settings",
            "open_game_folder",
            "microsoft_sign_out",
            "account_profile",
            "microsoft_sign_in",
            "wizard_info",
            "wizard_folder",
            "wizard_install",
            "wizard_remove",
            "wizard_launch",
        ]),
    ))
    .expect("failed to prepare launcher permissions");
}
