---
name: 앨리스의 게임랜드
description: 아이보리 종이와 파란 리본을 쓰는 정원 안내책자
colors:
  paper: '#f8f4ea'
  surface: '#fffcf5'
  rail: '#f1eee6'
  ink: '#1d2a3a'
  muted: '#526170'
  line: '#d8d4c7'
  control-line: '#8a8c90'
  blue: '#245ba8'
  blue-hover: '#1c498a'
  blue-wash: '#e4eefc'
  focus: '#174581'
  success: '#286644'
  warning: '#82550e'
  danger: '#a13c42'
  on-blue: '#ffffff'
typography:
  entry-title:
    fontFamily: '"Pretendard Variable", "Malgun Gothic", "Apple SD Gothic Neo", system-ui, sans-serif'
    fontSize: 25px
    fontWeight: 700
    lineHeight: 1.25
    letterSpacing: '-0.025em'
  server-title:
    fontFamily: '"Pretendard Variable", "Malgun Gothic", "Apple SD Gothic Neo", system-ui, sans-serif'
    fontSize: 24px
    fontWeight: 700
    lineHeight: 1.3
    letterSpacing: '-0.025em'
  server-list:
    fontSize: 16px
    fontWeight: 600
    lineHeight: 1.45
  settings-title:
    fontFamily: '"Pretendard Variable", "Malgun Gothic", "Apple SD Gothic Neo", system-ui, sans-serif'
    fontSize: 24px
    fontWeight: 700
    lineHeight: 1.3
  settings-label:
    fontSize: 15px
    fontWeight: 650
  settings-section:
    fontSize: 17px
    fontWeight: 700
  changelog-code:
    fontFamily: 'ui-monospace, Consolas, monospace'
  loading-title:
    fontFamily: '"Avenir Medium", sans-serif'
    fontSize: 24px
  dialog-title:
    fontFamily: '"Pretendard Variable", "Malgun Gothic", "Apple SD Gothic Neo", system-ui, sans-serif'
    fontSize: 21px
    fontWeight: 700
    lineHeight: 1.25
  installer-title:
    fontFamily: 'Pretendard, "Malgun Gothic", sans-serif'
    fontSize: 28px
    fontWeight: 750
    lineHeight: 1.35
    letterSpacing: '-0.025em'
  installer-body:
    fontFamily: 'Pretendard, "Malgun Gothic", sans-serif'
    fontSize: 15px
    lineHeight: 1.6
  body:
    fontFamily: '"Pretendard Variable", "Malgun Gothic", "Apple SD Gothic Neo", system-ui, sans-serif'
    fontSize: 14px
    lineHeight: 1.5
  meta:
    fontFamily: '"Pretendard Variable", "Malgun Gothic", "Apple SD Gothic Neo", system-ui, sans-serif'
    fontSize: 13px
rounded:
  icon: 6px
  control: 8px
  panel: 12px
spacing:
  compact: 4px
  control: 8px
  group: 16px
  stage: 24px
  section: 24px
components:
  button-primary:
    backgroundColor: '{colors.blue}'
    textColor: '{colors.on-blue}'
    rounded: '{rounded.control}'
    padding: 7px 14px
    height: 40px
  button-primary-hover:
    backgroundColor: '{colors.blue-hover}'
  button-primary-disabled:
    backgroundColor: '{colors.rail}'
    textColor: '{colors.muted}'
  button-quiet:
    backgroundColor: transparent
    textColor: '{colors.ink}'
    rounded: '{rounded.control}'
    padding: 7px 14px
    height: 40px
  input-text:
    backgroundColor: '{colors.surface}'
    textColor: '{colors.ink}'
    rounded: '{rounded.icon}'
    height: 38px
  navigation-selected:
    backgroundColor: '{colors.surface}'
    textColor: '{colors.ink}'
    rounded: '{rounded.control}'
  entry-panel:
    backgroundColor: '{colors.surface}'
    textColor: '{colors.ink}'
    rounded: '{rounded.panel}'
    padding: 36px
    width: 470px
  dialog-panel:
    backgroundColor: '{colors.surface}'
    textColor: '{colors.ink}'
    rounded: '{rounded.panel}'
    padding: 24px
    width: 540px
  launch-progress:
    backgroundColor: '{colors.blue-wash}'
    rounded: 5px
    height: 7px
  installer-primary:
    backgroundColor: '{colors.blue}'
    textColor: '{colors.on-blue}'
    rounded: '{rounded.control}'
    padding: 10px 20px
    height: 48px
  installer-folder:
    backgroundColor: '{colors.surface}'
    textColor: '{colors.ink}'
    rounded: '{rounded.control}'
  installer-progress:
    backgroundColor: '{colors.blue-wash}'
    rounded: 5px
    height: 8px
