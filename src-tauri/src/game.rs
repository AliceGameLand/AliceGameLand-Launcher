use std::{
    collections::{BTreeMap, HashMap},
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
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
type AttachedChild = Arc<Mutex<Child>>;
static RUNNING: LazyLock<Mutex<HashMap<String, Option<AttachedChild>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

struct RunningGame(String);

impl RunningGame {
    fn reserve(server_id: String) -> Result<Self> {
        let mut running = RUNNING
            .lock()
            .map_err(|_| "실행 중인 게임을 확인하지 못했어요.")?;
        let std::collections::hash_map::Entry::Vacant(slot) = running.entry(server_id.clone())
        else {
            return Err("이 서버의 게임이 이미 실행 중이에요.".into());
        };
        slot.insert(None);
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

pub fn stop_attached() {
    let Ok(games) = RUNNING.lock() else { return };
    for child in games.values().flatten() {
        if let Ok(mut child) = child.lock() {
            let _ = child.kill();
            let _ = child.wait();
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
    let output = java::command(java)
        .arg("-Djava.awt.headless=true")
        .arg("-jar")
        .arg(installer.strip_prefix(common)?)
        .arg("--install-client")
        .arg(".")
        .current_dir(common)
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() || !nonempty_zip(&patched) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut details: Vec<_> = stderr
            .lines()
            .rev()
            .take(6)
            .chain(stdout.lines().rev().take(6))
            .collect();
        details.reverse();
        return Err(format!(
            "NeoForge {version} 설치를 완료하지 못했어요. ({})\n{}",
            output.status,
            details.join("\n")
        )
        .into());
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
    let path = path_text(if cfg!(windows) {
        path.strip_prefix(instance)?
    } else {
        &path
    })?;
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

struct PreparedLaunch {
    command: Command,
    natives: tempfile::TempDir,
}

fn prepare_launch(
    launch: Launch,
    session: GameSession,
    progress: &Channel<&'static str>,
) -> Result<PreparedLaunch> {
    let (server, java) = prepare_files(&launch.profile, &launch.root, &launch.server_id, progress)?;
    let common = launch.root.join("common");
    let instance = launch.root.join("instances").join(&server.id);
    // Windows Java converts command-line arguments through the system code page.
    let java_root = if cfg!(windows) {
        Path::new("../..")
    } else {
        &launch.root
    };
    let java_common = java_root.join("common");
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

    let mut native_dir = tempfile::Builder::new();
    native_dir.prefix("alice-natives-");
    let (natives, native_argument) = if cfg!(windows) {
        let natives = native_dir.tempdir_in(&common)?;
        let argument = java_common.join(natives.path().strip_prefix(&common)?);
        (natives, argument)
    } else {
        let natives = native_dir.tempdir()?;
        let argument = natives.path().to_path_buf();
        (natives, argument)
    };
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
    if cfg!(windows) {
        for path in &mut classpath {
            *path = java_root.join(path.strip_prefix(&launch.root)?);
        }
    }
    let classpath = env::join_paths(classpath)?
        .into_string()
        .map_err(|_| "클래스 경로를 읽지 못했어요.")?;
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
        (
            "game_directory",
            path_text(if cfg!(windows) {
                Path::new(".")
            } else {
                &instance
            })?,
        ),
        ("assets_root", path_text(&java_common.join("assets"))?),
        ("assets_index_name", vanilla.assets.clone()),
        (
            "resolution_width",
            launch.settings.res_width.unwrap_or(1280).to_string(),
        ),
        (
            "resolution_height",
            launch.settings.res_height.unwrap_or(720).to_string(),
        ),
        ("natives_directory", path_text(&native_argument)?),
        (
            "library_directory",
            path_text(&java_common.join("libraries"))?,
        ),
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
    let (min, max) = launch
        .java_options
        .effective_heap(server.recommended_heap());
    args.push(format!("-Xms{min}"));
    args.push(format!("-Xmx{max}"));
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
    let mut command = java::command(&java);
    command
        .args(args)
        .current_dir(instance)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    Ok(PreparedLaunch { command, natives })
}

pub fn play(launch: Launch, session: GameSession, progress: Channel<&'static str>) -> Result<u32> {
    let running = RunningGame::reserve(launch.server_id.clone())?;
    let _preparation = PREPARATION
        .try_lock()
        .map_err(|_| "다른 서버의 파일을 준비하고 있어요.")?;
    let detached = launch.settings.launch_detached.unwrap_or(true);
    let PreparedLaunch {
        mut command,
        natives,
    } = prepare_launch(launch, session, &progress)?;
    let _ = progress.send("게임 시작 중");
    let mut child = command.spawn()?;
    let pid = child.id();
    if detached {
        std::thread::spawn(move || {
            let _ = child.wait();
            drop(natives);
            drop(running);
        });
    } else {
        let attached = Arc::new(Mutex::new(child));
        if let Ok(mut games) = RUNNING.lock() {
            games.insert(running.0.clone(), Some(Arc::clone(&attached)));
        }
        std::thread::spawn(move || {
            loop {
                let status = attached
                    .lock()
                    .ok()
                    .and_then(|mut child| child.try_wait().ok());
                if !matches!(status, Some(None)) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            drop(natives);
            drop(running);
        });
    }
    Ok(pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "downloads the live Minecraft distribution and installs the NeoForge client loader"]
    fn prepares_real_fabric_and_neoforge_launches() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let profile = temporary.path().join("런처 프로필");
        let root = temporary.path().join("앨리스 게임 폴더");
        let progress = Channel::new(|message| {
            if let tauri::ipc::InvokeResponseBody::Json(stage) = message {
                eprintln!("{stage}");
            }
            Ok(())
        });
        for (server, main_class) in [
            ("AliceGameLand-26.2", "net.neoforged.fml.startup.Client"),
            (
                "BurcomPvP-26.3",
                "net.fabricmc.loader.impl.launch.knot.KnotClient",
            ),
        ] {
            let instance = root.join("instances").join(server);
            fs::create_dir_all(&instance)?;
            fs::write(
                instance.join("options.txt"),
                "guiScale:3\ntutorialStep:movement\n",
            )?;
            let launch = Launch {
                profile: profile.clone(),
                root: root.clone(),
                server_id: server.into(),
                launcher_version: "1.0.41".into(),
                settings: GameSettings::default(),
                java_options: JavaOptions::default(),
            };
            let session = GameSession {
                profile: crate::auth::Profile {
                    id: "00000000000000000000000000000000".into(),
                    name: "LaunchFixture".into(),
                },
                access_token: "fixture-only".into(),
                client_id: "fixture-only".into(),
                xuid: None,
            };
            let prepared = prepare_launch(launch, session, &progress)
                .map_err(|error| format!("{server}: {error}"))?;
            assert!(profile.join("distribution.json").is_file());
            let command = prepared.command;
            let args: Vec<_> = command.get_args().collect();
            let cached_java =
                java::cached_executable(&profile.join("java")).ok_or("Java cache is missing")?;
            assert_eq!(Path::new(command.get_program()), cached_java);
            assert_eq!(command.get_current_dir(), Some(instance.as_path()));
            let main = args
                .iter()
                .position(|arg| *arg == main_class)
                .ok_or("Loader entry point is missing")?;
            assert!(fs::read_dir(instance.join("mods"))?
                .filter_map(std::result::Result::ok)
                .any(|entry| entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "jar")));
            assert_eq!(
                args.iter().any(|arg| *arg == "-XstartOnFirstThread"),
                cfg!(target_os = "macos")
            );
            let classpath = args
                .windows(2)
                .find(|pair| pair[0] == "-cp")
                .ok_or("Launch classpath is missing")?[1];
            for jar in env::split_paths(classpath) {
                assert!(
                    instance.join(&jar).is_file(),
                    "Missing runtime library {}",
                    jar.display()
                );
            }
            if cfg!(windows) {
                assert!(args.iter().all(|arg| arg.as_encoded_bytes().is_ascii()));
            }
            assert!(fs::read_dir(prepared.natives.path().join("java"))?
                .next()
                .is_some());
            let options = fs::read_to_string(instance.join("options.txt"))?;
            assert!(options.contains("guiScale:3\n"));
            assert!(options.contains("tutorialStep:none\n"));
            fs::write(
                instance.join("CheckLaunch.java"),
                r#"class CheckLaunch {
    public static void main(String[] args) throws Exception {
        Class.forName(args[0], false, ClassLoader.getSystemClassLoader())
             .getMethod("main", String[].class);
    }
}"#,
            )?;
            let probe = java::command(&cached_java)
                .args(&args[..main])
                .args(["CheckLaunch.java", main_class])
                .current_dir(&instance)
                .output()?;
            assert!(
                probe.status.success(),
                "{server} could not load the loader entry point: {}",
                String::from_utf8_lossy(&probe.stderr)
            );
            println!(
                "Prepared {server} with verified Java, Minecraft, loader, natives and settings"
            );
        }
        Ok(())
    }

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
