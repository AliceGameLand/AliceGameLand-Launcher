use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};

use super::{Profile, Result, SavedAccount};

const SERVICE: &str = "kr.dreamcraft.alicegameland.launcher";
const LEGACY_USER: &str = "microsoft-tauri";
const LEGACY_SIGNED_OUT: &str = "electron-account-signed-out";

#[derive(Clone, Deserialize, Serialize)]
pub struct Account {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct AccountIndex {
    selected: Option<String>,
    accounts: Vec<Account>,
}

impl AccountIndex {
    fn remember(&mut self, profile: &Profile) {
        let entry = Account {
            id: profile.id.clone(),
            name: profile.name.clone(),
        };
        if let Some(previous) = self.accounts.iter_mut().find(|item| item.id == entry.id) {
            *previous = entry;
        } else {
            self.accounts.push(entry);
        }
        self.selected = Some(profile.id.clone());
    }

    fn has(&self, id: &str) -> bool {
        self.accounts.iter().any(|entry| entry.id == id)
    }
}

pub(super) struct AccountStore {
    profile: PathBuf,
    data_root: PathBuf,
    user: String,
}

pub(super) async fn with_store<T: Send + 'static>(
    app: &AppHandle,
    action: impl FnOnce(&AccountStore) -> Result<T> + Send + 'static,
) -> Result<T> {
    let store = AccountStore::new(app)?;
    tauri::async_runtime::spawn_blocking(move || action(&store))
        .await
        .map_err(|_| "계정 저장소 작업이 중단됐어요.")?
}

impl AccountStore {
    fn new(app: &AppHandle) -> Result<Self> {
        if let Some(profile) =
            crate::test_profile().map_err(|_| "테스트 프로필을 확인하지 못했어요.")?
        {
            let user = format!(
                "test-{:x}",
                Sha256::digest(profile.as_os_str().as_encoded_bytes())
            );
            return Ok(Self {
                data_root: profile.clone(),
                profile,
                user,
            });
        }
        Ok(Self {
            profile: app
                .path()
                .app_data_dir()
                .map_err(|_| "런처 설정 위치를 찾지 못했어요.")?,
            data_root: app
                .path()
                .data_dir()
                .map_err(|_| "기존 계정 위치를 찾지 못했어요.")?,
            user: LEGACY_USER.into(),
        })
    }

    fn credential(&self, id: Option<&str>) -> Result<keyring::Entry> {
        let user = match id {
            Some(id) => format!("{}-{}", self.user, compact_uuid(id)?),
            None => self.user.clone(),
        };
        keyring::Entry::new(SERVICE, &user)
            .map_err(|_| "운영체제의 계정 보관함을 열지 못했어요.".into())
    }

    fn read_credential(&self, id: Option<&str>) -> Result<Option<SavedAccount>> {
        match self.credential(id)?.get_password() {
            Ok(saved) => serde_json::from_str(&saved)
                .map(Some)
                .map_err(|_| "저장된 계정 정보를 읽지 못했어요.".into()),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("운영체제의 계정 보관함을 읽지 못했어요.".into()),
        }
    }

    pub(super) fn write_credential(&self, account: &SavedAccount) -> Result<()> {
        let stored =
            serde_json::to_string(account).map_err(|_| "계정 정보를 저장하지 못했어요.")?;
        self.credential(Some(&account.profile.id))?
            .set_password(&stored)
            .map_err(|_| "운영체제의 계정 보관함에 저장하지 못했어요.".into())
    }

    fn delete_credential(&self, id: Option<&str>) -> Result<()> {
        match self.credential(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("운영체제의 계정 보관함에서 계정을 삭제하지 못했어요.".into()),
        }
    }

    fn write_index(&self, index: &AccountIndex) -> io::Result<()> {
        fs::create_dir_all(&self.profile)?;
        let mut file = tempfile::NamedTempFile::new_in(&self.profile)?;
        serde_json::to_writer(&mut file, index)?;
        file.flush()?;
        file.as_file().sync_all()?;
        file.persist(self.profile.join("accounts.json"))?;
        Ok(())
    }

    fn save_index(&self, index: &AccountIndex) -> Result<()> {
        self.write_index(index)
            .map_err(|_| "계정 목록을 저장하지 못했어요.".into())
    }

