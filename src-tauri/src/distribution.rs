use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use reqwest::{blocking::Client, Url};
use serde::{Deserialize, Serialize};

use crate::artifacts::{self, safe_segment, Download, Hash, PlannedArtifact, Result};

const BUNDLED: &str = include_str!("../../app/assets/distribution.json");
const REMOTE: &str = "https://github.com/AliceGameLand/AliceGameLand-Distribution/releases/latest/download/distribution.json";
const MANIFEST_LIMIT: u64 = 8 * 1024 * 1024;

#[derive(Deserialize)]
pub struct Distribution {
    pub servers: Vec<Server>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: String,
    pub name: String,
    pub minecraft_version: String,
    #[serde(default, skip_serializing)]
    pub address: String,
    #[serde(default, skip_serializing, rename = "autoconnect")]
    pub auto_connect: bool,
    #[serde(default)]
    pub main_server: bool,
    #[serde(skip_serializing)]
    modules: Vec<Module>,
}

#[derive(Clone, Deserialize)]
struct Module {
    id: String,
    #[serde(rename = "type")]
    kind: Kind,
    artifact: Artifact,
    #[serde(default = "enabled")]
    classpath: bool,
    #[serde(default, rename = "subModules")]
    children: Vec<Module>,
}

#[derive(Clone, Deserialize)]
enum Kind {
    Library,
    NeoForge,
    Fabric,
    VersionManifest,
    File,
    Forge,
    ForgeHosted,
    LiteLoader,
    ForgeMod,
    LiteMod,
    FabricMod,
}

#[derive(Clone, Deserialize)]
struct Artifact {
    url: Url,
    size: u64,
    #[serde(rename = "MD5")]
    md5: Option<String>,
    path: Option<String>,
}

impl Distribution {
    fn parse(raw: &[u8]) -> Result<Self> {
        let mut distribution: Self = serde_json::from_slice(raw)?;
        if distribution.servers.is_empty() {
            return Err("Empty distribution".into());
        }
        let mut ids = HashSet::new();
        for server in &mut distribution.servers {
            if !safe_segment(&server.id) || !ids.insert(server.id.clone()) {
                return Err("Invalid or duplicate server ID".into());
            }
            if server.id == "AliceDiamond-26.2" && server.name == "버컴퍼니 다이아몬드 대결"
            {
                server.name = "버컴퍼니 다이아 대결".into();
            }
            server.files()?;
        }
        Ok(distribution)
    }

