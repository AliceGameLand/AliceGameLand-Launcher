use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::Path,
    time::Duration,
};

use reqwest::blocking::Client;
use serde::Deserialize;

const REMOTE: &str = "https://raw.githubusercontent.com/AliceGameLand/AliceGameLand-Distribution/main/server-visibility.json";
const MAX_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Group {
    All,
    Burcompany,
    Hyeanjyeoseu,
}

impl Group {
    fn allows(self, server_id: &str) -> bool {
        match self {
            Self::All => true,
            Self::Burcompany => matches!(
                server_id,
                "AliceDiamond-26.2" | "AliceVrMinigames-26.2" | "BurcomPvP-26.3"
            ),
            Self::Hyeanjyeoseu => matches!(
                server_id,
                "AliceGameLand-26.2" | "AliceCrimeScene-26.2" | "AliceTailTag-26.2"
            ),
        }
    }
}

#[derive(Deserialize)]
struct Manifest {
    version: u32,
    all: Vec<String>,
    burcompany: Vec<String>,
    hyeanjyeoseu: Vec<String>,
}

#[derive(Clone)]
pub struct Policy(HashMap<String, Group>);

impl Policy {
    pub fn allows(&self, profile_id: &str, server_id: &str) -> bool {
        normalize_uuid(profile_id)
            .and_then(|id| self.0.get(&id))
            .is_some_and(|group| group.allows(server_id))
    }

    pub fn refresh(path: &Path, previous: Option<Self>) -> Option<Self> {
        Self::refresh_from(path, previous, REMOTE)
    }

    fn refresh_from(path: &Path, previous: Option<Self>, url: &str) -> Option<Self> {
        let download = Client::builder()
            .timeout(Duration::from_secs(4))
            .build()
            .and_then(|client| client.get(url).send())
            .and_then(|response| response.error_for_status());
        if let Some((policy, bytes)) = download.ok().and_then(Self::read) {
            // Use valid settings for this session even when the disk cache cannot be written.
            let _ = Self::save(path, &bytes);
            return Some(policy);
        }
        previous.or_else(|| {
            fs::File::open(path)
                .ok()
                .and_then(Self::read)
                .map(|(policy, _)| policy)
        })
    }

    fn read(reader: impl Read) -> Option<(Self, Vec<u8>)> {
        let mut bytes = Vec::new();
        reader.take(MAX_BYTES + 1).read_to_end(&mut bytes).ok()?;
        if bytes.len() as u64 > MAX_BYTES {
            return None;
        }
        let manifest: Manifest = serde_json::from_slice(&bytes).ok()?;
        if manifest.version != 1 {
            return None;
        }
        let mut groups = HashMap::new();
        for (group, ids) in [
            (Group::All, manifest.all),
            (Group::Burcompany, manifest.burcompany),
            (Group::Hyeanjyeoseu, manifest.hyeanjyeoseu),
        ] {
            for id in ids {
                if groups.insert(normalize_uuid(&id)?, group).is_some() {
                    return None;
                }
            }
        }
        Some((Self(groups), bytes))
    }

    fn save(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| std::io::Error::other("Server visibility cache directory is missing"))?;
        fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(bytes)?;
        file.persist(path)?;
        Ok(())
    }
}

fn normalize_uuid(id: &str) -> Option<String> {
    let compact = id.replace('-', "");
    if compact.len() != 32 || !compact.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(compact.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    const ADMIN: &str = "11111111111141118111111111111111";
    const BURCOMPANY: &str = "22222222222242228222222222222222";
    const TEST_HEYAN: &str = "A0BC24D3-C19D-40C3-94A7-4A8C730ADFBC";
    const HEYAN: &str = "33333333333343338333333333333333";

    fn manifest(burcompany: &str, hyeanjyeoseu: &str) -> String {
        format!(
            r#"{{"version":1,"all":["{ADMIN}"],"burcompany":["{burcompany}"],"hyeanjyeoseu":["{hyeanjyeoseu}","{HEYAN}"]}}"#
        )
    }

    #[test]
    fn filters_the_bundled_servers_by_uuid_and_keeps_distribution_order() {
        let policy = Policy::read(manifest(BURCOMPANY, TEST_HEYAN).as_bytes())
            .unwrap()
            .0;
        let profile = tempfile::tempdir().unwrap();
        let distro =
            crate::distribution::Distribution::local(&profile.path().join("distribution.json"))
                .unwrap();
        let visible = |id: &str| {
            distro
                .servers
                .iter()
                .filter(|server| policy.allows(id, &server.id))
                .map(|server| server.id.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(visible(ADMIN).len(), 6);
        assert!(policy.allows(ADMIN, "future-server"));
        assert_eq!(
            visible(BURCOMPANY),
            [
                "AliceDiamond-26.2",
                "AliceVrMinigames-26.2",
                "BurcomPvP-26.3"
            ]
        );
        let heyan_servers = [
            "AliceTailTag-26.2",
            "AliceGameLand-26.2",
            "AliceCrimeScene-26.2",
        ];
        assert_eq!(visible(HEYAN), heyan_servers);
        assert_eq!(visible("a0bc24d3c19d40c394a74a8c730adfbc"), heyan_servers);
        assert!(visible("44444444444444448444444444444444").is_empty());
    }

    fn serve(status: &str, body: &str) -> String {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let url = format!(
            "http://{}/server-visibility.json",
            listener.local_addr().unwrap()
        );
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.read(&mut [0; 1024]).unwrap();
            stream.write_all(response.as_bytes()).unwrap();
        });
        url
    }

    #[test]
    fn replaces_the_cached_list_and_recovers_from_bad_responses() {
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join("server-visibility.json");
        assert!(Policy::refresh_from(&path, None, &serve("429 Too Many Requests", "")).is_none());

        let initial = Policy::refresh_from(
            &path,
            None,
            &serve("200 OK", &manifest(BURCOMPANY, TEST_HEYAN)),
        )
        .unwrap();
        assert!(initial.allows(TEST_HEYAN, "AliceGameLand-26.2"));

        let changed = Policy::refresh_from(
            &path,
            Some(initial),
            &serve("200 OK", &manifest(TEST_HEYAN, BURCOMPANY)),
        )
        .unwrap();
        assert!(changed.allows(TEST_HEYAN, "BurcomPvP-26.3"));
        assert!(!changed.allows(TEST_HEYAN, "AliceGameLand-26.2"));

        let duplicated = manifest(TEST_HEYAN, &TEST_HEYAN.replace('-', "").to_lowercase());
        let cached = Policy::refresh_from(&path, None, &serve("200 OK", &duplicated)).unwrap();
        assert!(cached.allows(TEST_HEYAN, "BurcomPvP-26.3"));
        let cached =
            Policy::refresh_from(&path, None, &serve("429 Too Many Requests", "")).unwrap();
        assert!(cached.allows(TEST_HEYAN, "BurcomPvP-26.3"));
    }
}