    fn index(&self) -> Result<AccountIndex> {
        let path = self.profile.join("accounts.json");
        match File::open(path) {
            Ok(file) => {
                return serde_json::from_reader(file)
                    .map_err(|_| "계정 목록을 읽지 못했어요.".into())
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err("계정 목록을 읽지 못했어요.".into()),
        }

        let mut index = AccountIndex::default();
        let previous = self.read_credential(None)?;
        let from_tauri = previous.is_some();
        let signed_out = self
            .profile
            .join(LEGACY_SIGNED_OUT)
            .try_exists()
            .map_err(|_| "계정 이전 상태를 확인하지 못했어요.")?;
        let account = if from_tauri || signed_out {
            previous
        } else {
            read_legacy_account(&self.data_root)
                .map_err(|_| "기존 Microsoft 계정을 읽지 못했어요.")?
        };
        if let Some(mut account) = account {
            account.profile.id = compact_uuid(&account.profile.id)?;
            self.write_credential(&account)?;
            index.remember(&account.profile);
            self.save_index(&index)?;
            // The old credential is needed until the per-account entry and index are both durable.
            if from_tauri {
                let _ = self.delete_credential(None);
            }
        }
        Ok(index)
    }

    pub(super) fn list(&self) -> Result<Vec<Account>> {
        Ok(self.index()?.accounts)
    }

    pub(super) fn selected(&self) -> Result<Option<SavedAccount>> {
        let index = self.index()?;
        match index.selected {
            Some(id) => self.read_credential(Some(&id)),
            None => Ok(None),
        }
    }

    pub(super) fn save(&self, mut account: SavedAccount) -> Result<Profile> {
        let mut index = self.index()?;
        account.profile.id = compact_uuid(&account.profile.id)?;
        self.write_credential(&account)?;
        index.remember(&account.profile);
        self.save_index(&index)?;
        Ok(account.profile)
    }

    pub(super) fn select(&self, id: &str) -> Result<Profile> {
        let mut index = self.index()?;
        let id = compact_uuid(id)?;
        if !index.has(&id) {
            return Err("저장된 계정을 찾지 못했어요.".into());
        }
        let account = self
            .read_credential(Some(&id))?
            .ok_or("이 계정으로 다시 로그인해 주세요.")?;
        index.selected = Some(id);
        self.save_index(&index)?;
        Ok(account.profile)
    }

    pub(super) fn remove(&self, id: &str) -> Result<Option<Profile>> {
        let id = compact_uuid(id)?;
        let previous = self.index()?;
        if !previous.has(&id) {
            return Err("저장된 계정을 찾지 못했어요.".into());
        }
        let mut index = previous.clone();
        index.accounts.retain(|account| account.id != id);
        index.selected = None;
        // Keep the active account when removing another one. Otherwise select the first usable one.
        let remaining = previous
            .selected
            .as_deref()
            .filter(|selected| *selected != id)
            .into_iter()
            .chain(index.accounts.iter().map(|account| account.id.as_str()));
        let mut active = None;
        for candidate in remaining {
            if let Some(account) = self.read_credential(Some(candidate))? {
                index.selected = Some(candidate.to_owned());
                active = Some(account.profile);
                break;
            }
        }
        self.save_index(&index)?;
        if let Err(error) = self.delete_credential(Some(&id)) {
            self.save_index(&previous)?;
            return Err(error);
        }
        Ok(active)
    }
}

pub(super) fn compact_uuid(id: &str) -> Result<String> {
    let id = id.replace('-', "").to_ascii_lowercase();
    if id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(id)
    } else {
        Err("계정 UUID가 올바르지 않아요.".into())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyAccounts {
    selected_account: Option<String>,
    #[serde(default)]
    authentication_database: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyAccount {
    #[serde(rename = "type")]
    kind: String,
    display_name: String,
    uuid: String,
    microsoft: LegacyMicrosoft,
}

#[derive(Deserialize)]
struct LegacyMicrosoft {
    refresh_token: String,
}

pub(super) fn read_legacy_account(data_root: &Path) -> io::Result<Option<SavedAccount>> {
    for path in crate::preferences::legacy_configs(data_root) {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let source: LegacyAccounts = serde_json::from_reader(file)?;
        let Some(account) = source
            .selected_account
            .and_then(|id| source.authentication_database.get(&id))
            .and_then(|account| LegacyAccount::deserialize(account).ok())
        else {
            return Ok(None);
        };
        let name = account.display_name.trim();
        let Ok(id) = compact_uuid(&account.uuid) else {
            return Ok(None);
        };
        if account.kind != "microsoft"
            || name.is_empty()
            || account.microsoft.refresh_token.trim().is_empty()
        {
            return Ok(None);
        }
        return Ok(Some(SavedAccount {
            profile: Profile {
                id,
                name: name.to_owned(),
                skins: Vec::new(),
            },
            refresh_token: account.microsoft.refresh_token,
        }));
    }
    Ok(None)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn stores_separate_credentials_and_persists_selection_and_sign_out() {
        let root = tempfile::tempdir().unwrap();
        let store = AccountStore {
            data_root: root.path().into(),
            profile: root.path().into(),
            user: format!(
                "test-{:x}",
                Sha256::digest(root.path().as_os_str().as_encoded_bytes())
            ),
        };
        let first = "11111111111141118111111111111111";
        let second = "22222222222242228222222222222222";
        let account = |id: &str, name: &str| SavedAccount {
            profile: Profile {
                id: id.into(),
                name: name.into(),
                skins: Vec::new(),
            },
            refresh_token: format!("fixture-{id}"),
        };
        let fixture = serde_json::to_string(&account(first, "First")).unwrap();
        store
            .credential(None)
            .unwrap()
            .set_password(&fixture)
            .unwrap();
        assert_eq!(store.selected().unwrap().unwrap().profile.id, first);
        assert!(store.read_credential(None).unwrap().is_none());
        store.save(account(second, "Second")).unwrap();
        store.save(account(second, "Updated")).unwrap();

        assert_eq!(store.list().unwrap().len(), 2);
        assert_eq!(store.selected().unwrap().unwrap().profile.name, "Updated");
        assert_eq!(store.select(first).unwrap().name, "First");
        assert_eq!(
            store.selected().unwrap().unwrap().refresh_token,
            format!("fixture-{first}")
        );
        assert_eq!(store.remove(second).unwrap().unwrap().id, first);
        assert!(store.read_credential(Some(second)).unwrap().is_none());
        assert!(store.remove(first).unwrap().is_none());
        assert!(store.selected().unwrap().is_none());
        assert!(store.list().unwrap().is_empty());
    }
}
