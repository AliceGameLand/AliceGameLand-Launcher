<p align="center"><img src="./app/assets/images/SealCircle.png" width="120" height="120" alt="앨리스의 게임랜드 로고"></p>

<h1 align="center">앨리스의 게임랜드 런처</h1>

<p align="center">앨리스와 함께하는 Minecraft 콘텐츠를 한곳에서 즐겨보세요.</p>

<p align="center">
  <a href="https://github.com/AliceGameLand/AliceGameLand-Launcher/releases/latest">최신 버전 다운로드</a> ·
  <a href="https://github.com/AliceGameLand/AliceGameLand-Launcher/releases">업데이트 내역</a> ·
  <a href="https://github.com/AliceGameLand/AliceGameLand-Launcher/issues">문제 신고</a>
</p>

## 설치 및 시작

Windows x64용 설치파일은 [최신 릴리스](https://github.com/AliceGameLand/AliceGameLand-Launcher/releases/latest)에서 받을 수 있습니다.

1. `alice-gameland-setup-버전.exe`를 내려받아 실행합니다.
2. 설치 위치를 확인하고 런처를 설치합니다. 게임 실행에 필요한 Java도 함께 설치됩니다.
3. 런처를 열고 Minecraft Java Edition을 보유한 Microsoft 계정으로 로그인합니다.
4. 플레이할 서버를 선택한 뒤 **플레이**를 누릅니다.

게임 파일과 서버별 모드는 플레이할 때 확인하고, 필요한 파일을 자동으로 내려받습니다. 처음 실행할 때는 다운로드에 시간이 걸릴 수 있습니다.

## 주요 기능

- **서버 선택** — 서버별 Minecraft 버전과 로더에 맞는 실행 환경을 준비합니다.
- **Microsoft 계정 관리** — 여러 계정을 등록하고 전환할 수 있습니다.
- **게임 파일 관리** — 실행 전 파일을 확인하고 누락되거나 손상된 파일을 다시 내려받습니다. 정상 확인한 파일은 캐시를 사용해 반복 검사 부담을 줄입니다.
- **서버별 설정** — 게임 해상도·전체 화면·모드·Java 및 메모리 설정을 관리합니다.
- **여러 게임 실행** — 게임 준비가 끝나면 다시 **플레이**를 눌러 다른 게임 창을 실행할 수 있습니다.
- **빠른 접근** — 홈에서 게임 폴더를 열거나 문제를 신고할 수 있습니다.

## 런처 업데이트

새 버전이 확인되면 런처에 업데이트 알림이 표시됩니다. **다운로드와 설치는 사용자가 버튼을 눌러 진행**하며, 백그라운드 자동 다운로드와 종료 시 자동 설치는 사용하지 않습니다.

업데이트와 설치파일 재실행 시 기존 설치 위치를 인식합니다. 게임 데이터는 프로그램 설치 폴더와 별도로 관리하며, 기존 계정과 설정을 유지하도록 구성되어 있습니다.

## 게임 데이터와 문제 신고

Windows의 기본 게임 데이터 경로는 `%APPDATA%\.alicegameland`이며, 설정에서 저장 위치를 변경할 수 있습니다. 서버별 게임 파일은 그 아래 `instances/<서버 ID>`에 저장됩니다. 홈의 **게임 폴더 열기**로 해당 서버의 폴더를 확인할 수 있습니다.

문제가 발생하면 홈의 **문제 신고** 또는 [GitHub Issues](https://github.com/AliceGameLand/AliceGameLand-Launcher/issues)를 이용해 주세요. 런처 버전, 선택한 서버, 재현 순서와 오류 화면을 함께 남겨주시면 확인에 도움이 됩니다. 로그인 토큰 등 인증 정보는 공개하지 마세요.

## 개발

### 실행 환경

- Node.js 22 및 npm
- Windows 설치파일을 빌드하려면 Windows x64 환경

```powershell
git clone https://github.com/AliceGameLand/AliceGameLand-Launcher.git
cd AliceGameLand-Launcher
npm ci
npm start
```

Microsoft 로그인 설정은 `app/assets/config/auth.json`에서 관리합니다. 별도 런처를 개발하는 경우 본인 소유 Entra 애플리케이션의 클라이언트 ID를 사용하세요. 로컬 테스트에서는 `ALICE_MS_CLIENT_ID` 환경 변수로 덮어쓸 수 있습니다. 자세한 설정은 [Microsoft 인증 안내](docs/MicrosoftAuth.md)를 참고하세요.

### 검증

```powershell
npm run lint
node --test test/*.test.js
```

### Windows 설치파일 빌드

```powershell
npm run runtime:prepare
npm run dist:setup
```

`dist/custom-setup/`에 사용자용 설치파일, `.blockmap`, `latest.yml`이 생성됩니다. 설치 화면은 Electron으로 구현했으며, 실제 설치는 내부 NSIS 엔진으로 처리합니다. 이 명령은 릴리스를 게시하지 않습니다.

### Tauri 미리보기 업데이트 준비

Windows x64에서는 `npm run tauri -- build --no-bundle` 후 `node scripts/build-tauri-setup.mjs`를 실행합니다. Apple Silicon에서는 `npm run tauri -- build --bundles app,dmg`로 DMG와 앱 번들을 만들고 `src-tauri/target/release/bundle/macos`의 앱 번들을 `.app.tar.gz`로 압축합니다.

업데이트를 사용할 빌드에는 빌드 전에 `ALICE_TAURI_UPDATE_PUBLIC_KEY`에 Tauri 공개 키 내용을 설정합니다. 두 플랫폼의 패키지를 준비한 뒤 서명 키를 `TAURI_SIGNING_PRIVATE_KEY_PATH` 또는 `TAURI_SIGNING_PRIVATE_KEY`에 지정하고 아래 명령을 실행합니다.

```text
node scripts/prepare-tauri-update.mjs WINDOWS_SETUP_EXE MACOS_APP_TAR_GZ
```

`dist/tauri-update/`에 버전 서명이 적용된 패키지와 `tauri-update.json`이 생성됩니다. 패키지는 메타데이터가 가리키는 버전 태그에 올립니다. 안정 버전은 해당 릴리스의 메타데이터를, 프리릴리스는 `tauri-preview-feed` 태그의 메타데이터를 확인합니다.

### 주요 구조

| 경로 | 역할 |
| --- | --- |
| `index.js` | Electron 메인 프로세스, 창 및 업데이트 관리 |
| `app/` | EJS 화면, 스타일, 로그인·설정·게임 실행 코드 |
| `installer/` | Windows 설치 화면과 설치 처리 |
| `scripts/` | Java 준비 및 설치파일 빌드 도구 |
| `patches/` | 런처에서 사용하는 의존성 수정 사항 |
| `test/` | 회귀 테스트 |

런처 자체의 설치파일과 업데이트는 이 저장소의 GitHub Releases에서 제공합니다. 서버 목록·모드·리소스팩 등 게임 배포 정보는 [AliceGameLand-Distribution](https://github.com/AliceGameLand/AliceGameLand-Distribution)에서 별도로 관리합니다.

## 기반 프로젝트 및 저작권

이 런처는 [Helios Launcher](https://github.com/dscalzi/HeliosLauncher)를 기반으로 앨리스의 게임랜드에 맞게 개발한 독립 프로젝트입니다. 원본 코드의 저작권과 MIT 라이선스 고지는 [LICENSE.txt](LICENSE.txt)에 보존합니다. 포함된 이미지·폰트 등 개별 자산의 권리는 각 권리자에게 있습니다.

Minecraft는 Mojang 및 Microsoft의 상표입니다. 이 프로젝트는 Mojang 또는 Microsoft의 공식 제품이 아닙니다.
