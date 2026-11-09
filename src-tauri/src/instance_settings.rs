use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{artifacts, distribution::Server};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstanceFolder {
    Mods,
    Shaderpacks,
}

impl InstanceFolder {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Mods => "mods",
            Self::Shaderpacks => "shaderpacks",
        }
    }
}

#[derive(Serialize)]
struct LocalMod {
    file: PathBuf,
    name: String,
    enabled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LocalModAction {
    Enable,
    Disable,
    Trash,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModSettings {
    required: Vec<String>,
    local: Vec<LocalMod>,
    shaders: Vec<String>,
    selected_shader: Option<String>,
    shader_config_ready: bool,
}

const SHADER_OVERRIDE: &[u8] = b"\n\n# AliceGameLand launcher shader selection\n";
const IRIS_CONFIG_LIMIT: u64 = 64 * 1024;

fn mod_file(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jar" | "zip" | "litemod"
            )
        })
}

fn filenames(directory: &Path) -> io::Result<Vec<String>> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            if let Ok(name) = entry.file_name().into_string() {
                names.push(name);
            }
        }
    }
    names.sort_unstable();
    Ok(names)
}

impl ModSettings {
    pub fn load(server: &Server, instance: &Path) -> artifacts::Result<Self> {
        let (required, managed) = managed_mods(server)?;
        let local = local_mods(server, instance, &managed)?;
        let shaders = shader_files(instance)?;
        let config = iris_config(instance)?;
        let selected_shader = config
            .as_deref()
            .map(javaproperties::Properties::parse)
            .transpose()?
            .and_then(|settings| {
                if settings.get("enableShaders") == Some("false") {
                    return None;
                }
                settings
                    .get("shaderPack")
                    .filter(|pack| !pack.is_empty())
                    .map(str::to_owned)
            });
        Ok(Self {
            required,
            local,
            shaders,
            selected_shader,
            shader_config_ready: config.is_some(),
        })
    }
}

fn managed_mods(server: &Server) -> artifacts::Result<(Vec<String>, BTreeSet<String>)> {
    let server_mods = Path::new("instances").join(&server.id).join("mods");
    let mut managed = BTreeSet::new();
    let mut required = BTreeSet::new();
    for artifact in server.files()? {
        let path = artifact.relative_path;
        if !mod_file(&path) {
            continue;
        }
        if let Ok(relative) = path.strip_prefix(&server_mods) {
            // Protect alternate casing on case-insensitive game volumes.
            managed.insert(relative.to_string_lossy().to_lowercase());
        } else if !path.starts_with("common/modstore") && !path.starts_with("common/mods/fabric") {
            continue;
        }
        if let Some(name) = path.file_name().and_then(OsStr::to_str) {
            required.insert(name.to_owned());
        }
    }
    Ok((required.into_iter().collect(), managed))
}

fn local_mods(
    server: &Server,
    instance: &Path,
    managed: &BTreeSet<String>,
) -> io::Result<Vec<LocalMod>> {
    let mods = instance.join("mods");
    let mut local = Vec::new();
    scan_local(&mods, Path::new(""), managed, &mut local)?;
    if artifacts::safe_segment(&server.minecraft_version) {
        scan_local(
            &mods,
            Path::new(&server.minecraft_version),
            managed,
            &mut local,
        )?;
    }
    Ok(local)
}

fn scan_local(
    mods: &Path,
    scope: &Path,
    managed: &BTreeSet<String>,
    output: &mut Vec<LocalMod>,
) -> io::Result<()> {
    for filename in filenames(&mods.join(scope))? {
        let base = filename.strip_suffix(".disabled").unwrap_or(&filename);
        let path = scope.join(base);
        if mod_file(&path) && !managed.contains(&path.to_string_lossy().to_lowercase()) {
            output.push(LocalMod {
                file: scope.join(&filename),
                name: path.to_string_lossy().into_owned(),
                enabled: filename == base,
            });
        }
    }
    Ok(())
}