    pub fn local(path: &Path) -> Result<Self> {
        match fs::File::open(path) {
            Ok(file) => {
                let mut raw = Vec::new();
                file.take(MANIFEST_LIMIT + 1).read_to_end(&mut raw)?;
                if raw.len() as u64 <= MANIFEST_LIMIT {
                    if let Ok(distribution) = Self::parse(&raw) {
                        return Ok(distribution);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Self::parse(BUNDLED.as_bytes())
    }

    pub fn refresh(client: &Client, path: &Path) -> Result<Self> {
        let remote = (|| -> Result<_> {
            let mut raw = Vec::new();
            client
                .get(REMOTE)
                .send()?
                .error_for_status()?
                .take(MANIFEST_LIMIT + 1)
                .read_to_end(&mut raw)?;
            if raw.len() as u64 > MANIFEST_LIMIT {
                return Err("Distribution download exceeded its size limit".into());
            }
            Ok((Self::parse(&raw)?, raw))
        })();

        match remote {
            Ok((distribution, raw)) => {
                let parent = path
                    .parent()
                    .ok_or("Distribution cache directory is missing")?;
                fs::create_dir_all(parent)?;
                let mut stage = tempfile::NamedTempFile::new_in(parent)?;
                stage.write_all(&raw)?;
                stage.as_file().sync_all()?;
                stage.persist(path)?;
                Ok(distribution)
            }
            Err(_) => Self::local(path),
        }
    }
}

fn enabled() -> bool {
    true
}

pub struct Loader {
    pub manifest: PathBuf,
    pub installer: Option<(String, PathBuf)>,
    pub libraries: Vec<(String, PathBuf)>,
    pub mods: Vec<String>,
}

pub fn library_key(coordinate: &str) -> Result<String> {
    let mut parts = coordinate.split('@').next().unwrap_or_default().split(':');
    let group = parts.next().ok_or("Missing Maven group")?;
    let artifact = parts.next().ok_or("Missing Maven artifact")?;
    parts.next().ok_or("Missing Maven version")?;
    Ok(match parts.next() {
        Some(classifier) => format!("{group}:{artifact}:{classifier}"),
        None => format!("{group}:{artifact}"),
    })
}
impl Server {
    pub fn loader(&self, root: &Path) -> Result<Loader> {
        let loader = self
            .modules
            .iter()
            .find(|module| matches!(module.kind, Kind::NeoForge | Kind::Fabric))
            .ok_or("No supported loader in the distribution")?;
        let manifest = loader
            .children
            .iter()
            .find(|module| matches!(module.kind, Kind::VersionManifest))
            .ok_or("Loader version manifest is missing")?
            .path(&self.id)?;
        let installer = if matches!(loader.kind, Kind::NeoForge) {
            let version = loader
                .id
                .split(':')
                .nth(2)
                .filter(|version| safe_segment(version))
                .ok_or("Invalid NeoForge version")?;
            Some((version.to_owned(), root.join(loader.path(&self.id)?)))
        } else {
            None
        };
        let mut libraries = Vec::new();
        let mut mods = Vec::new();
        for module in &self.modules {
            module.runtime_files(&self.id, root, &mut libraries, &mut mods)?;
        }
        Ok(Loader {
            manifest: root.join(manifest),
            installer,
            libraries,
            mods,
        })
    }

    pub fn files(&self) -> Result<Vec<PlannedArtifact>> {
        let mut files = Vec::new();
        for module in &self.modules {
            module.plan(&self.id, &mut files)?;
        }
        Ok(files)
    }
}

impl Module {
    fn runtime_files(
        &self,
        server_id: &str,
        root: &Path,
        libraries: &mut Vec<(String, PathBuf)>,
        mods: &mut Vec<String>,
    ) -> Result<()> {
        match self.kind {
            Kind::Fabric | Kind::Library if self.classpath => {
                libraries.push((library_key(&self.id)?, root.join(self.path(server_id)?)));
            }
            Kind::ForgeMod => mods.push(self.id.split('@').next().unwrap_or_default().to_owned()),
            Kind::FabricMod => mods.push(
                root.join(self.path(server_id)?)
                    .into_os_string()
                    .into_string()
                    .map_err(|_| "Mod path is not valid Unicode")?,
            ),
            _ => {}
        }
        for child in &self.children {
            child.runtime_files(server_id, root, libraries, mods)?;
        }
        Ok(())
    }

    fn path(&self, server_id: &str) -> Result<PathBuf> {
        Ok(if matches!(self.kind, Kind::VersionManifest) {
            if !safe_segment(&self.id) {
                return Err("Invalid loader manifest name".into());
            }
            PathBuf::from("common")
                .join("versions")
                .join(&self.id)
                .join(format!("{}.json", self.id))
        } else {
            let relative = match &self.artifact.path {
                Some(path) => artifacts::relative_path(path)?,
                None => maven_path(&self.id)?,
            };
            let base = match self.kind {
                Kind::Library
                | Kind::NeoForge
                | Kind::Fabric
                | Kind::Forge
                | Kind::ForgeHosted
                | Kind::LiteLoader => PathBuf::from("common").join("libraries"),
                Kind::ForgeMod | Kind::LiteMod => PathBuf::from("common").join("modstore"),
                Kind::FabricMod => PathBuf::from("common").join("mods").join("fabric"),
                Kind::File => PathBuf::from("instances").join(server_id),
                Kind::VersionManifest => unreachable!(),
            };
            base.join(relative)
        })
    }

    fn plan(&self, server_id: &str, files: &mut Vec<PlannedArtifact>) -> Result<()> {
        let relative_path = self.path(server_id)?;
        let download = Download {
            url: self.artifact.url.clone(),
            size: Some(self.artifact.size),
            hash: self.artifact.md5.clone().map(Hash::Md5),
        };
        artifacts::validate_metadata(&download)?;
        files.push(PlannedArtifact {
            download,
            relative_path,
        });
        for child in &self.children {
            child.plan(server_id, files)?;
        }
        Ok(())
    }
}

fn maven_path(id: &str) -> Result<PathBuf> {
    let (coordinate, extension) = id.split_once('@').unwrap_or((id, "jar"));
    let parts: Vec<_> = coordinate.split(':').collect();
    let [group, artifact, version, rest @ ..] = parts.as_slice() else {
        return Err("Invalid Maven coordinate".into());
    };
    if rest.len() > 1
        || !group.split('.').all(safe_segment)
        || !safe_segment(artifact)
        || !safe_segment(version)
        || !safe_segment(extension)
        || rest.iter().any(|value| !safe_segment(value))
    {
        return Err("Invalid Maven coordinate".into());
    }
    let classifier = rest
        .first()
        .map(|value| format!("-{value}"))
        .unwrap_or_default();
    let mut path = PathBuf::new();
    path.extend(group.split('.'));
    Ok(path
        .join(artifact)
        .join(version)
        .join(format!("{artifact}-{version}{classifier}.{extension}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_actual_loader_libraries_and_user_settings() {
        let distribution = Distribution::parse(BUNDLED.as_bytes()).unwrap();
        assert_eq!(distribution.servers.len(), 6);
        let server = distribution
            .servers
            .iter()
            .find(|server| server.id == "AliceDiamond-26.2")
            .unwrap();
        let files = server.files().unwrap();
        assert!(files.iter().any(|file| file.relative_path == Path::new("common/libraries/net/neoforged/neoforge/26.2.0.60/neoforge-26.2.0.60-installer.jar")));
        let options = files
            .iter()
            .find(|file| file.relative_path == Path::new("instances/AliceDiamond-26.2/options.txt"))
            .unwrap();
        assert!(options.download.hash.is_none());
        let iris = files
            .iter()
            .find(|file| {
                file.relative_path
                    == Path::new("instances/AliceDiamond-26.2/config/iris.properties")
            })
            .unwrap();
        assert!(iris.download.hash.is_none());
    }

    #[test]
    fn bundled_servers_resolve_their_loader_and_runtime_libraries() {
        let distribution = Distribution::parse(BUNDLED.as_bytes()).unwrap();
        for server in distribution.servers {
            let loader = server.loader(Path::new("fixture")).unwrap();
            assert!(loader.manifest.starts_with("fixture/common/versions"));
            assert!(!loader.libraries.is_empty());
            if server.minecraft_version == "26.3" {
                assert!(loader.installer.is_none());
                assert!(loader
                    .libraries
                    .iter()
                    .any(|(key, _)| key == "net.fabricmc:fabric-loader"));
            } else {
                assert!(loader.installer.is_some());
                assert!(!loader
                    .libraries
                    .iter()
                    .any(|(key, _)| key.starts_with("net.neoforged:neoforge:")));
            }
        }
    }
    #[test]
    fn rejects_paths_that_escape_the_game_directory() {
        for value in [
            "../secret",
            "/secret",
            "C:/secret",
            "mods/../secret",
            "mods\\secret",
            "mods/.. /secret",
        ] {
            assert!(artifacts::relative_path(value).is_err(), "{value}");
        }
        assert!(maven_path("org.example:../bad:1.0").is_err());
    }
}
