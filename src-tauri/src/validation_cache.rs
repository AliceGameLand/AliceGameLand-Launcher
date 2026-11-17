use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::artifacts::Result;

const MAX_AGE_SECONDS: u64 = 24 * 60 * 60;
const MAX_CACHE_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
struct FileStamp {
    device: u64,
    id: u128,
    size: u64,
    modified: i128,
    changed: i128,
}

#[cfg(unix)]
fn file_stamp(file: &File) -> Option<FileStamp> {
    use std::os::unix::fs::MetadataExt;

    let meta = file.metadata().ok()?;
    let nanos = |secs: i64, nsec: i64| i128::from(secs) * 1_000_000_000 + i128::from(nsec);
    Some(FileStamp {
        device: meta.dev(),
        id: meta.ino().into(),
        size: meta.len(),
        modified: nanos(meta.mtime(), meta.mtime_nsec()),
        changed: nanos(meta.ctime(), meta.ctime_nsec()),
    })
}

#[cfg(windows)]
fn file_stamp(file: &File) -> Option<FileStamp> {
    use std::{mem::size_of, os::windows::io::AsRawHandle, ptr::from_mut};
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{
            FileBasicInfo, FileIdInfo, GetFileInformationByHandleEx, FILE_BASIC_INFO, FILE_ID_INFO,
        },
    };

    let handle = HANDLE(file.as_raw_handle());
    let mut identity = FILE_ID_INFO::default();
    let mut times = FILE_BASIC_INFO::default();
    // Query the open file, so a path replacement cannot pair its identity with another file's hash.
    unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileIdInfo,
            from_mut(&mut identity).cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )
        .ok()?;
        GetFileInformationByHandleEx(
            handle,
            FileBasicInfo,
            from_mut(&mut times).cast(),
            size_of::<FILE_BASIC_INFO>() as u32,
        )
        .ok()?;
    }
    let id = u128::from_le_bytes(identity.FileId.Identifier);
    if id == 0 || times.ChangeTime == 0 {
        return None;
    }
    Some(FileStamp {
        device: identity.VolumeSerialNumber,
        id,
        size: file.metadata().ok()?.len(),
        modified: times.LastWriteTime.into(),
        changed: times.ChangeTime.into(),
    })
}

#[derive(Clone, Deserialize, Serialize)]
struct Entry {
    stamp: FileStamp,
    expected: String,
    checked_at: u64,
}

impl Entry {
    fn fresh(&self, now: u64) -> bool {
        now.checked_sub(self.checked_at)
            .is_some_and(|age| age < MAX_AGE_SECONDS)
    }
}

fn now_seconds() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|time| time.as_secs())
}

type Entries = HashMap<PathBuf, Entry>;

fn read_entries(path: &Path) -> Option<Entries> {
    let file = File::open(path).ok()?;
    if file.metadata().ok()?.len() > MAX_CACHE_BYTES {
        return None;
    }
    let mut entries: Entries =
        serde_json::from_reader(BufReader::new(file.take(MAX_CACHE_BYTES))).ok()?;
    let now = now_seconds()?;
    entries.retain(|_, entry| entry.fresh(now));
    Some(entries)
}

struct State {
    entries: Entries,
    updated: HashSet<PathBuf>,
}

pub struct ValidationCache {
    path: PathBuf,
    state: Mutex<State>,
}

impl ValidationCache {
    pub fn load(path: &Path) -> Self {
        Self {
            path: path.to_owned(),
            state: Mutex::new(State {
                entries: read_entries(path).unwrap_or_default(),
                updated: HashSet::new(),
            }),
        }
    }

    /// Only an unchanged file that previously passed this digest can skip a full read.
    pub fn verify(
        &self,
        path: &Path,
        file: &mut File,
        expected: &str,
        check: impl FnOnce(&mut File) -> Result<bool>,
    ) -> Result<bool> {
        let (Some(before), Some(now)) = (file_stamp(file), now_seconds()) else {
            return check(file);
        };
        if let Ok(state) = self.state.lock() {
            if state.entries.get(path).is_some_and(|entry| {
                entry.fresh(now)
                    && entry.stamp == before
                    && entry.expected.eq_ignore_ascii_case(expected)
            }) {
                return Ok(true);
            }
        }

        if !check(file)? {
            return Ok(false);
        }
        match file_stamp(file) {
            Some(after) if after != before => return Ok(false),
            None => return Ok(true),
            _ => {}
        }
        if path.to_str().is_some() {
            if let Ok(mut state) = self.state.lock() {
                state.entries.insert(
                    path.to_owned(),
                    Entry {
                        stamp: before,
                        expected: expected.to_owned(),
                        checked_at: now,
                    },
                );
                state.updated.insert(path.to_owned());
            }
        }
        Ok(true)
    }

