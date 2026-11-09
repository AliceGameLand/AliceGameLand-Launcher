use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs, io,
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
    name: String,
    enabled: bool,
}

#[derive(Serialize)]
pub struct ModSettings {
    required: Vec<String>,
    local: Vec<LocalMod>,
    shaders: Vec<String>,
}

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
        let server_mods = Path::new("instances").join(&server.id).join("mods");
        let mut managed_in_instance = BTreeSet::new();
        let mut required = BTreeSet::new();
        for artifact in server.files()? {
            let path = artifact.relative_path;
            if !mod_file(&path) {
                continue;
            }
            if let Ok(relative) = path.strip_prefix(&server_mods) {
                managed_in_instance.insert(relative.to_path_buf());
            } else if !path.starts_with("common/modstore")
                && !path.starts_with("common/mods/fabric")
            {
                continue;
            }
            if let Some(name) = path.file_name().and_then(OsStr::to_str) {
                required.insert(name.to_owned());
            }
        }

        let mods = instance.join("mods");
        let mut local = Vec::new();
        scan_local(&mods, Path::new(""), &managed_in_instance, &mut local)?;
        if artifacts::safe_segment(&server.minecraft_version) {
            scan_local(
                &mods,
                Path::new(&server.minecraft_version),
                &managed_in_instance,
                &mut local,
            )?;
        }
        let shaders = filenames(&instance.join("shaderpacks"))?
            .into_iter()
            .filter(|name| name.to_ascii_lowercase().ends_with(".zip"))
            .collect();
        Ok(Self {
            required: required.into_iter().collect(),
            local,
            shaders,
        })
    }
}

fn scan_local(
    mods: &Path,
    scope: &Path,
    managed: &BTreeSet<PathBuf>,
    output: &mut Vec<LocalMod>,
) -> io::Result<()> {
    for filename in filenames(&mods.join(scope))? {
        let base = filename.strip_suffix(".disabled").unwrap_or(&filename);
        let path = scope.join(base);
        if mod_file(&path) && !managed.contains(&path) {
            output.push(LocalMod {
                name: path.to_string_lossy().into_owned(),
                enabled: filename == base,
            });
        }
    }
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
    }
}
