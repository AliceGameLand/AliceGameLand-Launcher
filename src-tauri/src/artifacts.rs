use std::{
    collections::HashSet,
    fs,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    thread,
    time::Duration,
};

use md5::{digest::DynDigest, Md5};
use reqwest::{blocking::Client, redirect::Policy, Url};
use sha1::Sha1;

mod progress;
use crate::validation_cache::ValidationCache;
pub use progress::{LaunchProgress, ProgressCounter};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const UNSIZED_LIMIT: u64 = 8 * 1024 * 1024;

pub enum Hash {
    Md5(String),
    Sha1(String),
}

impl Hash {
    fn expected(&self) -> &str {
        match self {
            Self::Md5(value) | Self::Sha1(value) => value,
        }
    }

    fn digest(&self) -> Box<dyn DynDigest> {
        match self {
            Self::Md5(_) => Box::<Md5>::default(),
            Self::Sha1(_) => Box::<Sha1>::default(),
        }
    }
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

fn existing(
    root: &Path,
    target: &Path,
    download: &Download,
    cache: &ValidationCache,
) -> Result<bool> {
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
    if !size_matches {
        return Ok(false);
    }
    cache.verify(
        &actual,
        &mut fs::File::open(&actual)?,
        expected.expected(),
        |file| matches_hash(file, expected),
    )
}

fn matches_hash(mut file: impl Read, hash: &Hash) -> Result<bool> {
    let mut digest = hash.digest();
    let mut buffer = [0; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(digest_matches(digest, hash))
}

fn digest_matches(digest: Box<dyn DynDigest>, hash: &Hash) -> bool {
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
        .eq_ignore_ascii_case(hash.expected())
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

pub fn ensure_metadata(
    client: &Client,
    root: &Path,
    file: &PlannedArtifact,
    cache: &ValidationCache,
) -> Result<()> {
    let target = canonical_target(root, &file.relative_path)?;
    if !existing(root, &target, &file.download, cache)? {
        download_file(client, root, file, cache, |_| {})?;
    }
    Ok(())
}

fn download_file(
    client: &Client,
    root: &Path,
    file: &PlannedArtifact,
    cache: &ValidationCache,
    advance: impl Fn(u64),
) -> Result<()> {
    let download = &file.download;
    let target = canonical_target(root, &file.relative_path)?;

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
    let mut received = 0;
    let mut digest = download.hash.as_ref().map(Hash::digest);
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = response.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        received += count as u64;
        if received > limit {
            return Err("Downloaded game file exceeds its size limit".into());
        }
        stage.write_all(&buffer[..count])?;
        if let Some(digest) = &mut digest {
            digest.update(&buffer[..count]);
        }
        advance(count as u64);
    }
    if download.size.is_some_and(|size| received != size) {
        return Err("Downloaded game file has an unexpected size".into());
    }
    stage.as_file().sync_all()?;
    if let (Some(hash), Some(digest)) = (&download.hash, digest) {
        if !digest_matches(digest, hash) {
            return Err("Downloaded game file failed integrity verification".into());
        }
        // Close the writer before recording the published file's final NTFS timestamps.
        drop(stage.persist(&target)?);
        if !existing(root, &target, download, cache)? {
            return Err("Installed game file failed integrity verification".into());
        }
    } else {
        // Another launch may have created player-owned defaults during the download.
        match stage.persist_noclobber(&target) {
            Ok(_) => {}
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
                if existing(root, &target, download, cache)? {
                    return Ok(());
                }
                return Err(error.into());
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

pub fn ensure_all(
    client: &Client,
    root: &Path,
    files: &[PlannedArtifact],
    cache: &ValidationCache,
    report: impl Fn(LaunchProgress) + Sync,
) -> Result<()> {
    // Mojang entries follow the distribution and own the final contents of shared library paths.
    let mut paths = HashSet::new();
    let files: Vec<_> = files
        .iter()
        .rev()
        .filter(|file| paths.insert(&file.relative_path))
        .collect();
    let checking = ProgressCounter::new(|current| {
        report(LaunchProgress::Checking {
            current,
            total: files.len() as u64,
        })
    });
    let missing = parallel_files(&files, |file| {
        let target = canonical_target(root, &file.relative_path)?;
        let present = existing(root, &target, &file.download, cache)?;
        checking.advance(1);
        Ok(!present)
    })?;
    checking.finish();
    if missing.is_empty() {
        return Ok(());
    }
    let total = missing.iter().map(|file| file.download.size).sum();
    let downloading = ProgressCounter::new(|current| {
        report(LaunchProgress::Downloading {
            message: "게임 파일 다운로드 중",
            current,
            total,
        })
    });
    parallel_files(&missing, |file| {
        download_file(client, root, file, cache, |bytes| {
            downloading.advance(bytes)
        })?;
        Ok(false)
    })?;
    downloading.finish();
    Ok(())
}

fn parallel_files<'a>(
    files: &[&'a PlannedArtifact],
    visit: impl Fn(&PlannedArtifact) -> Result<bool> + Sync,
) -> Result<Vec<&'a PlannedArtifact>> {
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    thread::scope(|scope| {
        let mut workers = Vec::new();
        for _ in 0..files.len().min(6) {
            workers.push(scope.spawn(|| -> Result<Vec<&PlannedArtifact>> {
                let mut selected = Vec::new();
                while !stop.load(Ordering::Relaxed) {
                    let Some(&file) = files.get(next.fetch_add(1, Ordering::Relaxed)) else {
                        break;
                    };
                    match visit(file) {
                        Ok(true) => selected.push(file),
                        Ok(false) => {}
                        Err(error) => {
                            stop.store(true, Ordering::Relaxed);
                            return Err(format!("{}: {error}", file.relative_path.display()).into());
                        }
                    }
                }
                Ok(selected)
            }));
        }
        let mut selected = Vec::new();
        let mut failure = None;
        for worker in workers {
            match worker
                .join()
                .unwrap_or_else(|_| Err("Download worker failed".into()))
            {
                Ok(mut batch) => selected.append(&mut batch),
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(selected),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use md5::Digest;
    use std::{
        io::{BufRead, BufReader},
        net::TcpListener,
        sync::Mutex,
        time::Instant,
    };

    #[test]
    fn preserves_edited_defaults_but_checks_hashed_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let cache = ValidationCache::load(&root.join("cache.json"));
        let file = root.join("options.txt");
        fs::write(&file, b"edited").unwrap();
        let mut download = Download {
            url: Url::parse("https://github.com/example").unwrap(),
            size: Some(7),
            hash: None,
        };
        assert!(existing(&root, &file, &download, &cache).unwrap());
        download.hash = Some(Hash::Md5("00000000000000000000000000000000".into()));
        assert!(!existing(&root, &file, &download, &cache).unwrap());
        download.size = Some(6);
        download.hash = Some(Hash::Md5(format!("{:x}", Md5::digest(b"edited"))));
        assert!(existing(&root, &file, &download, &cache).unwrap());
        download.size = None;
        download.hash = Some(Hash::Sha1(format!("{:x}", Sha1::digest(b"edited"))));
        assert!(existing(&root, &file, &download, &cache).unwrap());
    }

    fn serve(payload: Vec<u8>) -> (Url, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = Url::parse(&format!("http://{}/file", listener.local_addr().unwrap())).unwrap();
        let worker = thread::spawn(move || {
            let start = Instant::now();
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        assert!(
                            start.elapsed() < Duration::from_secs(5),
                            "No download request"
                        );
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut headers = BufReader::new(&stream).lines();
            assert!(headers.any(|line| line.unwrap().is_empty()));
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                payload.len()
            )
            .unwrap();
            stream.write_all(&payload).unwrap();
        });
        (url, worker)
    }

    #[test]
    fn reports_only_missing_bytes_and_recovers_after_failed_transfer() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        let cache_path = root.join("cache.json");
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()?;
        let payload = vec![0x37; 192 * 1024];
        let size = payload.len() as u64;
        let (url, first) = serve(payload.clone());
        let mut plan = vec![
            PlannedArtifact {
                relative_path: "library.jar".into(),
                download: Download {
                    url: url.clone(),
                    size: Some(size),
                    hash: Some(Hash::Sha1(format!("{:x}", Sha1::digest(&payload)))),
                },
            },
            PlannedArtifact {
                relative_path: "options.txt".into(),
                download: Download {
                    url,
                    size: Some(100),
                    hash: None,
                },
            },
        ];
        fs::write(root.join("options.txt"), b"edited")?;
        let run = |plan: &[PlannedArtifact]| -> Result<Vec<LaunchProgress>> {
            let cache = ValidationCache::load(&cache_path);
            let events = Mutex::new(Vec::new());
            ensure_all(&client, &root, plan, &cache, |event| {
                events.lock().unwrap().push(event)
            })?;
            cache.flush()?;
            Ok(events.into_inner().unwrap())
        };
        let initial = run(&plan)?;
        first.join().unwrap();
        assert!(matches!(
            initial.first(),
            Some(LaunchProgress::Checking {
                current: 0,
                total: 2
            })
        ));
        assert!(initial.iter().any(|event| matches!(
            event,
            LaunchProgress::Checking {
                current: 2,
                total: 2
            }
        )));
        assert!(
            matches!(initial.last(), Some(LaunchProgress::Downloading { current, total: Some(total), .. }) if *current == size && *total == size)
        );
        assert_eq!(fs::read(root.join("options.txt"))?, b"edited");

        // The fixture server is now closed. A warm launch must complete without a single request.
        let warm = run(&plan)?;
        assert!(warm
            .iter()
            .all(|event| matches!(event, LaunchProgress::Checking { .. })));
        fs::write(root.join("library.jar"), vec![0x00; payload.len()])?;
        let (bad_url, bad) = serve(vec![0x42; payload.len()]);
        plan[0].download.url = bad_url;
        assert!(run(&plan).is_err());
        bad.join().unwrap();
        assert_eq!(
            fs::read(root.join("library.jar"))?,
            vec![0x00; payload.len()]
        );

        let (good_url, retry) = serve(payload.clone());
        plan[0].download.url = good_url;
        plan[0].download.size = None;
        let repaired = run(&plan)?;
        retry.join().unwrap();
        let downloads: Vec<_> = repaired
            .iter()
            .filter_map(|event| match event {
                LaunchProgress::Downloading {
                    current,
                    total: None,
                    ..
                } => Some(*current),
                _ => None,
            })
            .collect();
        assert_eq!(downloads.first(), Some(&0));
        assert_eq!(downloads.last(), Some(&size));
        assert!(downloads.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(fs::read(root.join("library.jar"))?, payload);
        Ok(())
    }

    #[test]
    fn rewrites_only_known_legacy_repository_paths() {
        let old = Url::parse("https://github.com/Dot0420/AliceGameLand-Distribution/releases/download/tag/a%2Bb?v=12").unwrap();
        let new = direct_url(&old);
        assert_eq!(new.as_str(), "https://github.com/AliceGameLand/AliceGameLand-Distribution/releases/download/tag/a%2Bb?v=12");
    }
}