    /// Other launchers may have verified a different server since this snapshot was opened.
    pub fn flush(&self) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("Validation cache lock failed"))?;
        if state.updated.is_empty() {
            return Ok(());
        }
        let parent = self
            .path
            .parent()
            .ok_or_else(|| io::Error::other("Missing cache directory"))?;
        fs::create_dir_all(parent)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.path.with_extension("lock"))?;
        lock.lock()?;
        let mut entries = read_entries(&self.path).unwrap_or_default();
        for path in &state.updated {
            if let Some(entry) = state.entries.get(path) {
                entries.insert(path.clone(), entry.clone());
            }
        }
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        {
            let mut writer = BufWriter::new(temp.as_file_mut());
            serde_json::to_writer(&mut writer, &entries)?;
            writer.flush()?;
        }
        temp.as_file().sync_all()?;
        temp.persist(&self.path)?;
        state.updated.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha1::{Digest, Sha1};
    use std::io::Read;

    fn digest(bytes: &[u8]) -> String {
        format!("{:x}", Sha1::digest(bytes))
    }

    fn validate(cache: &ValidationCache, path: &Path, expected: &str) -> (bool, bool) {
        let mut read = false;
        let mut file = File::open(path).unwrap();
        let valid = cache
            .verify(path, &mut file, expected, |file| {
                read = true;
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)?;
                Ok(digest(&bytes).eq_ignore_ascii_case(expected))
            })
            .unwrap();
        (valid, read)
    }

    #[test]
    fn reuses_verified_files_across_launches_and_checks_changed_expectations() {
        let temp = tempfile::tempdir().unwrap();
        let index = temp.path().join("cache.json");
        let asset = temp.path().join("asset");
        fs::write(&asset, b"original").unwrap();
        let cache = ValidationCache::load(&index);
        let expected = digest(b"original");
        assert_eq!(validate(&cache, &asset, &expected), (true, true));
        cache.flush().unwrap();
        let reloaded = ValidationCache::load(&index);
        assert_eq!(validate(&reloaded, &asset, &expected), (true, false));
        assert_eq!(
            validate(&reloaded, &asset, &digest(b"other")),
            (false, true)
        );
    }

    #[test]
    fn rejects_same_size_edits_and_replacements_with_restored_modification_time() {
        let temp = tempfile::tempdir().unwrap();
        let cache = ValidationCache::load(&temp.path().join("cache.json"));
        let asset = temp.path().join("asset");
        fs::write(&asset, b"good").unwrap();
        let expected = digest(b"good");
        let modified = fs::metadata(&asset).unwrap().modified().unwrap();
        assert_eq!(validate(&cache, &asset, &expected), (true, true));
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&asset, b"bad!").unwrap();
        OpenOptions::new()
            .write(true)
            .open(&asset)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        assert_eq!(validate(&cache, &asset, &expected), (false, true));
        let mut replacement = tempfile::NamedTempFile::new_in(temp.path()).unwrap();
        replacement.write_all(b"bad?").unwrap();
        replacement.as_file().set_modified(modified).unwrap();
        replacement.persist(&asset).unwrap();
        assert_eq!(validate(&cache, &asset, &expected), (false, true));
    }

    #[test]
    fn expires_after_a_day_or_a_clock_rollback_and_skips_damaged_cache() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("asset");
        let index = temp.path().join("cache.json");
        fs::write(&index, b"corrupt").unwrap();
        fs::write(&path, b"good").unwrap();
        let cache = ValidationCache::load(&index);
        let expected = digest(b"good");
        assert_eq!(validate(&cache, &path, &expected), (true, true));
        let now = now_seconds().unwrap();
        for timestamp in [now - MAX_AGE_SECONDS, now + MAX_AGE_SECONDS] {
            cache
                .state
                .lock()
                .unwrap()
                .entries
                .get_mut(&path)
                .unwrap()
                .checked_at = timestamp;
            assert_eq!(validate(&cache, &path, &expected), (true, true));
        }
        cache.flush().unwrap();
        assert_eq!(
            validate(&ValidationCache::load(&index), &path, &expected),
            (true, false)
        );
    }

    #[test]
    fn merges_files_from_overlapping_launcher_sessions() {
        let temp = tempfile::tempdir().unwrap();
        let index = temp.path().join("cache.json");
        let first = ValidationCache::load(&index);
        let second = ValidationCache::load(&index);
        let asset_a = temp.path().join("first");
        let asset_b = temp.path().join("second");
        let expected = digest(b"good");
        for (cache, asset) in [(&first, &asset_a), (&second, &asset_b)] {
            fs::write(asset, b"good").unwrap();
            assert_eq!(validate(cache, asset, &expected), (true, true));
        }
        std::thread::scope(|scope| {
            scope.spawn(|| first.flush().unwrap());
            scope.spawn(|| second.flush().unwrap());
        });
        let both = ValidationCache::load(&index);
        assert_eq!(validate(&both, &asset_a, &expected), (true, false));
        assert_eq!(validate(&both, &asset_b, &expected), (true, false));
    }
}
