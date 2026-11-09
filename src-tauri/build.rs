fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "launcher_snapshot",
            "select_server",
            "prepare_java",
            "account_profile",
            "microsoft_sign_in",
        ]),
    ))
    .expect("failed to prepare launcher permissions");
}
