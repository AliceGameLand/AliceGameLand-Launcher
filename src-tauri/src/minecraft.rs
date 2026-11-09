use std::{
    collections::{BTreeMap, HashSet},
    env, fs,
    io::Read,
    path::{Path, PathBuf},
};

use reqwest::{blocking::Client, Url};
use serde::Deserialize;

use crate::artifacts::{self, Download, Hash, PlannedArtifact, Result};

const INDEX: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const METADATA_LIMIT: u64 = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct VersionIndex {
    versions: Vec<IndexEntry>,
}

#[derive(Deserialize)]
struct IndexEntry {
    id: String,
    sha1: String,
    url: Url,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Version {
    id: String,
    downloads: ClientDownloads,
    asset_index: AssetIndex,
    libraries: Vec<Library>,
}

#[derive(Deserialize)]
struct ClientDownloads {
    client: FileDownload,
}

#[derive(Deserialize)]
struct FileDownload {
    sha1: String,
    size: u64,
    url: Url,
}

#[derive(Deserialize)]
struct AssetIndex {
    id: String,
    #[serde(flatten)]
    download: FileDownload,
}

#[derive(Deserialize)]
struct Assets {
    objects: BTreeMap<String, Asset>,
}

#[derive(Deserialize)]
struct Asset {
    hash: String,
    size: u64,
}

#[derive(Deserialize)]
struct Library {
    name: String,
    downloads: LibraryDownloads,
    #[serde(default)]
    rules: Vec<Rule>,
}

#[derive(Deserialize)]
struct LibraryDownloads {
    artifact: LibraryArtifact,
}

#[derive(Deserialize)]
struct LibraryArtifact {
    path: String,
    #[serde(flatten)]
    download: FileDownload,
}

#[derive(Deserialize)]
struct Rule {
    action: Action,
    os: Option<OsRule>,
    #[serde(default)]
    features: BTreeMap<String, bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Action {
    Allow,
    Disallow,
}

#[derive(Deserialize)]
struct OsRule {
    name: Option<String>,
    arch: Option<String>,
    version: Option<String>,
}

fn rules_allow(rules: &[Rule], os: &str, arch: &str) -> Result<bool> {
    let mut allowed = rules.is_empty();
    for rule in rules {
        if let Some(platform) = &rule.os {
            if platform.name.as_deref().is_some_and(|name| name != os)
                || platform.arch.as_deref().is_some_and(|rule| {
                    !matches!((rule, arch), ("arm64", "aarch64") | ("x64", "x86_64"))
                        && rule != arch
                })
            {
                continue;
            }
            if platform.version.is_some() {
                return Err("Mojang library uses an unsupported OS version rule".into());
            }
        }
        if rule.features.values().any(|&enabled| enabled) {
            continue;
        }
        allowed = matches!(rule.action, Action::Allow);
    }
    Ok(allowed)
}

fn native_for_platform(coordinate: &str, os: &str, arch: &str) -> bool {
    match coordinate.split(':').nth(3) {
        Some("natives-macos-arm64" | "osx-aarch_64") => os == "osx" && arch == "aarch64",
        Some("natives-macos" | "osx-x86_64") => os == "osx" && arch == "x86_64",
        Some("natives-windows") => os == "windows" && arch == "x86_64",
        Some("natives-windows-arm64") => os == "windows" && arch == "aarch64",
        Some("natives-windows-x86") => os == "windows" && arch == "x86",
        _ => true,
    }
}

fn plan(path: PathBuf, file: FileDownload) -> Result<PlannedArtifact> {
    let download = Download {
        url: file.url,
        size: Some(file.size),
        hash: Some(Hash::Sha1(file.sha1)),
    };
    artifacts::validate_metadata(&download)?;
    Ok(PlannedArtifact {
        download,
        relative_path: path,
    })
}

fn assets_plan(index: Assets) -> Result<Vec<PlannedArtifact>> {
    let mut seen = HashSet::new();
    let mut files = Vec::new();
    for asset in index.objects.into_values() {
        if !artifacts::valid_hex(&asset.hash, 40) {
            return Err("Invalid Mojang asset hash".into());
        }
        if !seen.insert(asset.hash.clone()) {
            continue;
        }
        let bucket = &asset.hash[..2];
        let url = Url::parse(&format!(
            "https://resources.download.minecraft.net/{bucket}/{}",
            asset.hash
        ))?;
        files.push(plan(
            PathBuf::from("common/assets/objects")
                .join(bucket)
                .join(&asset.hash),
            FileDownload {
                size: asset.size,
                sha1: asset.hash,
                url,
            },
        )?);
    }
    Ok(files)
}

pub fn files(client: &Client, root: &Path, version_id: &str) -> Result<Vec<PlannedArtifact>> {
    if !artifacts::safe_segment(version_id) {
        return Err("Invalid Minecraft version".into());
    }
    let mut bytes = Vec::new();
    client
        .get(INDEX)
        .send()?
        .error_for_status()?
        .take(METADATA_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > METADATA_LIMIT {
        return Err("Mojang version index exceeded its size limit".into());
    }
    let index: VersionIndex = serde_json::from_slice(&bytes)?;
    let entry = index
        .versions
        .into_iter()
        .find(|entry| entry.id == version_id)
        .ok_or("Minecraft version is absent from Mojang's index")?;
    let version_base = PathBuf::from("common/versions").join(version_id);
    let version_json = PlannedArtifact {
        download: Download {
            url: entry.url,
            size: None,
            hash: Some(Hash::Sha1(entry.sha1)),
        },
        relative_path: version_base.join(format!("{version_id}.json")),
    };
    artifacts::validate_metadata(&version_json.download)?;
    artifacts::ensure(client, root, &version_json)?;
    let version: Version =
        serde_json::from_reader(fs::File::open(root.join(&version_json.relative_path))?)?;
    if version.id != version_id || !artifacts::safe_segment(&version.asset_index.id) {
        return Err("Unexpected Mojang version or asset index".into());
    }

    let asset_index = plan(
        PathBuf::from("common/assets/indexes").join(format!("{}.json", version.asset_index.id)),
        version.asset_index.download,
    )?;
    artifacts::ensure(client, root, &asset_index)?;
    let assets = serde_json::from_reader(fs::File::open(root.join(&asset_index.relative_path))?)?;
    let mut files = assets_plan(assets)?;
    files.push(plan(
        version_base.join(format!("{version_id}.jar")),
        version.downloads.client,
    )?);

    let os = match env::consts::OS {
        "macos" => "osx",
        other => other,
    };
    for library in version.libraries {
        if !rules_allow(&library.rules, os, env::consts::ARCH)?
            || !native_for_platform(&library.name, os, env::consts::ARCH)
        {
            continue;
        }
        let artifact = library.downloads.artifact;
        files.push(plan(
            PathBuf::from("common/libraries").join(artifacts::relative_path(&artifact.path)?),
            artifact.download,
        )?);
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_native_libraries_follow_the_machine_architecture() {
        assert!(native_for_platform(
            "org.lwjgl:lwjgl:3:natives-macos",
            "osx",
            "x86_64"
        ));
        assert!(!native_for_platform(
            "org.lwjgl:lwjgl:3:natives-macos",
            "osx",
            "aarch64"
        ));
        assert!(native_for_platform(
            "org.lwjgl:lwjgl:3:natives-macos-arm64",
            "osx",
            "aarch64"
        ));
        assert!(!native_for_platform(
            "io.netty:netty-transport-native-kqueue:4:osx-x86_64",
            "osx",
            "aarch64"
        ));
        assert!(native_for_platform(
            "io.netty:netty-transport-native-kqueue:4:osx-aarch_64",
            "osx",
            "aarch64"
        ));
    }

    #[test]
    fn uses_last_matching_mojang_library_rule() {
        let rules: Vec<Rule> = serde_json::from_str(
            r#"[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}}]"#,
        )
        .unwrap();
        assert!(rules_allow(&rules, "windows", "x86_64").unwrap());
        assert!(!rules_allow(&rules, "osx", "aarch64").unwrap());
    }
}
