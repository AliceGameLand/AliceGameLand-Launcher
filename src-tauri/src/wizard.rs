use std::{
    env,
    ffi::OsStr,
    fs, io,
    os::windows::{ffi::OsStrExt, process::CommandExt},
    path::{Component, Path, PathBuf, Prefix},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{ipc::Channel, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use windows::{
    core::{w, HRESULT, PCWSTR},
    Win32::{
        Foundation::{CloseHandle, ERROR_CANCELLED, ERROR_SUCCESS, HWND, WAIT_OBJECT_0},
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
            },
            Registry::{
                RegGetValueW, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ,
                RRF_SUBKEY_WOW6464KEY,
            },
            Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE},
        },
        UI::{
            Shell::{
                FileOpenDialog, IFileOpenDialog, ShellExecuteExW, FOS_FORCEFILESYSTEM,
                FOS_PICKFOLDERS, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, SIGDN_FILESYSPATH,
            },
            WindowsAndMessaging::SW_HIDE,
        },
    },
};

const FOLDER: &str = "alice-gameland-launcher";
const PREVIEW_FOLDER: &str = "alice-gameland-tauri-preview";
const LAUNCHER: &str = "AliceGameLand-Tauri.exe";
const UNINSTALLER: &str = "Uninstall AliceGameLand-Tauri.exe";
const ELECTRON_KEY: PCWSTR = w!(r"Software\3d6e8bdf-a47b-5327-b17b-ac5678029a9c");
const ELECTRON_EXE: &str = "앨리스의 게임랜드.exe";
const ELECTRON_UNINSTALLER: &str = "Uninstall 앨리스의 게임랜드.exe";
const INSTALL_KEY: PCWSTR = w!(r"Software\69bd89cc-35ff-5408-9b98-972ba4fb446c");
const INVALID_PATH: &str = "설치할 폴더를 다시 선택해 주세요.";
const RUNNING: &str = "실행 중인 런처를 닫은 뒤 다시 시도해 주세요.";
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Scope {
    User,
    All,
}

impl Scope {
    fn flag(self) -> &'static str {
        match self {
            Self::User => "/currentuser",
            Self::All => "/allusers",
        }
    }

    pub(crate) fn update_flag(self) -> &'static str {
        match self {
            Self::User => "--alice-update-currentuser",
            Self::All => "--alice-update-allusers",
        }
    }
}

pub(crate) enum Mode {
    Install {
        backend: PathBuf,
        update_scope: Option<Scope>,
    },
    Remove {
        backend: PathBuf,
        root: PathBuf,
        scope: Scope,
    },
}

impl Mode {
    pub fn from_args() -> io::Result<Option<Self>> {
        let args: Vec<_> = env::args_os().skip(1).collect();
        let invalid = || io::Error::new(io::ErrorKind::InvalidInput, "Invalid wizard arguments");
        let checked_backend = |path: &OsStr| {
            let path = PathBuf::from(path);
            if path.is_absolute() && path.is_file() {
                Ok(path)
            } else {
                Err(invalid())
            }
        };
        match args.as_slice() {
            [mode, backend] if mode == "--alice-setup" => Ok(Some(Self::Install {
                backend: checked_backend(backend)?,
                update_scope: None,
            })),
            [mode, backend, scope] if mode == "--alice-setup" => {
                let update_scope = [Scope::User, Scope::All]
                    .into_iter()
                    .find(|mode| scope == mode.update_flag())
                    .ok_or_else(invalid)?;
                Ok(Some(Self::Install {
                    backend: checked_backend(backend)?,
                    update_scope: Some(update_scope),
                }))
            }
            [mode, backend, root, scope] if mode == "--alice-remove" => {
                let root = root.to_str().ok_or_else(invalid)?;
                let scope = match scope.to_str() {
                    Some("/currentuser") => Scope::User,
                    Some("/allusers") => Scope::All,
                    _ => return Err(invalid()),
                };
                Ok(Some(Self::Remove {
                    backend: checked_backend(backend)?,
                    root: directory(root).map_err(|_| invalid())?,
                    scope,
                }))
            }
            [mode, ..] if mode == "--alice-setup" || mode == "--alice-remove" => Err(invalid()),
            _ => Ok(None),
        }
    }
}

struct Wizard {
    mode: Mode,
    update_target: Option<UpdateTarget>,
    busy: AtomicBool,
    installed: Mutex<Option<PathBuf>>,
}