---

# Design System: 앨리스의 게임랜드

## Overview

정원 안내책자는 원본 일러스트와 아이보리 배경을 사용한다. 파란 리본은 선택과 실행을 표시한다.

설치 프로그램은 같은 팔레트와 상단 마스코트를 쓰는 별도의 프레임 없는 창이다. 정원 안내 패널 없이 설치 제목과 옵션이 전체 폭을 사용한다.

## Colors

아이보리 배경과 밝은 콘텐츠 면을 구분하고, 파란색은 플레이·저장·선택에 쓴다.

## Typography

Pretendard Variable을 런처에 포함해 오프라인에서도 같은 글꼴을 사용한다. 홈과 설정 제목은 24px, 설정의 분류 제목은 17px, 항목명은 15px, 본문과 설명은 14px이다. 서버 목록은 16px·600 굵기이고 선택한 서버는 650 굵기다. 홈 제목의 줄 간격은 1.3, 목록은 1.45이다. 런처 버전과 계정 UUID 같은 보조 정보는 13px이다.

760px 미만에서는 홈 제목을 20px, 가로 서버 목록을 14px로 표시한다. 설정 탐색은 15px·550 굵기이고 선택한 탭은 700 굵기다. 시작 스플래시의 제목은 기존 Avenir Medium 24px을 사용한다.

설치 창의 Pretendard는 런처와 같은 로컬 PretendardVariable.woff2의 별칭이다. 단계 제목은 28px·750 굵기·줄 간격 1.35이며, 680px 이하에서는 단계 제목 25px이다. 설치 본문은 15px·줄 간격 1.6, 단계 설명은 줄 간격 1.7을 사용한다.

