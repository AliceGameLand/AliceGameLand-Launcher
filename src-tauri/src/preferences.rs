use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const LEGACY_APP: &str = "앨리스의 게임랜드";

pub(crate) fn legacy_configs(data_root: &Path) -> [PathBuf; 2] {
    [
        data_root.join(LEGACY_APP).join("config.json"),
        data_root.join(".alicegameland").join("config.json"),
    ]
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub selected_server: Option<String>,
    settings: Settings,
    mod_configurations: Vec<Value>,
    java_config: BTreeMap<String, JavaOptions>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    game: GameSettings,
    launcher: LauncherSettings,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GameSettings {
    pub res_width: Option<u32>,
    pub res_height: Option<u32>,
    pub fullscreen: Option<bool>,
    pub auto_connect: Option<bool>,
    pub launch_detached: Option<bool>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct LauncherSettings {
    allow_prerelease: Option<bool>,
    data_directory: Option<PathBuf>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct JavaOptions {
    #[serde(rename = "minRAM")]
    pub min_ram: Option<String>,
    #[serde(rename = "maxRAM")]
    pub max_ram: Option<String>,
    pub jvm_options: Option<Vec<String>>,
}

pub fn heap_megabytes(value: &str) -> Option<u32> {
    let (number, unit) = value.split_at_checked(value.len().checked_sub(1)?)?;
    let quantity: u32 = number.parse().ok()?;
    match unit {
        "G" | "g" => quantity.checked_mul(1024),
        "M" | "m" => Some(quantity),
        _ => None,
    }
}

impl JavaOptions {
    pub fn effective_heap(&self, recommended: u32) -> (String, String) {
        let min = self.min_ram.clone().unwrap_or_else(|| {
            let max = self
                .max_ram
                .as_deref()
                .and_then(heap_megabytes)
                .unwrap_or(recommended);
            format!("{}M", recommended.min(max))
        });
        let max = self.max_ram.clone().unwrap_or_else(|| {
            let min = self
                .min_ram
                .as_deref()
                .and_then(heap_megabytes)
                .unwrap_or(recommended);
            format!("{}M", recommended.max(min))
        });
        (min, max)
    }
}

impl Preferences {
    pub fn game_data_dir(&self) -> Option<&Path> {
        self.settings.launcher.data_directory.as_deref()
    }
    pub fn launch_settings(&self, server_id: &str) -> (GameSettings, JavaOptions) {
        (
            self.settings.game.clone(),
            self.java_config.get(server_id).cloned().unwrap_or_default(),
        )
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn allow_prerelease(&self) -> bool {
        self.settings.launcher.allow_prerelease.unwrap_or(false)
    }

    pub fn update_game_settings(
        &mut self,
        settings: GameSettings,
        java: Option<(String, JavaOptions)>,
        data_directory: Option<PathBuf>,
        allow_prerelease: Option<bool>,
        path: &Path,
    ) -> io::Result<()> {
        let previous = std::mem::replace(&mut self.settings.game, settings);
        let previous_java = java.map(|(server_id, options)| {
            let previous = self.java_config.insert(server_id.clone(), options);
            (server_id, previous)
        });
        let previous_directory = data_directory
            .map(|directory| self.settings.launcher.data_directory.replace(directory));
        let previous_prerelease = allow_prerelease
            .map(|allowed| self.settings.launcher.allow_prerelease.replace(allowed));
        if let Err(error) = self.save(path) {
            self.settings.game = previous;
            if let Some(directory) = previous_directory {
                self.settings.launcher.data_directory = directory;
            }
            if let Some(allowed) = previous_prerelease {
                self.settings.launcher.allow_prerelease = allowed;
            }
            match previous_java {
                Some((server_id, Some(options))) => {
                    self.java_config.insert(server_id, options);
                }
                Some((server_id, None)) => {
                    self.java_config.remove(&server_id);
                }
                None => {}
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn load(current: &Path, data_root: &Path) -> io::Result<Self> {
        if let Some(preferences) = read(current)? {
            return Ok(preferences);
        }

        let mut preferences = Self::default();
        let mut imported_legacy = false;
        for legacy in legacy_configs(data_root) {
            if let Some(imported) = read(&legacy)? {
                preferences = imported;
                imported_legacy = true;
                break;
            }
        }
        preferences
            .settings
            .launcher
            .data_directory
            .get_or_insert_with(|| data_root.join(".alicegameland"));
        if !imported_legacy {
            return Ok(preferences);
        }

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
    fn fresh_profile_saves_only_after_preferences_change() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("settings.json");
        let mut preferences = Preferences::load(&path, root.path()).unwrap();
        assert_eq!(
            preferences.game_data_dir(),
            Some(root.path().join(".alicegameland").as_path())
        );
        assert!(!path.exists());

        preferences.selected_server = Some("alice".into());
        preferences.save(&path).unwrap();
        assert_eq!(
            Preferences::load(&path, root.path())
                .unwrap()
                .selected_server
                .as_deref(),
            Some("alice")
        );
    }

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

    #[test]
    fn saves_java_game_and_directory_settings_together() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("settings.json");
        let directory = root.path().join("게임 폴더");
        let mut preferences = Preferences::default();
        let game = GameSettings {
            fullscreen: Some(true),
            ..GameSettings::default()
        };
        let java = JavaOptions {
            min_ram: Some("3072M".into()),
            max_ram: Some("6G".into()),
            jvm_options: Some(vec!["-Dexample=one value".into()]),
        };
        preferences
            .update_game_settings(
                game,
                Some(("alice".into(), java)),
                Some(directory.clone()),
                Some(true),
                &path,
            )
            .unwrap();

        let mut reopened = Preferences::load(&path, root.path()).unwrap();
        let error = reopened.update_game_settings(
            GameSettings::default(),
            Some(("alice".into(), JavaOptions::default())),
            Some(root.path().join("another-folder")),
            Some(false),
            &path.join("not-a-directory"),
        );
        assert!(error.is_err());
        assert_eq!(reopened.game_data_dir(), Some(directory.as_path()));
        assert!(reopened.allow_prerelease());
        let (game, java) = reopened.launch_settings("alice");
        assert_eq!(game.fullscreen, Some(true));
        assert_eq!(java.min_ram.as_deref(), Some("3072M"));
        assert_eq!(java.max_ram.as_deref(), Some("6G"));
        assert_eq!(java.jvm_options.unwrap(), ["-Dexample=one value"]);

        let partial = JavaOptions {
            max_ram: Some("3G".into()),
            ..JavaOptions::default()
        };
        assert_eq!(partial.effective_heap(4096), ("3072M".into(), "3G".into()));
    }
}