pub fn change_local_mod(
    server: &Server,
    instance: &Path,
    requested: &Path,
    action: LocalModAction,
) -> artifacts::Result<Option<PathBuf>> {
    let (_, managed) = managed_mods(server)?;
    let entry = local_mods(server, instance, &managed)?
        .into_iter()
        .find(|entry| entry.file == requested)
        .ok_or("개인 모드 파일을 찾지 못했어요. 목록을 새로고침해 주세요.")?;
    let instance = instance.canonicalize()?;
    let path = instance.join("mods").join(&entry.file);
    let parent = path.parent().ok_or("모드 폴더를 찾지 못했어요.")?;
    if parent.canonicalize()?.as_path() != parent || !fs::symlink_metadata(&path)?.is_file() {
        return Err("개인 모드 폴더 안의 파일만 변경할 수 있어요.".into());
    }
    let enabled = match action {
        LocalModAction::Enable => true,
        LocalModAction::Disable => false,
        LocalModAction::Trash => {
            move_to_trash(&path)?;
            return Ok(None);
        }
    };
    if enabled == entry.enabled {
        return Ok(Some(entry.file));
    }
    let filename = entry
        .file
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or("잘못된 모드 파일 이름이에요.")?;
    let destination = if enabled {
        filename
            .strip_suffix(".disabled")
            .ok_or("이미 켜져 있는 모드예요.")?
            .to_owned()
    } else {
        format!("{filename}.disabled")
    };
    rename_mod(&path, &parent.join(&destination))?;
    Ok(Some(entry.file.with_file_name(destination)))
}

#[cfg(windows)]
fn rename_mod(source: &Path, target: &Path) -> artifacts::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{core::PCWSTR, Win32::Storage::FileSystem::MoveFileW};
    let source: Vec<u16> = source.as_os_str().encode_wide().chain([0]).collect();
    let target: Vec<u16> = target.as_os_str().encode_wide().chain([0]).collect();
    unsafe { MoveFileW(PCWSTR(source.as_ptr()), PCWSTR(target.as_ptr()))? };
    Ok(())
}

#[cfg(target_os = "macos")]
fn rename_mod(source: &Path, target: &Path) -> artifacts::Result<()> {
    use rustix::fs::{renameat_with, RenameFlags, CWD};
    Ok(renameat_with(
        CWD,
        source,
        CWD,
        target,
        RenameFlags::NOREPLACE,
    )?)
}

#[cfg(windows)]
fn move_to_trash(path: &Path) -> artifacts::Result<()> {
    Ok(trash::delete(path)?)
}

