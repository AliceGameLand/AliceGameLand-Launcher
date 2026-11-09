use std::{
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    thread,
    time::Duration,
};

use md5::{digest::DynDigest, Md5};
use reqwest::{blocking::Client, redirect::Policy, Url};
use sha1::Sha1;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const UNSIZED_LIMIT: u64 = 8 * 1024 * 1024;

pub enum Hash {
    Md5(String),
    Sha1(String),
}

pub struct Download {
    pub url: Url,
    pub size: Option<u64>,
    pub hash: Option<Hash>,
}

pub struct PlannedArtifact {
    pub download: Download,
    pub relative_path: PathBuf,
}

fn trusted_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
                    | "maven.neoforged.net"
                    | "libraries.minecraft.net"
                    | "maven.fabricmc.net"
                    | "cdn.modrinth.com"
                    | "mediafilez.forgecdn.net"
                    | "piston-meta.mojang.com"
                    | "piston-data.mojang.com"
                    | "resources.download.minecraft.net"
            )
        )
}

pub fn client() -> Result<Client> {
    Ok(Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(120))
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !trusted_url(attempt.url()) {
                attempt.error("Unexpected download redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()?)
}

pub fn safe_segment(value: &str) -> bool {
    !value.is_empty()
        && !value.ends_with([' ', '.'])
        && !value
            .chars()
            .any(|ch| ch.is_control() || "/\\:<>|\"?*".contains(ch))
        && !matches!(
            value
                .split('.')
                .next()
                .unwrap_or_default()
                .to_ascii_uppercase()
                .as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        )
}

pub fn relative_path(path: &str) -> Result<PathBuf> {
    if !path.split('/').all(safe_segment) {
        return Err("Invalid relative game file path".into());
    }
    Ok(PathBuf::from(path))
}

pub fn valid_hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn validate_metadata(download: &Download) -> Result<()> {
    let hash_ok = match &download.hash {
        Some(Hash::Md5(hash)) => valid_hex(hash, 32),
        Some(Hash::Sha1(hash)) => valid_hex(hash, 40),
        None => download.size.is_some(),
    };
    if !trusted_url(&download.url)
        || download.size.is_some_and(|size| size > 1024 * 1024 * 1024)
        || !hash_ok
    {
        return Err("Invalid game file metadata".into());
    }
    Ok(())
}

fn canonical_target(root: &Path, relative: &Path) -> Result<PathBuf> {
    let mut parent = root.to_path_buf();
    for component in relative
        .parent()
        .ok_or("Missing game file parent")?
        .components()
    {
        let Component::Normal(name) = component else {
            return Err("Invalid game file directory".into());
        };
        let directory = parent.join(name);
        match fs::create_dir(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        parent = directory.canonicalize()?;
        if !parent.starts_with(root) || !parent.is_dir() {
            return Err("Game file directory is outside the data folder".into());
        }
    }
    Ok(parent.join(relative.file_name().ok_or("Missing game file name")?))
}

fn existing(root: &Path, target: &Path, download: &Download) -> Result<bool> {
    let metadata = match fs::metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let actual = target.canonicalize()?;
    if !metadata.is_file() || !actual.starts_with(root) {
        return Err("Game file points outside the data folder or is not a file".into());
    }
    let Some(expected) = &download.hash else {
        // Defaults without a digest are player-owned once present.
        return Ok(true);
    };
    let size_matches = download
        .size
        .map_or(metadata.len() <= UNSIZED_LIMIT, |size| {
            metadata.len() == size
        });
    Ok(size_matches && matches_hash(fs::File::open(actual)?, expected)?)
}

fn matches_hash(mut file: impl Read, hash: &Hash) -> Result<bool> {
    let (mut digest, expected): (Box<dyn DynDigest>, &str) = match hash {
        Hash::Md5(value) => (Box::<Md5>::default(), value),
        Hash::Sha1(value) => (Box::<Sha1>::default(), value),
    };
    let mut buffer = [0; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    let actual = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(actual.eq_ignore_ascii_case(expected))
}

fn direct_url(original: &Url) -> Url {
    let mut url = original.clone();
    if url.host_str() == Some("github.com") {
        for repo in ["AliceGameLand-Distribution", "AliceGameLand-Launcher"] {
            let old = format!("/Dot0420/{repo}/");
            if let Some(rest) = url.path().strip_prefix(&old) {
                let path = format!("/AliceGameLand/{repo}/{rest}");
                url.set_path(&path);
                break;
            }
        }
    }
    url
}

pub fn ensure(client: &Client, root: &Path, file: &PlannedArtifact) -> Result<bool> {
    let download = &file.download;
    let target = canonical_target(root, &file.relative_path)?;
    if existing(root, &target, download)? {
        return Ok(false);
    }

    let url = direct_url(&download.url);
    let limit = download.size.unwrap_or(UNSIZED_LIMIT);
    let mut response = client
        .get(url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(reqwest::Error::without_url)?
        .take(limit + 1);
    let mut stage =
        tempfile::NamedTempFile::new_in(target.parent().ok_or("Missing game file directory")?)?;
    let received = io::copy(&mut response, &mut stage)?;
    if received > limit || download.size.is_some_and(|size| received != size) {
        return Err("Downloaded game file has an unexpected size".into());
    }
    stage.as_file().sync_all()?;
    if let Some(hash) = &download.hash {
        if !matches_hash(stage.reopen()?, hash)? {
            return Err("Downloaded game file failed integrity verification".into());
        }
        stage.persist(&target)?;
    } else {
        // Another launch may have created player-owned defaults during the download.
        match stage.persist_noclobber(&target) {
            Ok(_) => {}
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
                if existing(root, &target, download)? {
                    return Ok(false);
                }
                return Err(error.into());
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(true)
}

pub fn ensure_all(client: &Client, root: &Path, files: &[PlannedArtifact]) -> Result<usize> {
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    thread::scope(|scope| {
        let mut workers = Vec::new();
        for _ in 0..files.len().min(6) {
            workers.push(scope.spawn(|| -> Result<usize> {
                let mut downloaded = 0;
                while !stop.load(Ordering::Relaxed) {
                    let Some(file) = files.get(next.fetch_add(1, Ordering::Relaxed)) else {
                        break;
                    };
                    match ensure(client, root, file) {
                        Ok(true) => downloaded += 1,
                        Ok(false) => {}
                        Err(error) => {
                            stop.store(true, Ordering::Relaxed);
                            return Err(format!("{}: {error}", file.relative_path.display()).into());
                        }
                    }
                }
                Ok(downloaded)
            }));
        }
        let mut total = 0;
        let mut failure = None;
        for worker in workers {
            match worker
                .join()
                .unwrap_or_else(|_| Err("Download worker failed".into()))
            {
                Ok(count) => total += count,
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(total),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use md5::Digest;

    #[test]
    fn preserves_edited_defaults_but_checks_hashed_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let file = root.join("options.txt");
        fs::write(&file, b"edited").unwrap();
        let mut download = Download {
            url: Url::parse("https://github.com/example").unwrap(),
            size: Some(7),
            hash: None,
        };
        assert!(existing(&root, &file, &download).unwrap());
        download.hash = Some(Hash::Md5("00000000000000000000000000000000".into()));
        assert!(!existing(&root, &file, &download).unwrap());
        download.size = Some(6);
        download.hash = Some(Hash::Md5(format!("{:x}", Md5::digest(b"edited"))));
        assert!(existing(&root, &file, &download).unwrap());
        download.size = None;
        download.hash = Some(Hash::Sha1(format!("{:x}", Sha1::digest(b"edited"))));
        assert!(existing(&root, &file, &download).unwrap());
    }

    #[test]
    fn rewrites_only_known_legacy_repository_paths() {
        let old = Url::parse("https://github.com/Dot0420/AliceGameLand-Distribution/releases/download/tag/a%2Bb?v=12").unwrap();
        let new = direct_url(&old);
        assert_eq!(new.as_str(), "https://github.com/AliceGameLand/AliceGameLand-Distribution/releases/download/tag/a%2Bb?v=12");
    }
}
