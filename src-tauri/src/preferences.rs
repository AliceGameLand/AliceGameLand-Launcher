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

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GameSettings {
    pub res_width: Option<u32>,
    pub res_height: Option<u32>,
    pub fullscreen: Option<bool>,
    pub auto_connect: Option<bool>,
    pub launch_detached: Option<bool>,
}

#[derive(Default, Deserialize, Serialize)]
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

    pub fn game_settings(&self) -> &GameSettings {
        &self.settings.game
    }

    pub fn update_game_settings(
        &mut self,
        settings: GameSettings,
        java: Option<(String, JavaOptions)>,
        path: &Path,
    ) -> io::Result<()> {
        let previous = std::mem::replace(&mut self.settings.game, settings);
        let previous_java = java.map(|(server_id, options)| {
            let previous = self.java_config.insert(server_id.clone(), options);
            (server_id, previous)
        });
        if let Err(error) = self.save(path) {
            self.settings.game = previous;
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

    #[test]
    fn saves_java_and_game_settings_together() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("settings.json");
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
            .update_game_settings(game, Some(("alice".into(), java)), &path)
            .unwrap();

        let mut reopened = Preferences::load(&path, root.path()).unwrap();
        let error = reopened.update_game_settings(
            GameSettings::default(),
            Some(("alice".into(), JavaOptions::default())),
            &path.join("not-a-directory"),
        );
        assert!(error.is_err());
        let (game, java) = reopened.launch_settings("alice");
        assert_eq!(game.fullscreen, Some(true));
        assert_eq!(java.min_ram.as_deref(), Some("3072M"));
        assert_eq!(java.max_ram.as_deref(), Some("6G"));
        assert_eq!(java.jvm_options.unwrap(), ["-Dexample=one value"]);
    }
}
