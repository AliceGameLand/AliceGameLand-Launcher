use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const LEGACY_APP: &str = "앨리스의 게임랜드";

#[derive(Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub selected_server: Option<String>,
    settings: Settings,
    mod_configurations: Vec<Value>,
    java_config: BTreeMap<String, JavaOptions>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
struct Settings {
    game: GameSettings,
    launcher: LauncherSettings,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct GameSettings {
    res_width: Option<u32>,
    res_height: Option<u32>,
    fullscreen: Option<bool>,
    auto_connect: Option<bool>,
    launch_detached: Option<bool>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct LauncherSettings {
    allow_prerelease: Option<bool>,
    data_directory: Option<PathBuf>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct JavaOptions {
    #[serde(rename = "minRAM")]
    min_ram: Option<String>,
    #[serde(rename = "maxRAM")]
    max_ram: Option<String>,
    jvm_options: Option<Vec<String>>,
}

impl Preferences {
    pub fn load(current: &Path, data_root: &Path) -> io::Result<Self> {
        if let Some(preferences) = read(current)? {
            return Ok(preferences);
        }

        let mut preferences = Self::default();
        for legacy in [data_root.join(LEGACY_APP), data_root.join(".alicegameland")] {
            if let Some(imported) = read(&legacy.join("config.json"))? {
                preferences = imported;
                break;
            }
        }
        preferences
            .settings
            .launcher
            .data_directory
            .get_or_insert_with(|| data_root.join(".alicegameland"));

        let staged = preferences.stage(current)?;
        match staged.persist_noclobber(current) {
            Ok(_) => Ok(preferences),
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
                read(current)?.ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
            }
            Err(error) => Err(error.error),
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        self.stage(path)?
            .persist(path)
            .map_err(|error| error.error)?;
        Ok(())
    }

    fn stage(&self, path: &Path) -> io::Result<tempfile::NamedTempFile> {
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
        fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut file, self)?;
        file.write_all(b"\n")?;
        file.as_file().sync_all()?;
        Ok(file)
    }
}

fn read(path: &Path) -> io::Result<Option<Preferences>> {
    match File::open(path) {
        Ok(file) => Ok(Some(serde_json::from_reader(file)?)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_game_settings_once_without_account_or_old_java_path() {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join(LEGACY_APP);
        fs::create_dir(&legacy).unwrap();
        let config = r#"{"selectedServer":"alice","settings":{"game":{"fullscreen":true},
            "launcher":{"dataDirectory":"existing-data"}},
            "javaConfig":{"alice":{"executable":"old/javaw.exe","minRAM":"4G"}},
            "authenticationDatabase":{"id":{"accessToken":"fixture-only"}}}"#;
        let legacy_file = legacy.join("config.json");
        fs::write(&legacy_file, config).unwrap();
        let path = root.path().join("tauri/settings.json");

        let mut preferences = Preferences::load(&path, root.path()).unwrap();
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.contains("existing-data"));
        assert!(saved.contains("4G"));
        assert!(!saved.contains("old/javaw.exe"));
        assert!(!saved.contains("fixture-only"));
        assert_eq!(fs::read_to_string(legacy_file).unwrap(), config);

        preferences.selected_server = Some("new".into());
        preferences.save(&path).unwrap();
        assert_eq!(
            Preferences::load(&path, root.path())
                .unwrap()
                .selected_server
                .as_deref(),
            Some("new")
        );
    }
}