[Pretendard v1.3.9](https://github.com/orioncactus/pretendard/tree/v1.3.9)의 SIL Open Font License 전문은 app/assets/fonts/Pretendard-LICENSE.txt에 포함한다.

## Layout

홈 사이드바는 `clamp(240px, 20vw, 260px)`, 설정 사이드바는 `clamp(220px, 21vw, 268px)`을 사용한다. 48px 공통 창 프레임을 사용한다. 서버 목록 옆의 홈 콘텐츠는 본문 전체 폭과 높이를 채운다. 외부 패딩·카드 테두리·둥근 모서리 없이 그림과 실행 영역을 붙인다. 실행 영역의 높이를 확보한 뒤 남은 높이에 그림을 cover로 배치하며 비율은 유지하고 가장자리를 자른다. 그림 초점은 가로 중앙·세로 65%이다.

서버 목록의 제목과 하단 도구는 고정하고 목록만 스크롤한다. 하단 두 도구의 아이콘과 문구 열을 맞추고, 버전은 문제 신고 오른쪽의 같은 행에 14px·450 굵기의 보조 문구로 표시한다. 두 도구는 각각 40px 높이이며 구분선 아래와 마지막 행 아래의 내부 여백은 12px로 맞춘다. 하단에는 선택한 서버의 게임 폴더 열기와 문제 신고를 둔다. 서버 이름은 한 줄 말줄임으로 표시하며 버튼의 title과 선택한 서버의 본문 제목에는 전체 이름을 유지한다. 760px 미만에서는 목록을 68px 높이의 가로 탐색으로 옮기고, 두 기능은 목록 오른쪽의 36px 아이콘 버튼과 그 아래 버전으로 표시한다.

홈 실행 영역은 최소 112px 높이와 상하 20px·좌우 24px 패딩을 사용한다. 좁은 홈에서도 그림과 실행 영역은 빈 간격 없이 붙이고 실행 영역의 내부 패딩만 12px로 줄인다. 조작 영역은 기본 240px, 760px 미만에서는 220px이다. 플레이 버튼은 이 안에서 오른쪽에 정렬하고 준비 진행률은 전체 폭을 사용해 문구와 백분율을 읽을 수 있게 한다. 첫 실행부터 환영 문구와 Microsoft 로그인 버튼을 함께 보여주며 별도의 환영·계속 화면은 두지 않는다. 로그인한 계정이 있으면 홈으로 이동한다. 로그인은 정원 그림과 안내를 나란히 보여주며 좁은 창에서는 안내에 집중한다.

설정은 최대 840px 읽기폭, 섹션 간 24px 간격을 사용한다. 콘텐츠 패딩은 상하 24px, 좌우 `clamp(18px, 3vw, 40px)`이며 창 높이 630px 이하는 상하 20px이다. 제목과 설명 아래에는 긴 구분선 대신 20px 간격을 두고 좁은 창에서는 16px로 줄인다. 게임·런처의 일반 설정 항목은 밑선 없이 상하 14px 패딩과 항목 간 공간으로 구분한다. 탐색의 런처 항목 다음 구분선으로 정보·업데이트를 분리한다. 760px 미만 설정 탐색 폭은 154px, 590px 이하는 126px이다. 창 높이 540px 이하에서는 탐색 행 최소 높이를 42px에서 36px로 줄인다.

설치 창은 기본 720×520px이며 작업 영역에 맞춰 축소하고 최소 640×480px을 유지한다. 48px 드래그 제목 표시줄 아래 설치 내용이 전체 폭을 사용하고 필요에 따라 스크롤하며 아래 행동은 남은 공간의 끝에 둔다. 내용 패딩은 상단 24px·좌우 28px·하단 20px이며, 680px 이하에서는 상단 20px·좌우 24px·하단 16px이다. 높이 500px 이하에서는 상단 패딩을 18px로 줄인다. 일반 그룹 간격은 20px, 완료 화면은 16px로 정리하고 완료 행동은 경로 아래 20px 간격으로 두고 두 버튼 사이는 8px로 둔다.

## Elevation & Depth

기본 면은 색과 얇은 구분선으로 나누고 대화상자는 그림자로 구분한다. 설정의 모드 목록과 릴리스 노트는 외곽 카드 없이 분류 제목과 가로 구분선으로 묶는다. 선택한 서버·계정, 런처 버전 상태에는 기존 아이보리 면을 사용한다.

## Shapes

일러스트 패널과 대화상자는 12px, 기본 버튼은 8px 반경을 쓴다. 설치 버튼과 위치 입력은 8px 반경을 쓴다.

## Components

시작 로딩은 기존 정원 일러스트를 창에 채우고 왼쪽 아래에 78px 마스코트와 24px 제목을 표시한다. 기본 버튼에는 최초 개편의 파랑과 흰색 글자를 쓴다. 로딩 중에는 상단 창 프레임을 숨긴다.

홈 서버 사이드바의 좌우 패딩은 16px이다. 서버 행에는 32px 아이콘과 16px 이름을 10px 간격으로 정렬하고 10px 패딩을 사용한다. 행 최소 높이는 60px이며 서로 압축되지 않는다. 선택한 서버는 밝은 면, 파란 이름과 세로 중앙의 24px 책갈피로 표시한다. 좁은 창의 행은 내용에 맞춘 최대 280px 폭과 높이 56px이며 한 줄 말줄임을 유지하고 아이콘은 26px이다. 서버 목록 제목은 기본 20px이고 좁은 창은 13px이다.

플레이 버튼은 폭 200px·최소 높이 56px, 기본 공통 버튼은 최소 40px, 설정의 보조 버튼과 입력은 최소 38px이다. 준비 단계와 진행률은 14px로 표시하며 단계 문구는 600 굵기, 막대 위 간격은 8px로 정리하고 240px 조작 영역 전체를 사용하며 좁은 창에서는 220px로 줄인다. 파일 검사·다운로드에는 측정된 백분율을 표시하고 설치·실행 준비에는 백분율 없이 불확정 막대를 사용한다. 진행률 전환에 180ms reveal, 불확정 막대에 1.6s 왕복 이동, 기본 버튼 누름에 1px 이동 피드백을 사용하며 동작 줄이기 설정을 반영한다. 준비 중에는 중복 실행과 서버 전환을 잠그고 준비가 끝나면 여러 게임을 실행할 수 있다. 실행 중인 게임이 있으면 버튼을 추가 실행으로 표시한다. 각 게임의 종료·시작 오류·출력은 독립적으로 처리하고 Discord 상태는 마지막으로 시작한 게임을 따른다.

홈 서버명은 750 굵기와 1.25 줄 높이를 사용하고 버전·로더·접속 상태를 아래의 별도 행으로 묶어 6px 간격을 둔다. 보조 정보는 14px, 버전은 450·상태는 550 굵기로 구분한다. 좁은 창에서는 서버명과 정보 행 사이 간격을 6px로 줄인다. 서버 조회 실패는 접속 상태 확인 불가만 표시한다. 게임 준비 오류는 다시 시도·닫기·기존 문제 신고 연결을 제공한다. 기술 상세는 펼침 영역에 두고 오류 정보 복사 시 자격 증명·이메일·개인 홈 경로를 가린다. 짧은 창에서는 상세 영역 높이를 줄이고 대화상자 내부 스크롤로 모든 조작에 접근한다.

설정 입력값 검증은 저장 버튼과 서버 변경에 공통 적용한다. 오류가 있으면 저장 영역에 입력값 확인 링크를 표시하고 해당 탭의 입력칸으로 포커스를 옮긴다. Microsoft 인증 거부·토큰 교환 실패는 로그인 시작 화면으로 돌아가 재시도할 수 있게 하며, 설정에서 시작한 계정 추가는 설정으로 돌아간다. 오류창을 닫으면 시작한 로그인 버튼으로 포커스를 돌린다. 계정 없는 홈에서는 플레이를 비활성화한다.

설정의 화면 제목은 24px·750, 일반 항목명은 15px·600, 파일 경로 등 그룹 제목은 16px·700으로 구분한다. 항목명과 설명 사이에는 4px 간격을 두고 보조 버튼은 같은 높이와 중앙 정렬을 사용한다. 경로 입력과 선택 버튼은 합친 외곽 높이가 일반 입력과 같도록 내부 최소 높이를 36px로 맞춘다.

로그인 화면은 최대 920px의 그림·안내 구도로 정리하고 전체를 감싸는 박스를 제거한다. 두 열 간격은 36px, 안내 묶음은 최대 360px이다. 기존 마스코트는 56px, 제목은 25px·750, 설명은 14px와 1.75 줄 높이를 사용한다. 제목·설명을 10px 간격으로 묶고 로그인 버튼까지 24px를 둔다. 좁은 창에서는 그림을 숨기고 최대 360px의 안내와 버튼을 중앙에 배치한다. 환영 문구와 Microsoft 로그인 동작을 직접 연결한다.

상단의 계정과 설정 버튼은 공통 flex 중앙 정렬과 최소 36px 높이를 사용한다. Windows 창 조작 버튼은 폭 38px, 제목 표시줄의 내부 높이 전체를 채우는 직사각형 hover 면을 사용한다. 최소화·최대화는 기존 보조 면, 닫기는 오류색을 사용하며 아이콘은 같은 크기와 중앙 정렬을 유지한다.

설정 탐색은 7개의 같은 선형 아이콘을 사용한다. 아이콘은 20px·1.5px 선이며 좁은 창은 18px이다. 선택한 탐색 행은 밝은 면, 굵은 글자, 파란 책갈피와 아이콘으로 표시한다. Mod와 Java의 적용 서버 영역은 같은 역할의 아이보리 면과 12px 패딩을 사용한다.

모드 분류 제목은 17px, 행 최소 높이는 52px이고 상하 8px 패딩을 사용한다. 이름은 15px·600, 버전과 필수 상태는 14px이다. 필수·선택 분류는 표시할 항목이 있을 때 나타난다. 넓은 창에서는 이름과 버전은 왼쪽, 상태와 조작은 오른쪽에 두고, 좁은 창에서는 조작을 다음 줄 오른쪽으로 옮겨 긴 파일명의 읽기폭을 확보한다. 계정 행의 최소 높이는 88px이며 이미지와 조작 영역에 12px 패딩을 사용한다. 현재 계정 행에는 아이보리 선택 면을 적용하고 선택 버튼을 비활성화한다. 계정 선택·로그아웃은 가로로 두며 좁은 창에서 줄바꿈한다. UUID는 펼침 영역에 둔다.

Java의 전체·사용 가능 RAM은 슬라이더 아래 가로 정보로 표시한다. 메모리 설명과 다음 섹션의 간격은 공통 섹션 값으로 관리한다. Java와 데이터 경로는 선택하고 복사할 수 있다. 정보·업데이트의 버전 상태와 행동은 20px 패딩의 한 줄에 묶고 가용 폭에 따라 줄바꿈한다. 릴리스 노트는 위 구분선과 17px 제목으로 시작하며 줄 간격 1.7, 문단 최대 72ch를 사용한다. 코드는 기존 연한 아이보리 배경으로 표시한다.

대화상자는 최대 540px, 패딩 24px이고 선택 목록만 스크롤한다. 팝업 선택 배경과 키보드 포커스 외곽선을 나눠 표시한다.

플레이어 머리는 배경색과 모서리 마스크 없이 투명하게 표시한다. 클릭음은 저역 통과 필터를 적용한 40ms의 짧고 둥근 탭 소리이며 실제 사용자 입력에만 반응한다. 오디오 장치를 사용할 수 없을 때도 버튼 동작은 계속된다.

설치 창은 위치·설치 대상 선택, 확인·설치, 완료의 세 화면을 전체 폭에서 전환한다. 설치 제목과 환영 문구는 친근한 해요체를 쓰고, Java 설치·파일 확인·설치 완료 같은 상태 안내는 합니다체를 섞어 구분한다. 위치 입력은 읽기 전용이고 변경 버튼으로 폴더를 선택하며 설치 대상은 펼침 영역에 둔다. 주요 버튼은 최소 48px 높이와 650 굵기, 파란 배경을 사용한다. 파일 해시 확인에만 실제 읽은 바이트의 백분율과 8px 진행 막대를 표시한다. 설치 단계는 백분율 없이 불확정 막대와 현재 단계 문구를 표시하며 중복 실행과 쓰기 중 닫기를 막는다. 오류는 본문과 다시 시도 버튼으로 알리고 완료 시 런처 시작과 나중에 시작을 200px 폭의 세로 버튼 그룹으로 제공한다. 화면 진입은 280ms, 불확정 막대는 1.5s 전환을 사용하고 동작 줄이기를 반영한다. 설치 창 자산은 기존 원본을 복사하며 스테이징의 asset-origins.json에 원본 경로를 기록한다.

## Do's and Don'ts

정원 그림의 원본 비율을 유지하고 키보드 포커스를 표시한다. 글자 크기를 줄여 긴 내용을 맞추지 않는다. 본문의 긴 내용은 줄바꿈·공간 재배치를 사용하고 홈 서버 목록은 한 줄 말줄임과 전체 이름 title을 유지한다. 섹션마다 외곽 박스를 추가하지 않는다. 이번 정리에서는 새 라이브러리·폰트·래스터 자산을 추가하지 않고 기존 팔레트와 정원·마스코트 원본을 유지한다. 설치 진행률에 측정하지 않은 백분율을 만들지 않는다.

로그인·로그아웃 대기는 박스 없이 종이 배경 위에 40px 스피너와 25px·700 굵기의 상태 문구를 20px 간격으로 중앙 배치한다.

신규 설정 생성 시에만 서버 선택 안내를 활성화한다. 로그인 완료 후 박스 없는 전용 서버 선택 페이지로 안내하고, 서버를 직접 선택하면 선택한 서버의 홈으로 이동한다. 홈에는 별도 안내 문구를 두지 않는다. 기본으로 표시된 서버를 다시 선택해도 확인을 완료하며, 안내 완료 여부는 재시작 후에도 유지한다. 기존 설정에는 안내를 소급 적용하지 않는다.

최초 서버 선택 페이지는 최대 540px의 한 열을 사용한다. 제목은 26px·750, 설명은 15px로 표시하고 24px 아래부터 서버 버튼을 배치한다. 각 버튼은 밝은 종이 면과 1px 테두리, 8px 모서리, 17px 문구·36px 아이콘·이동 화살표로 구성한다. 기본 행 높이는 66px, 간격은 8px이며 좁은 창에서는 각각 56px·6px로 줄인다. 페이지 전체를 감싸는 박스는 두지 않는다.

반복 실행 시에도 버튼 이름은 플레이로 유지한다. 파일 확인은 기대 해시와 파일 크기·수정 시각·변경 시각·파일 식별자가 같은 정상 파일만 24시간 동안 재사용하며, 이후에는 다시 전체 해시를 검사한다. 다운로드 후 검증은 캐시를 사용하지 않는다. 진행바는 기존 표시와 1.6초 왕복 애니메이션을 유지한다.

## 서버별 홈 배경 (1.0.29)

혜안져스 미니게임은 기존 정원 `0.jpg`를 유지한다. 다이아 대결·버컴퍼니 미니게임·PvP 연습·꼬리잡기·크라임씬은 `app/assets/images/backgrounds/servers/`의 개별 배경을 사용한다. 새 그림은 기존 정원과 캐릭터를 기준으로 만든 뒤 게임별 표정을 조절한 최종안(배포 저장소의 illustrated-v3)이다. 서버 ID로 선택하고, 알 수 없는 서버나 이미지 로드 오류에는 기존 정원을 표시한다.

홈 배경에만 `brightness(0.96) saturate(1.06) contrast(1.02)`를 적용한다. 메뉴·아이콘·버튼과 로그인·설치 화면은 변경하지 않는다. 이번 배포에는 새 아이콘 시안을 포함하지 않는다.
