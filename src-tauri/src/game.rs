use std::{
    collections::{BTreeMap, HashSet},
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{LazyLock, Mutex},
};

use serde::Deserialize;
use tauri::ipc::Channel;

use crate::{
    artifacts::{self, Result},
    auth::GameSession,
    distribution::{Distribution, Loader, Server},
    java,
    minecraft::{self, Arguments, Version},
    preferences::{GameSettings, JavaOptions},
};

static PREPARATION: Mutex<()> = Mutex::new(());
static RUNNING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

struct RunningGame(String);

impl RunningGame {
    fn reserve(server_id: String) -> Result<Self> {
        if !RUNNING
            .lock()
            .map_err(|_| "실행 중인 게임을 확인하지 못했어요.")?
            .insert(server_id.clone())
        {
            return Err("이 서버의 게임이 이미 실행 중이에요.".into());
        }
        Ok(Self(server_id))
    }
}

impl Drop for RunningGame {
    fn drop(&mut self) {
        if let Ok(mut running) = RUNNING.lock() {
            running.remove(&self.0);
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoaderVersion {
    id: String,
    inherits_from: String,
    main_class: String,
    #[serde(default)]
    arguments: Arguments,
}

fn prepare_files(
    profile: &Path,
    root: &Path,
    server_id: &str,
    progress: &Channel<&'static str>,
) -> Result<(Server, PathBuf)> {
    if !root.is_absolute() {
        return Err("게임 데이터 폴더는 절대 경로여야 해요.".into());
    }
    let client = artifacts::client()?;
    let distribution = Distribution::refresh(&client, &profile.join("distribution.json"))?;
    let server = distribution
        .servers
        .into_iter()
        .find(|server| server.id == server_id)
        .ok_or("배포 목록에서 서버를 찾지 못했어요.")?;

    let _ = progress.send("Java 25 확인 중");
    let java = java::prepare(&profile.join("java"))?;
    let _ = progress.send("게임 파일 확인 중");
    fs::create_dir_all(root)?;
    let canonical_root = root.canonicalize()?;
    artifacts::ensure_all(&client, &canonical_root, &server.files()?)?;
    let vanilla = minecraft::files(&client, &canonical_root, &server.minecraft_version)?;
    artifacts::ensure_all(&client, &canonical_root, &vanilla)?;
    Ok((server, java))
}

fn nonempty_zip(path: &Path) -> bool {
    fs::File::open(path)
        .ok()
        .and_then(|file| zip::ZipArchive::new(file).ok())
        .is_some_and(|archive| !archive.is_empty())
}

fn install_neoforge(common: &Path, java: &Path, version: &str, installer: &Path) -> Result<()> {
    let patched = common
        .join("libraries/net/neoforged/minecraft-client-patched")
        .join(version)
        .join(format!("minecraft-client-patched-{version}.jar"));
    if nonempty_zip(&patched) {
        return Ok(());
    }
    let profiles = common.join("launcher_profiles.json");
    let mut stage = tempfile::NamedTempFile::new_in(common)?;
    stage.write_all(br#"{"profiles":{},"settings":{},"version":3}"#)?;
    match stage.persist_noclobber(profiles) {
        Ok(_) => {}
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    let executable = if cfg!(windows) {
        java.with_file_name("java.exe")
    } else {
        java.to_path_buf()
    };
    let mut command = Command::new(executable);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let status = command
        .arg("-jar")
        .arg(installer)
        .arg("--install-client")
        .arg(common)
        .current_dir(common)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() || !nonempty_zip(&patched) {
        return Err(format!("NeoForge {version} 설치를 완료하지 못했어요. ({status})").into());
    }
    Ok(())
}

fn extract_natives(archive: &Path, target: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(fs::File::open(archive)?)?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        let Some(path) = entry.enclosed_name() else {
            continue;
        };
        if !entry.is_file() || path.starts_with("META-INF") {
            continue;
        }
        if !matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("dll" | "dylib" | "jnilib" | "so")
        ) {
            continue;
        }
        let name = path
            .file_name()
            .ok_or("Native library file name is missing")?;
        io::copy(&mut entry, &mut fs::File::create(target.join(name))?)?;
    }
    Ok(())
}

fn expand(mut argument: &str, variables: &BTreeMap<&str, String>) -> Result<String> {
    let mut value = String::new();
    while let Some((prefix, rest)) = argument.split_once("${") {
        let (key, tail) = rest
            .split_once('}')
            .ok_or("Unterminated Minecraft argument variable")?;
        let replacement = variables
            .get(key)
            .ok_or_else(|| format!("Unknown Minecraft argument variable: {key}"))?;
        value.push_str(prefix);
        value.push_str(replacement);
        argument = tail;
    }
    value.push_str(argument);
    Ok(value)
}

fn path_text(path: &Path) -> Result<String> {
    Ok(path
        .to_str()
        .ok_or("게임 경로를 읽지 못했어요.")?
        .to_owned())
}

fn set_game_options(instance: &Path, fullscreen: bool) -> Result<()> {
    let path = instance.join("options.txt");
    let original = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    let mut updated = original.clone();
    for (key, value) in [
        ("tutorialStep:", "none"),
        ("fullscreen:", if fullscreen { "true" } else { "false" }),
    ] {
        let mut found = false;
        let mut lines = String::new();
        for line in updated.split_inclusive('\n') {
            if line.starts_with(key) {
                let setting = line.trim_end_matches(['\r', '\n']);
                lines.push_str(key);
                lines.push_str(value);
                lines.push_str(&line[setting.len()..]);
                found = true;
            } else {
                lines.push_str(line);
            }
        }
        if !found {
            let newline = if original.contains("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            if !lines.is_empty() && !lines.ends_with('\n') {
                lines.push_str(newline);
            }
            lines.push_str(key);
            lines.push_str(value);
            lines.push_str(newline);
        }
        updated = lines;
    }
    if updated != original {
        let mut stage = tempfile::NamedTempFile::new_in(instance)?;
        stage.write_all(updated.as_bytes())?;
        stage.as_file().sync_all()?;
        stage.persist(path)?;
    }
    Ok(())
}

fn write_mod_list(instance: &Path, loader: &Loader) -> Result<Vec<String>> {
    if loader.mods.is_empty() {
        return Ok(Vec::new());
    }
    let mut stage = tempfile::NamedTempFile::new_in(instance)?;
    stage.write_all(loader.mods.join("\n").as_bytes())?;
    let path = instance.join("forgeMods.list");
    stage.persist(&path)?;
    let path = path_text(&path)?;
    if loader.installer.is_none() {
        Ok(vec!["--fabric.addMods".into(), format!("@{path}")])
    } else {
        Ok(vec![
            "--fml.mavenRoots".into(),
            "../../common/modstore".into(),
            "--fml.modLists".into(),
            path,
        ])
    }
}

pub struct Launch {
    pub profile: PathBuf,
    pub root: PathBuf,
    pub server_id: String,
    pub launcher_version: String,
    pub settings: GameSettings,
    pub java_options: JavaOptions,
}

pub fn play(launch: Launch, session: GameSession, progress: Channel<&'static str>) -> Result<u32> {
    let running = RunningGame::reserve(launch.server_id.clone())?;
    let _preparation = PREPARATION
        .try_lock()
        .map_err(|_| "다른 서버의 파일을 준비하고 있어요.")?;
    let (server, java) =
        prepare_files(&launch.profile, &launch.root, &launch.server_id, &progress)?;
    let common = launch.root.join("common");
    let libraries = common.join("libraries");
    let loader = server.loader(&launch.root)?;
    let loader_version: LoaderVersion = serde_json::from_reader(fs::File::open(&loader.manifest)?)?;
    let minecraft_id = &server.minecraft_version;
    let version_dir = launch.root.join(minecraft::version_path(minecraft_id));
    let vanilla: Version = serde_json::from_reader(fs::File::open(
        version_dir.join(format!("{minecraft_id}.json")),
    )?)?;
    if vanilla.id != *minecraft_id || loader_version.inherits_from != *minecraft_id {
        return Err("서버와 게임 로더의 Minecraft 버전이 일치하지 않아요.".into());
    }
    if let Some((version, installer)) = &loader.installer {
        let _ = progress.send("NeoForge 확인 중");
        install_neoforge(&common, &java, version, installer)?;
    }

    let natives = tempfile::Builder::new()
        .prefix("alice-natives-")
        .tempdir()?;
    let java_natives = natives.path().join("java");
    fs::create_dir(&java_natives)?;
    let mut classpath = Vec::new();
    if loader.installer.is_none() {
        classpath.push(version_dir.join(format!("{minecraft_id}.jar")));
    }
    let mut runtime = Vec::new();
    for (key, path, native) in vanilla.runtime_libraries(&launch.root)? {
        if native {
            extract_natives(&path, &java_natives)?;
        } else {
            runtime.push((key, path));
        }
    }
    for (key, path) in &loader.libraries {
        if let Some((_, previous)) = runtime.iter_mut().find(|(name, _)| name == key) {
            previous.clone_from(path);
        } else {
            runtime.push((key.clone(), path.clone()));
        }
    }
    classpath.extend(runtime.into_iter().map(|(_, path)| path));
    let classpath = env::join_paths(classpath)?
        .into_string()
        .map_err(|_| "클래스 경로를 읽지 못했어요.")?;
    let instance = launch.root.join("instances").join(&server.id);
    fs::create_dir_all(&instance)?;
    let fullscreen = launch.settings.fullscreen.unwrap_or(false);
    let variables = BTreeMap::from([
        ("auth_player_name", session.profile.name),
        ("auth_uuid", session.profile.id),
        ("auth_access_token", session.access_token),
        ("auth_xuid", session.xuid.unwrap_or_default()),
        ("clientid", session.client_id),
        ("user_type", "msa".into()),
        ("version_name", server.id.clone()),
        ("version_type", vanilla.version_type.clone()),
        ("game_directory", path_text(&instance)?),
        ("assets_root", path_text(&common.join("assets"))?),
        ("assets_index_name", vanilla.assets.clone()),
        (
            "resolution_width",
            launch.settings.res_width.unwrap_or(1280).to_string(),
        ),
        (
            "resolution_height",
            launch.settings.res_height.unwrap_or(720).to_string(),
        ),
        ("natives_directory", path_text(natives.path())?),
        ("library_directory", path_text(&libraries)?),
        ("launcher_name", "AliceGameLand-Launcher".into()),
        ("launcher_version", launch.launcher_version),
        (
            "classpath_separator",
            if cfg!(windows) { ";" } else { ":" }.into(),
        ),
        ("classpath", classpath),
    ]);
    let (vanilla_jvm, vanilla_game) = vanilla.arguments.resolve(true)?;
    let (loader_jvm, loader_game) = loader_version.arguments.resolve(true)?;
    let mut args = vanilla_jvm
        .into_iter()
        .map(|arg| expand(arg, &variables))
        .collect::<Result<Vec<_>>>()?;
    let mut loader_variables = variables.clone();
    loader_variables.insert("version_name", loader_version.id);
    for arg in loader_jvm {
        args.push(expand(arg, &loader_variables)?);
    }
    args.push(format!(
        "-Xms{}",
        launch.java_options.min_ram.as_deref().unwrap_or("2G")
    ));
    args.push(format!(
        "-Xmx{}",
        launch.java_options.max_ram.as_deref().unwrap_or("4G")
    ));
    args.extend(launch.java_options.jvm_options.unwrap_or_default());
    args.push(loader_version.main_class);
    for arg in vanilla_game.into_iter().chain(loader_game) {
        args.push(expand(arg, &variables)?);
    }
    if launch.settings.auto_connect.unwrap_or(true)
        && server.auto_connect
        && !server.address.is_empty()
    {
        args.push("--quickPlayMultiplayer".into());
        args.push(if server.address.contains(':') {
            server.address
        } else {
            format!("{}:25565", server.address)
        });
    }
    if fullscreen {
        args.push("--fullscreen".into());
    }
    args.extend(write_mod_list(&instance, &loader)?);
    set_game_options(&instance, fullscreen)?;
    let _ = progress.send("게임 시작 중");
    let mut child = Command::new(java)
        .args(args)
        .current_dir(instance)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let pid = child.id();
    std::thread::spawn(move || {
        let _ = child.wait();
        drop(natives);
        drop(running);
    });
    Ok(pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_multiple_placeholders_without_reprocessing_user_input() {
        let variables = BTreeMap::from([
            ("path", "test ${value}/file".into()),
            ("name", "alice".into()),
        ]);
        assert_eq!(
            expand("-Dtest=${path}/${name}", &variables).unwrap(),
            "-Dtest=test ${value}/file/alice"
        );
        assert!(expand("${missing}", &variables).is_err());
    }

    #[test]
    fn preserves_existing_options_and_line_endings() {
        let instance = tempfile::tempdir().unwrap();
        let path = instance.path().join("options.txt");
        fs::write(
            &path,
            "guiScale:4\r\ntutorialStep:movement\r\nresourcePacks:[]",
        )
        .unwrap();
        set_game_options(instance.path(), true).unwrap();
        let updated = fs::read_to_string(&path).unwrap();
        assert_eq!(
            updated,
            "guiScale:4\r\ntutorialStep:none\r\nresourcePacks:[]\r\nfullscreen:true\r\n"
        );
        set_game_options(instance.path(), true).unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), updated);
    }
}
