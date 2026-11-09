fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        build_windows_java_launcher();
    }
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "launcher_snapshot",
            "test_login_ready",
            "select_server",
            "server_status",
            "authentication_status",
            "open_project_page",
            "check_launcher_update",
            "install_launcher_update",
            "play_server",
            "game_settings",
            "choose_game_folder",
            "java_settings",
            "save_game_settings",
            "open_game_folder",
            "mod_settings",
            "change_local_mod",
            "open_instance_folder",
            "select_shaderpack",
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

fn build_windows_java_launcher() {
    use std::{env, path::PathBuf, process::Command};

    let source = "windows/java-launcher.rs";
    let manifest = "windows/java-launcher.manifest";
    println!("cargo:rerun-if-changed={source}");
    println!("cargo:rerun-if-changed={manifest}");
    let manifest = env::current_dir().unwrap().join(manifest);
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("alice-java.exe");
    let status = Command::new(env::var_os("RUSTC").unwrap())
        .args(["--edition=2021", "--target"])
        .arg(env::var_os("TARGET").unwrap())
        .args([
            "-Copt-level=s",
            "-Cstrip=symbols",
            "-Clink-arg=/MANIFEST:EMBED",
        ])
        .arg(format!("-Clink-arg=/MANIFESTINPUT:{}", manifest.display()))
        .arg(source)
        .arg("-o")
        .arg(output)
        .status()
        .expect("failed to compile the Windows Java launcher");
    assert!(status.success(), "Windows Java launcher build failed");
}
