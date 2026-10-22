# 앨리스의 게임랜드 런처 작업 지침

## 프로젝트 구조

- 이 저장소는 Electron 기반 Helios 런처다. 게임 파일과 서버별 모드 목록은 형제 저장소 `../AliceGameLand-Distribution/distribution.json`에서 관리한다.
- 게임 설정은 서버별 `instances/<서버 ID>/options.txt`와 `config/iris.properties`에 저장된다. 런처 소스와 배포 매니페스트를 함께 확인한다.
- 기존 작업 파일과 사용자 설정을 보존한다. 요청 범위 밖의 변경, 배포, 게임 또는 서버 실행은 하지 않는다.

## 신규 설치 기본값

- 마인크래프트 튜토리얼을 끈다: `options.txt`의 `tutorialStep:none`.
- 튜토리얼 비활성화 여부는 개인 PC의 옵션으로 판단하지 않는다. 실제 배포 인덱스의 URL에서 받은 각 서버의 `options.txt`와 생성 코드에 `tutorialStep:none`이 있는지 검증한다.
- 기존 설치도 게임 실행 직전에 `tutorialStep`만 `none`으로 맞춘다. 다른 옵션과 셰이더 설정은 보존하며, 옵션 파일 전체를 강제로 재다운로드하지 않는다.
- GUI 배율은 4로 설정한다: `options.txt`의 `guiScale:4`.
- 기본 셰이더는 `ComplementaryReimagined_r5.9.1.zip`이다. Iris 설정은 `enableShaders=true`와 `shaderPack=ComplementaryReimagined_r5.9.1.zip`을 사용하고, 해당 ZIP이 배포 목록에 있는지 확인한다.
- Sodium, Iris, Koreanify, More Culling과 기존 Dynamic FPS, FerriteCore, ImmediatelyFast, Lithium을 서버별 모드 목록에 유지한다. Koreanify는 한국어 지원 모드다. 모드 추가·버전 변경 전 Minecraft 26.2/NeoForge 호환성과 배포 파일을 확인한다.
- 이 값들은 새 인스턴스의 기본값이다. 이미 존재하는 `options.txt`와 `config/iris.properties`는 사용자 선택을 덮어쓰지 않는다. 배포 매니페스트에서 두 파일은 MD5 검사 없이 파일이 없을 때만 내려받도록 유지한다.

## 변경 및 검증

- 기본값을 바꿀 때는 배포 저장소의 `scripts/generate.mjs`, `scripts/add-diamond.mjs`, 기본 설정 원본, 각 서버의 `distribution.json` 항목을 확인한다. 크라임씬은 게임랜드 배포 항목을 복제하므로 함께 확인한다.
- 런처 코드 변경 시 `npm run lint`와 관련 테스트를 실행한다. 배포 변경 시 파일 경로, 해시, 다운로드 URL, 새 설치와 기존 설치의 설정 보존을 확인한다.
- 빌드·정적 검증과 실제 게임 실행·셰이더 화면 검증을 구분해 보고한다. 릴리스 게시나 사용자 게임 프로세스 실행은 별도 요청이 있을 때 진행한다.
