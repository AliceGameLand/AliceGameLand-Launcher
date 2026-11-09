use std::{
    error::Error,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
    time::Duration,
};

use sha2::{Digest, Sha256};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
static PREPARATION: Mutex<()> = Mutex::new(());
const VERSION: &str = "25.0.5+8";
const DOWNLOAD_BASE: &str =
    "https://github.com/adoptium/temurin25-binaries/releases/download/jdk-25.0.5%2B8/";

struct Runtime {
    archive: &'static str,
    bytes: u64,
    sha256: &'static str,
    console: &'static str,
    executable: &'static str,
}

fn runtime_for(os: &str, arch: &str) -> Result<Runtime> {
    let (archive, bytes, sha256) = match (os, arch) {
        ("windows", "x86_64") => (
            "OpenJDK25U-jdk_x64_windows_hotspot_25.0.5_8.zip",
            141_285_173,
            "636bdc13dc649ddc7d5921c5c7d23cca6e44fd19d662b45cc9c02135a6c96c42",
        ),
        ("macos", "aarch64") => (
            "OpenJDK25U-jdk_aarch64_mac_hotspot_25.0.5_8.tar.gz",
            136_446_056,
            "36dc14d797aaff6d89aab302cd053fdba6207dfdcf772052fa7b023a94b0e857",
        ),
        _ => return Err(format!("Unsupported Java platform: {os}/{arch}").into()),
    };
    let (console, executable) = if os == "windows" {
        ("bin/java.exe", "bin/javaw.exe")
    } else {
        ("Contents/Home/bin/java", "Contents/Home/bin/java")
    };
    Ok(Runtime {
        archive,
        bytes,
        sha256,
        console,
        executable,
    })
}

fn cache_destination(base: &Path) -> PathBuf {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    base.join(format!("temurin-{VERSION}-{os}-{arch}"))
}

pub fn cached_executable(base: &Path) -> Option<PathBuf> {
    let runtime = runtime_for(std::env::consts::OS, std::env::consts::ARCH).ok()?;
    let path = cache_destination(base).join(runtime.executable);
    path.is_file().then_some(path)
}

fn download(runtime: &Runtime, destination: &Path) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .build()?;
    let mut response = client
        .get(format!("{DOWNLOAD_BASE}{}", runtime.archive))
        .send()?
        .error_for_status()?
        .take(runtime.bytes + 1);
    let mut file = File::create(destination)?;
    let mut hash = Sha256::new();
    let mut bytes = 0;
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let count = response.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let chunk = &buffer[..count];
        file.write_all(chunk)?;
        hash.update(chunk);
        bytes += count as u64;
    }
    if bytes != runtime.bytes || format!("{:x}", hash.finalize()) != runtime.sha256 {
        return Err("Java archive size or SHA-256 did not match".into());
    }
    file.sync_all()?;
    Ok(())
}

pub fn prepare(base: &Path) -> Result<PathBuf> {
    let _guard = PREPARATION
        .lock()
        .map_err(|_| "Java preparation lock failed")?;
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let runtime = runtime_for(os, arch)?;
    let destination = cache_destination(base);
    let executable = destination.join(runtime.executable);
    if executable.is_file() {
        return Ok(executable);
    }

    fs::create_dir_all(base)?;
    let work = tempfile::tempdir_in(base)?;
    let archive = work.path().join(runtime.archive);
    download(&runtime, &archive)?;
    let extracted = work.path().join("extracted");
    fs::create_dir(&extracted)?;
    if runtime.archive.ends_with(".zip") {
        zip::ZipArchive::new(File::open(archive)?)?.extract(&extracted)?;
    } else {
        let gzip = flate2::read::GzDecoder::new(File::open(archive)?);
        tar::Archive::new(gzip).unpack(&extracted)?;
    }

    let mut directories = fs::read_dir(&extracted)?
        .collect::<io::Result<Vec<_>>>()?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.join(runtime.executable).is_file());
    let root = directories.next().ok_or("Java executable is missing")?;
    if directories.next().is_some() {
        return Err("Java archive contains more than one runtime".into());
    }
    let mut command = Command::new(root.join(runtime.console));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let version = command.arg("-version").output()?;
    if !version.status.success()
        || !String::from_utf8_lossy(&version.stderr).contains("version \"25.")
    {
        return Err("The extracted Java runtime could not be validated".into());
    }

    // Publish the complete runtime atomically so a retry cannot use a partial download.
    if let Err(error) = fs::rename(&root, &destination) {
        if !executable.is_file() {
            return Err(error.into());
        }
    }
    Ok(executable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_arch_specific_archives_and_native_java_paths() {
        let windows = runtime_for("windows", "x86_64").unwrap();
        let silicon = runtime_for("macos", "aarch64").unwrap();
        assert!(windows.archive.ends_with(".zip"));
        assert_eq!(windows.executable, "bin/javaw.exe");
        assert!(silicon.archive.contains("_aarch64_mac_"));
        assert_eq!(silicon.executable, "Contents/Home/bin/java");
        assert!(runtime_for("macos", "x86_64").is_err());
        assert!(runtime_for("linux", "x86_64").is_err());
    }

    #[test]
    #[ignore = "downloads the pinned Java archive"]
    fn downloads_and_reuses_java_25() -> Result<()> {
        let cache = tempfile::tempdir()?;
        assert!(cached_executable(cache.path()).is_none());
        let executable = prepare(cache.path())?;
        assert!(executable.is_file());
        assert_eq!(cached_executable(cache.path()).as_ref(), Some(&executable));
        let marker = cache_destination(cache.path()).join("cache-check");
        fs::write(&marker, b"first preparation")?;
        assert_eq!(prepare(cache.path())?, executable);
        assert_eq!(fs::read_to_string(marker)?, "first preparation");
        Ok(())
    }
}