struct Operation<'a>(&'a AtomicBool);

impl<'a> Operation<'a> {
    fn start(busy: &'a AtomicBool) -> Result<Self, String> {
        if busy.swap(true, Ordering::AcqRel) {
            Err("작업이 이미 진행 중입니다.".into())
        } else {
            Ok(Self(busy))
        }
    }
}

impl Drop for Operation<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[derive(Serialize)]
struct Defaults {
    user: PathBuf,
    all: PathBuf,
}

#[derive(Clone, Eq, PartialEq, Serialize)]
struct UpdateTarget {
    destination: PathBuf,
    mode: Scope,
}

fn registered_directory(scope: Scope) -> Option<PathBuf> {
    registered_location(scope, INSTALL_KEY)
}

fn registered_location(scope: Scope, key: PCWSTR) -> Option<PathBuf> {
    let hive = match scope {
        Scope::User => HKEY_CURRENT_USER,
        Scope::All => HKEY_LOCAL_MACHINE,
    };
    let mut value = [0u16; 201];
    let mut bytes = std::mem::size_of_val(&value) as u32;
    let status = unsafe {
        RegGetValueW(
            hive,
            key,
            w!("InstallLocation"),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            None,
            Some(value.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let end = value.iter().position(|&unit| unit == 0)?;
    directory(&String::from_utf16(&value[..end]).ok()?).ok()
}

fn registered_update_target(mode: Scope) -> Option<UpdateTarget> {
    let destination = registered_directory(mode)?;
    destination
        .join(LAUNCHER)
        .is_file()
        .then_some(UpdateTarget { destination, mode })
}

fn update_target(scope: Option<Scope>) -> Option<UpdateTarget> {
    match scope {
        Some(scope) => registered_update_target(scope),
        None => [Scope::User, Scope::All]
            .into_iter()
            .find_map(registered_update_target)
            .or_else(|| {
                [Scope::User, Scope::All].into_iter().find_map(|mode| {
                    let destination = registered_location(mode, ELECTRON_KEY)?;
                    (destination.join(ELECTRON_EXE).is_file()
                        && destination.join(ELECTRON_UNINSTALLER).is_file())
                    .then_some(UpdateTarget { destination, mode })
                })
            }),
    }
    .map(|mut target| {
        if target
            .destination
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case(PREVIEW_FOLDER))
        {
            target.destination.set_file_name(FOLDER);
        }
        target
    })
}

pub(crate) fn update_scope_for_running_install() -> Option<Scope> {
    let running = env::current_exe().and_then(fs::canonicalize).ok()?;
    let mut matching = [Scope::User, Scope::All].into_iter().filter(|&scope| {
        registered_update_target(scope)
            .and_then(|target| target.destination.join(LAUNCHER).canonicalize().ok())
            .as_ref()
            == Some(&running)
    });
    let scope = matching.next()?;
    matching.next().is_none().then_some(scope)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Info {
    version: String,
    defaults: Option<Defaults>,
    update_target: Option<UpdateTarget>,
    verify_removal_window: Option<String>,
    verify_electron_transition: bool,
}

#[tauri::command]
fn wizard_info(app: tauri::AppHandle, state: State<'_, Wizard>) -> Result<Info, String> {
    let defaults = if matches!(state.mode, Mode::Install { .. }) {
        let local = env::var_os("LOCALAPPDATA").ok_or("사용자 설치 위치를 찾지 못했어요.")?;
        let programs = env::var_os("ProgramFiles").ok_or("시스템 설치 위치를 찾지 못했어요.")?;
        Some(Defaults {
            user: PathBuf::from(local).join("Programs").join(FOLDER),
            all: PathBuf::from(programs).join(FOLDER),
        })
    } else {
        None
    };
    let verify_removal_window = matches!(state.mode, Mode::Remove { .. })
        && env::var_os("ALICE_TAURI_VERIFY_REMOVE_WIZARD").is_some();
    let mut verified = None;
    if verify_removal_window {
        if let Some(profile) = crate::test_profile().map_err(|error| error.to_string())? {
            fs::write(profile.join("remove-wizard-ready"), b"ready\n")
                .map_err(|error| error.to_string())?;
            verified = env::var("ALICE_TAURI_VERIFY_REMOVE_WIZARD").ok();
        }
    }
    Ok(Info {
        version: app.package_info().version.to_string(),
        defaults,
        update_target: state.update_target.clone(),
        verify_removal_window: verified,
        verify_electron_transition: env::var_os("ALICE_TAURI_VERIFY_ELECTRON_TRANSITION").is_some()
            && state.update_target.as_ref().is_some_and(|target| {
                target.mode == Scope::User
                    && crate::test_profile().ok().flatten().as_deref()
                        == target.destination.parent()
                    && target.destination.join(ELECTRON_EXE).is_file()
            }),
    })
}

fn directory(value: &str) -> Result<PathBuf, String> {
    if value
        .chars()
        .any(|ch| ch.is_control() || matches!(ch, '"' | '<' | '>' | '|' | '?' | '*'))
    {
        return Err(INVALID_PATH.into());
    }
    let mut components = Path::new(value).components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err(INVALID_PATH.into());
    };
    if !matches!(prefix.kind(), Prefix::Disk(_)) || components.next() != Some(Component::RootDir) {
        return Err(INVALID_PATH.into());
    }
    let mut path = PathBuf::from(prefix.as_os_str());
    path.push("\\");
    for component in components {
        let Component::Normal(part) = component else {
            return Err(INVALID_PATH.into());
        };
        let part = part.to_str().ok_or(INVALID_PATH)?;
        let name = part
            .split('.')
            .next()
            .unwrap_or_default()
            .trim_end()
            .to_ascii_lowercase();
        if part.contains(':')
            || part.ends_with(['.', ' '])
            || matches!(
                name.as_str(),
                "con" | "prn" | "aux" | "nul" | "conin$" | "conout$"
            )
            || ["com", "lpt"].iter().any(|prefix| {
                name.strip_prefix(prefix).is_some_and(|suffix| {
                    matches!(
                        suffix,
                        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                    )
                })
            })
        {
            return Err(INVALID_PATH.into());
        }
        path.push(part);
    }
    if path.parent().is_none() || path.as_os_str().encode_wide().count() > 200 {
        return Err(INVALID_PATH.into());
    }
    Ok(path)
}

fn destination(value: &str) -> Result<PathBuf, String> {
    let path = directory(value)?;
    if path
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case(FOLDER))
    {
        return Ok(path);
    }
    let joined = path.join(FOLDER);
    directory(joined.to_str().ok_or(INVALID_PATH)?)
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain([0]).collect()
}

struct Apartment;

impl Apartment {
    fn enter() -> windows::core::Result<Self> {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok()? };
        Ok(Self)
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

fn pick_folder(owner: isize, current: &str) -> Result<Option<PathBuf>, String> {
    use windows::Win32::UI::Shell::{IShellItem, SHCreateItemFromParsingName};

    let chosen = || -> windows::core::Result<_> {
        let _apartment = Apartment::enter()?;
        unsafe {
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
            dialog.SetOptions(dialog.GetOptions()? | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM)?;
            dialog.SetTitle(w!("설치할 폴더 선택"))?;
            if let Ok(initial) = directory(current) {
                let initial = wide(initial.as_os_str());
                let folder: windows::core::Result<IShellItem> =
                    SHCreateItemFromParsingName(PCWSTR(initial.as_ptr()), None);
                if let Ok(folder) = folder {
                    let _ = dialog.SetFolder(&folder);
                }
            }
            match dialog.Show(Some(HWND(owner as *mut _))) {
                Ok(()) => {}
                Err(error) if error.code() == HRESULT::from_win32(ERROR_CANCELLED.0) => {
                    return Ok(None)
                }
                Err(error) => return Err(error),
            }
            let path = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
            let value = path.to_string();
            CoTaskMemFree(Some(path.0.cast()));
            value.map(Some).map_err(|_| {
                windows::core::Error::from_hresult(windows::Win32::Foundation::E_INVALIDARG)
            })
        }
    };
    chosen()
        .map_err(|_| INVALID_PATH.to_owned())?
        .as_deref()
        .map(destination)
        .transpose()
}

#[tauri::command]
async fn wizard_folder(
    window: WebviewWindow,
    state: State<'_, Wizard>,
    current: String,
) -> Result<Option<PathBuf>, String> {
    if !matches!(state.mode, Mode::Install { .. }) {
        return Err(INVALID_PATH.into());
    }
    let _operation = Operation::start(&state.busy)?;
    let owner = window.hwnd().map_err(|_| INVALID_PATH)?.0 as isize;
    tauri::async_runtime::spawn_blocking(move || pick_folder(owner, &current))
        .await
        .map_err(|_| INVALID_PATH.to_owned())?
}

fn hidden(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn run_worker(backend: &Path, args: &str, elevate: bool) -> Result<(), String> {
    let failure = "작업을 완료하지 못했어요. 설치 위치와 권한을 확인해 주세요.";
    let code = if elevate {
        let _apartment = Apartment::enter().map_err(|_| failure)?;
        let file = wide(backend.as_os_str());
        let parameters = wide(OsStr::new(args));
        let mut launch = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpVerb: w!("runas"),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(parameters.as_ptr()),
            nShow: SW_HIDE.0,
            ..Default::default()
        };
        unsafe {
            ShellExecuteExW(&mut launch).map_err(|error| {
                if error.code() == HRESULT::from_win32(ERROR_CANCELLED.0) {
                    "관리자 권한 요청이 취소되었어요. 다시 시도할 수 있어요."
                } else {
                    failure
                }
            })?;
            if launch.hProcess.is_invalid() {
                return Err(failure.into());
            }
            let mut code = 1;
            let waited = WaitForSingleObject(launch.hProcess, INFINITE) == WAIT_OBJECT_0;
            let result = GetExitCodeProcess(launch.hProcess, &mut code);
            let _ = CloseHandle(launch.hProcess);
            if !waited || result.is_err() {
                return Err(failure.into());
            }
            Some(code)
        }
    } else {
        hidden(backend)
            .current_dir(backend.parent().ok_or(failure)?)
            .raw_arg(args)
            .status()
            .map_err(|_| failure)?
            .code()
            .map(|code| code as u32)
    };
    match code {
        Some(0) => Ok(()),
        Some(2) => Err(RUNNING.into()),
        _ => Err(failure.into()),
    }
}

#[derive(Deserialize)]
struct Options {
    destination: String,
    mode: Scope,
}

#[derive(Serialize)]
struct Progress {
    phase: &'static str,
    percent: Option<u8>,
    mode: Option<&'static str>,
}

#[derive(Serialize)]
struct Outcome {
    ok: bool,
    destination: Option<PathBuf>,
    message: Option<String>,
}

impl Outcome {
    fn from_result(result: Result<Option<PathBuf>, String>) -> Self {
        match result {
            Ok(destination) => Self {
                ok: true,
                destination,
                message: None,
            },
            Err(message) => Self {
                ok: false,
                destination: None,
                message: Some(message),
            },
        }
    }
}

fn remove_electron(target: &Path, scope: Scope) -> Result<(), String> {
    let data_root = env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or(INVALID_PATH)?;
    let preferences = crate::preferences::Preferences::load(
        &data_root
            .join("kr.dreamcraft.alicegameland.launcher")
            .join("preferences.json"),
        &data_root,
    )
    .map_err(|_| "기존 게임 데이터 위치를 확인하지 못했어요.")?;
    let root = target.canonicalize().map_err(|_| INVALID_PATH)?;
    let contains_data = preferences.game_data_dir().is_some_and(|data| {
        data.starts_with(target)
            || data
                .canonicalize()
                .is_ok_and(|path| path.starts_with(&root))
    }) || ["instances", "common", "config.json"]
        .iter()
        .any(|name| target.join(name).exists());
    if contains_data {
        return Err("런처 설치 폴더에 게임 파일이나 설정이 있어요. 게임 데이터 폴더를 옮긴 뒤 다시 시도해 주세요.".into());
    }
    let worker = target.join(ELECTRON_UNINSTALLER);
    let args = format!(
        r#"/S {} --updated --alice-wizard-root="{}""#,
        scope.flag(),
        target.display()
    );
    if scope == Scope::All {
        run_worker(&worker, &args, true)?;
    } else {
        let temporary = tempfile::tempdir().map_err(|_| INVALID_PATH)?;
        let copied = temporary.path().join(ELECTRON_UNINSTALLER);
        fs::copy(&worker, &copied).map_err(|_| "기존 런처 제거 파일을 준비하지 못했어요.")?;
        run_worker(&copied, &format!("{args} _?={}", target.display()), false)?;
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    while target.join(ELECTRON_EXE).exists()
        || registered_location(scope, ELECTRON_KEY).as_deref() == Some(target)
    {
        if Instant::now() >= deadline {
            return Err("기존 런처 제거가 끝나지 않았어요. 런처를 닫고 다시 시도해 주세요.".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn install(
    backend: &Path,
    options: Options,
    update: Option<UpdateTarget>,
    update_scope: Option<Scope>,
    progress: Channel<Progress>,
) -> Result<PathBuf, String> {
    if update != update_target(update_scope) || (update_scope.is_some() && update.is_none()) {
        return Err("기존 설치 위치가 바뀌었어요. 설치 파일을 다시 실행해 주세요.".into());
    }
    let updating = update.as_ref().is_some_and(|target| {
        registered_directory(target.mode).as_ref() == Some(&target.destination)
    });
    let (target, mode) = if let Some(update) = update {
        (update.destination, update.mode)
    } else {
        (destination(&options.destination)?, options.mode)
    };
    if !updating
        && registered_location(mode, ELECTRON_KEY).as_ref() == Some(&target)
        && target.join(ELECTRON_EXE).is_file()
        && target.join(ELECTRON_UNINSTALLER).is_file()
    {
        remove_electron(&target, mode)?;
    }
    if !updating {
        match fs::read_dir(&target) {
            Ok(mut entries) => {
                if entries.next().is_some() {
                    return Err("설치 위치에 파일이 있어요. 비어 있는 폴더를 선택해 주세요.".into());
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(INVALID_PATH.into()),
        }
    }
    let _ = progress.send(Progress {
        phase: "checking",
        percent: Some(100),
        mode: None,
    });
    let machine = matches!(mode, Scope::All);
    let _ = progress.send(Progress {
        phase: "installing",
        percent: None,
        mode: Some(if machine { "all" } else { "user" }),
    });
    // NSIS reads everything after /D= as the path. It has to be last and unquoted.
    let updated_flag = if updating { " --updated" } else { "" };
    let args = format!("/S{updated_flag} {} /D={}", mode.flag(), target.display());
    run_worker(backend, &args, machine)?;
    if !target.join(LAUNCHER).is_file() {
        return Err("설치한 런처를 찾지 못했어요.".into());
    }
    Ok(target)
}

#[tauri::command]
async fn wizard_install(
    state: State<'_, Wizard>,
    options: Options,
    progress: Channel<Progress>,
) -> Result<Outcome, String> {
    let Mode::Install {
        backend,
        update_scope,
    } = &state.mode
    else {
        return Err("이 창에서는 설치할 수 없어요.".into());
    };
    let _operation = Operation::start(&state.busy)?;
    let backend = backend.clone();
    let update = state.update_target.clone();
    let update_scope = *update_scope;
    let result = tauri::async_runtime::spawn_blocking(move || {
        install(&backend, options, update, update_scope, progress)
    })
    .await
    .map_err(|_| "설치 작업이 중단됐어요.")?
    .map(Some);
    if let Ok(installed) = &result {
        *state
            .installed
            .lock()
            .map_err(|_| "설치 결과를 읽지 못했어요.")? = installed.clone();
    }
    Ok(Outcome::from_result(result))
}

fn stop_installed_launcher(root: &Path) -> Result<(), String> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

    let error = "실행 중인 런처를 종료하지 못했어요. 런처를 닫은 뒤 다시 시도해 주세요.";
    let launcher = root.join(LAUNCHER);
    let target = match fs::canonicalize(&launcher) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(error.into()),
    };
    let mut system = System::new().map_err(|_| error)?;
    let refresh = ProcessRefreshKind::nothing().with_exe(UpdateKind::Always);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh);
        let mut running = false;
        for process in system.processes().values() {
            if process
                .exe()
                .filter(|path| {
                    path.file_name()
                        .is_some_and(|name| name.eq_ignore_ascii_case(LAUNCHER))
                })
                .and_then(|path| fs::canonicalize(path).ok())
                .as_ref()
                == Some(&target)
            {
                running = true;
                process.kill();
            }
        }
        if !running {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(error.into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[tauri::command]
async fn wizard_remove(state: State<'_, Wizard>) -> Result<Outcome, String> {
    let Mode::Remove {
        backend,
        root,
        scope,
    } = &state.mode
    else {
        return Err("이 창에서는 제거할 수 없어요.".into());
    };
    let _operation = Operation::start(&state.busy)?;
    let (backend, root, scope) = (backend.clone(), root.clone(), *scope);
    let result = tauri::async_runtime::spawn_blocking(move || {
        if registered_directory(scope).as_ref() != Some(&root) {
            return Err("등록된 설치 위치가 바뀌었어요. 제거 화면을 다시 열어 주세요.".into());
        }
        let installed_uninstaller = root.join(UNINSTALLER);
        let machine = scope == Scope::All;
        let mut args = format!(
            r#"/S {} --alice-wizard-root="{}""#,
            scope.flag(),
            root.display()
        );
        if machine {
            if backend != installed_uninstaller {
                return Err("설치 폴더의 제거 파일을 찾지 못했어요.".into());
            }
            // NSIS copies the elevated uninstaller itself so the installed executable can be deleted.
        } else {
            args.push_str(&format!(" _?={}", root.display()));
        }
        stop_installed_launcher(&root)?;
        run_worker(&backend, &args, machine)?;

        let deadline = Instant::now() + Duration::from_secs(15);
        let launcher = root.join(LAUNCHER);
        while launcher.try_exists().unwrap_or(true)
            || installed_uninstaller.try_exists().unwrap_or(true)
            || registered_directory(scope).as_ref() == Some(&root)
        {
            if !machine || Instant::now() >= deadline {
                return Err("제거가 끝나지 않았어요. 런처를 닫은 뒤 다시 시도해 주세요.".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(None)
    })
    .await
    .map_err(|_| "제거 작업이 중단됐어요.")?;
    Ok(Outcome::from_result(result))
}

#[tauri::command]
fn wizard_launch(state: State<'_, Wizard>) -> Result<(), String> {
    let installed = state
        .installed
        .lock()
        .map_err(|_| "설치 결과를 읽지 못했어요.")?;
    let root = installed.as_ref().ok_or("설치한 런처를 찾지 못했어요.")?;
    Command::new(root.join(LAUNCHER))
        .current_dir(root)
        .spawn()
        .map_err(|_| "런처를 열지 못했어요.")?;
    Ok(())
}

pub fn run(context: tauri::Context<tauri::Wry>, mode: Mode) {
    let (label, page, title, resizable) = match &mode {
        Mode::Install { .. } => (
            "setup-wizard",
            "wizard/setup.html",
            "앨리스의 게임랜드 설치",
            true,
        ),
        Mode::Remove { .. } => (
            "remove-wizard",
            "wizard/remove.html",
            "앨리스의 게임랜드 제거",
            false,
        ),
    };
    let update_target = match &mode {
        Mode::Install { update_scope, .. } => {
            let target = update_target(*update_scope);
            if update_scope.is_some() && target.is_none() {
                std::process::exit(2);
            }
            target
        }
        Mode::Remove { .. } => None,
    };
    let result = tauri::Builder::default()
        .manage(Wizard {
            mode,
            update_target,
            busy: AtomicBool::new(false),
            installed: Mutex::new(None),
        })
        .setup(move |app| {
            // NSIS owns and restricts its extraction folder. WebView2 needs a user-owned profile.
            let local_data = app.path().local_data_dir()?;
            let data = tempfile::Builder::new()
                .prefix("AliceGameLand-Wizard-")
                .tempdir_in(local_data)?;
            let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App(page.into()))
                .title(title)
                .inner_size(720.0, 520.0)
                .min_inner_size(640.0, 480.0)
                .decorations(false)
                .resizable(resizable)
                .center()
                .data_directory(data.path().to_path_buf())
                .build()?;
            app.manage(data);
            let owner = app.handle().clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    if owner.state::<Wizard>().busy.load(Ordering::Acquire) {
                        api.prevent_close();
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            wizard_info,
            wizard_folder,
            wizard_install,
            wizard_remove,
            wizard_launch
        ])
        .run(context);
    if result.is_err() {
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_nsis_destinations_to_normal_local_directories() {
        let root = r"C:\Program Files\앨리스";
        assert_eq!(destination(root).unwrap(), Path::new(root).join(FOLDER));
        assert_eq!(
            destination(&format!("{root}\\{FOLDER}")).unwrap(),
            Path::new(root).join(FOLDER)
        );
        for bad in [
            r"C:\",
            r"C:relative",
            r"\\host\share\apps",
            r"C:\apps\..\windows",
            r#"C:\bad"quote"#,
            r"C:\apps\NUL",
            r"C:\bad:stream",
            r"C:\apps\bad.",
        ] {
            assert!(destination(bad).is_err(), "{bad}");
        }
    }
}