#[cfg(target_os = "macos")]
fn move_to_trash(path: &Path) -> artifacts::Result<()> {
    use trash::macos::{DeleteMethod, TrashContextExtMacos};
    let mut context = trash::TrashContext::new();
    context.set_delete_method(DeleteMethod::NsFileManager);
    Ok(context.delete(path)?)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn rename_mod(_: &Path, _: &Path) -> artifacts::Result<()> {
    Err(io::Error::from(io::ErrorKind::Unsupported).into())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn move_to_trash(_: &Path) -> artifacts::Result<()> {
    Err(io::Error::from(io::ErrorKind::Unsupported).into())
}

fn shader_files(instance: &Path) -> io::Result<Vec<String>> {
    Ok(filenames(&instance.join("shaderpacks"))?
        .into_iter()
        .filter(|name| name.to_ascii_lowercase().ends_with(".zip"))
        .collect())
}

fn iris_config(instance: &Path) -> artifacts::Result<Option<Vec<u8>>> {
    let path = instance.join("config/iris.properties");
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file()
        || metadata.len() > IRIS_CONFIG_LIMIT
        || !path.canonicalize()?.starts_with(instance.canonicalize()?)
    {
        return Err("Invalid Iris settings file".into());
    }
    Ok(Some(fs::read(path)?))
}

pub fn select_shader(instance: &Path, shader: Option<&str>) -> artifacts::Result<()> {
    if let Some(shader) = shader {
        if !shader_files(instance)?.iter().any(|name| name == shader) {
            return Err("선택한 셰이더팩을 찾지 못했어요.".into());
        }
    }
    let original =
        iris_config(instance)?.ok_or("게임을 한 번 실행한 뒤 셰이더를 설정해 주세요.")?;
    let settings = javaproperties::Properties::parse(&original)?;
    let pack = shader
        .or_else(|| settings.get("shaderPack"))
        .unwrap_or_default();
    let enabled = if shader.is_some() { "true" } else { "false" };
    // Java properties use the last value for a key. Only replace our own trailing override.
    let suffix = original
        .windows(SHADER_OVERRIDE.len())
        .rposition(|bytes| bytes == SHADER_OVERRIDE);
    let mut base = original.as_slice();
    if let Some(position) = suffix {
        let tail = &original[position + SHADER_OVERRIDE.len()..];
        let lines: Vec<_> = tail.split(|&byte| byte == b'\n').collect();
        if matches!(lines.as_slice(), [enabled, pack, rest] if enabled.starts_with(b"enableShaders=") && pack.starts_with(b"shaderPack=") && rest.is_empty())
        {
            base = &original[..position];
        }
    }
    let config = instance.join("config");
    let mut stage = tempfile::NamedTempFile::new_in(&config)?;
    stage.write_all(base)?;
    stage.write_all(SHADER_OVERRIDE)?;
    writeln!(stage, "enableShaders={enabled}")?;
    writeln!(
        stage,
        "{}",
        javaproperties::join_key_value("shaderPack", pack, "=")
    )?;
    if stage.as_file().metadata()?.len() > IRIS_CONFIG_LIMIT {
        return Err("Iris settings file exceeds its size limit".into());
    }
    stage.as_file().sync_all()?;
    if iris_config(instance)?.as_deref() != Some(original.as_slice()) {
        return Err("Iris settings changed while saving".into());
    }
    stage.persist(config.join("iris.properties"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distribution::Distribution;

    #[test]
    fn lists_managed_mods_separately_from_user_files() {
        let root = tempfile::tempdir().unwrap();
        let distribution = Distribution::local(&root.path().join("missing.json")).unwrap();
        let server = distribution
            .servers
            .iter()
            .find(|server| server.id == "BurcomPvP-26.3")
            .unwrap();
        let instance = root.path();
        fs::create_dir_all(instance.join("mods/26.3")).unwrap();
        fs::create_dir_all(instance.join("shaderpacks")).unwrap();
        for path in [
            "mods/fabric-api-0.163.0+26.3.jar",
            "mods/fabric-api-0.163.0+26.3.jar.disabled",
            "mods/26.3/local.jar.disabled",
            "mods/client.jar",
            "mods/notes.txt",
            "shaderpacks/Pack.zip",
            "shaderpacks/README.txt",
        ] {
            fs::write(instance.join(path), []).unwrap();
        }
        let settings = ModSettings::load(server, instance).unwrap();
        assert!(settings
            .required
            .iter()
            .any(|name| name == "fabric-api-0.163.0+26.3.jar"));
        assert_eq!(settings.local.len(), 2);
        assert_eq!(settings.local[0].name, "client.jar");
        assert!(settings.local[0].enabled);
        assert!(!settings.local[1].enabled);
        assert_eq!(settings.shaders, ["Pack.zip"]);
        assert!(!settings.shader_config_ready);
        assert!(select_shader(instance, Some("Pack.zip")).is_err());
        assert!(!instance.join("config/iris.properties").exists());
    }

    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn toggles_local_mods_without_overwriting_or_touching_managed_files() {
        let root = tempfile::tempdir().unwrap();
        let distribution = Distribution::local(&root.path().join("missing.json")).unwrap();
        let server = distribution
            .servers
            .iter()
            .find(|server| server.id == "BurcomPvP-26.3")
            .unwrap();
        let instance = root.path();
        let mods = instance.join("mods");
        fs::create_dir_all(mods.join("26.3")).unwrap();
        let enabled = Path::new("26.3").join("테스트.jar");
        let disabled = enabled.with_file_name("테스트.jar.disabled");
        fs::write(mods.join(&enabled), b"mod").unwrap();
        fs::write(mods.join(&disabled), b"collision").unwrap();
        assert!(change_local_mod(server, instance, &enabled, LocalModAction::Disable).is_err());
        assert_eq!(fs::read(mods.join(&enabled)).unwrap(), b"mod");
        assert_eq!(fs::read(mods.join(&disabled)).unwrap(), b"collision");
        fs::remove_file(mods.join(&disabled)).unwrap();
        assert_eq!(
            change_local_mod(server, instance, &enabled, LocalModAction::Disable).unwrap(),
            Some(disabled.clone())
        );
        assert!(!mods.join(&enabled).exists());
        assert_eq!(
            change_local_mod(server, instance, &disabled, LocalModAction::Enable).unwrap(),
            Some(enabled.clone())
        );
        assert_eq!(fs::read(mods.join(&enabled)).unwrap(), b"mod");
        let managed = Path::new("FABRIC-API-0.163.0+26.3.JAR");
        fs::write(mods.join(managed), b"required").unwrap();
        assert!(change_local_mod(server, instance, managed, LocalModAction::Trash).is_err());
        assert!(change_local_mod(
            server,
            instance,
            Path::new("../world.jar"),
            LocalModAction::Trash
        )
        .is_err());
        assert_eq!(fs::read(mods.join(managed)).unwrap(), b"required");
    }

    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    #[ignore = "uses the host Trash; run on a disposable CI runner"]
    fn sends_only_the_selected_mod_to_os_trash() {
        let root = tempfile::tempdir().unwrap();
        let distribution = Distribution::local(&root.path().join("missing.json")).unwrap();
        let server = distribution.servers.first().unwrap();
        let mods = root.path().join("mods");
        fs::create_dir(&mods).unwrap();
        let selected = format!(
            "alice-ci-{}.jar",
            root.path().file_name().unwrap().to_string_lossy()
        );
        fs::write(mods.join(&selected), b"selected").unwrap();
        fs::write(mods.join("other.jar"), b"other").unwrap();
        assert_eq!(
            change_local_mod(
                server,
                root.path(),
                Path::new(&selected),
                LocalModAction::Trash
            )
            .unwrap(),
            None
        );
        assert!(!mods.join(selected).exists());
        assert_eq!(fs::read(mods.join("other.jar")).unwrap(), b"other");
    }

    #[test]
    fn saves_shader_selection_without_altering_existing_iris_properties() {
        let root = tempfile::tempdir().unwrap();
        let instance = root.path();
        fs::create_dir_all(instance.join("config")).unwrap();
        fs::create_dir_all(instance.join("shaderpacks")).unwrap();
        let config = instance.join("config/iris.properties");
        let original = b"# user configuration\r\nshaderPack=old.zip\r\nenableShaders=true\r\nmaxShadowRenderDistance=24";
        fs::write(&config, original).unwrap();
        let pack = "한글 🌙.zip";
        fs::write(instance.join("shaderpacks").join(pack), []).unwrap();
        select_shader(instance, Some(pack)).unwrap();
        let updated = fs::read(&config).unwrap();
        assert!(updated.starts_with(original));
        assert_eq!(
            javaproperties::Properties::parse(&updated)
                .unwrap()
                .get("shaderPack"),
            Some(pack)
        );
        select_shader(instance, None).unwrap();
        let disabled = fs::read(&config).unwrap();
        assert!(disabled.starts_with(original));
        assert_eq!(
            disabled
                .windows(SHADER_OVERRIDE.len())
                .filter(|bytes| *bytes == SHADER_OVERRIDE)
                .count(),
            1
        );
        let props = javaproperties::Properties::parse(&disabled).unwrap();
        assert_eq!(props.get("enableShaders"), Some("false"));
        assert_eq!(props.get("maxShadowRenderDistance"), Some("24"));
        assert_eq!(props.get("shaderPack"), Some(pack));
        assert!(select_shader(instance, Some("../missing.zip")).is_err());
        assert_eq!(fs::read(config).unwrap(), disabled);
    }
}
