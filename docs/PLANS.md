# Codex Analysis Visualizer Plan

## 문서 목적

이 문서는 이 저장소의 제품 방향과 구현 우선순위를 빠르게 공유하기 위한 실행 계획 문서다. 장문의 설계 메모를 누적하기보다, 현재 상태와 다음 단계, 의사결정 기준을 명확히 유지하는 것을 목표로 한다.

## 한 줄 목표

Codex가 코드베이스를 읽고 수정하는 과정을, 개발자가 실시간으로 이해하고 따라갈 수 있는 데스크톱 관찰 도구를 만든다.

## 제품 원칙

- 기존 작업 흐름을 바꾸지 않는다. 사용자는 Codex CLI, Codex App, Orca 같은 익숙한 환경을 계속 사용한다.
- 초반에는 제어보다 관찰에 집중한다. 우선은 읽기 중심의 observer UI를 만든다.
- 저장소 기반 추적을 기본 경로로 삼는다. 런타임 프로세스에 직접 attach하는 방식은 후순위다.
- 시각화는 전체 코드베이스보다 현재 작업 맥락에 집중한다.
- 구조는 단순하게 시작하되, 나중에 재생(replay)과 자동 인사이트로 확장 가능해야 한다.

## 현재 구현 상태

현재 저장소에는 아래 기반 작업이 이미 들어와 있다.

- Tauri + React 기반 데스크톱 앱 스캐폴드
- Rust 기반 세션 watch 뼈대
  - `state_5.sqlite`
  - `state_5.sqlite-wal`
  - `history.jsonl`
  - `sessions/`
- Rust 기반 코드 인덱서 스캐폴드
- 시각화 이벤트 타입 패키지
- MCP placeholder 서버 패키지

관련 구현 위치:

- `app/src-tauri/src/session_watch.rs`
- `app/src-tauri/src/indexer/*`

## 지금 기준의 MVP

MVP는 아래 질문에 답할 수 있어야 한다.

1. 지금 어떤 Codex 세션이 움직이고 있는가?
2. 그 세션이 어떤 workspace와 파일을 보고 있는가?
3. 어떤 명령, 패치, 메시지가 방금 일어났는가?
4. 사용자가 그 흐름을 UI에서 따라갈 수 있는가?

### MVP 범위

- Runtime home 후보를 찾고 세션 파일 변화를 감시한다.
- 활성 세션을 선택하고 세션 이벤트 스트림을 만든다.
- 이벤트를 타임라인으로 보여준다.
- 파일/디렉터리 중심의 작업 맥락 그래프를 보여준다.
- MCP placeholder 이벤트를 함께 표시한다.

### MVP에서 제외

- Codex 행동 자동 제어
- 완전한 의미 기반 코드 분석
- 대형 저장소 전체 그래프 렌더링
- 외부 서비스 본문 저장
- 범용 멀티 에이전트 orchestration

## 핵심 사용자 흐름

### 1. 세션 발견

앱이 시작되면 Codex runtime home 후보를 찾고, 최근 활동 중인 세션을 추정한다.

### 2. 세션 추적

선택된 세션의 SQLite, rollout, history 변화를 따라가며 이벤트를 수집한다.

### 3. 맥락 시각화

이벤트를 타임라인과 코드 구조 뷰에 연결해, 지금 Codex가 어디를 보고 무엇을 바꾸는지 보여준다.

### 4. 사용자 확인

개발자는 세션, 파일, 명령, 변경 흔적을 빠르게 훑고 이상 징후를 조기에 발견한다.

## 아키텍처 방향

### 데이터 수집

- 기본 source of truth: runtime home 내부 저장소
- 우선 대상:
  - `state_5.sqlite`
  - `state_5.sqlite-wal`
  - `history.jsonl`
  - `sessions/**/rollout-*.jsonl`

### 앱 구성

- Desktop shell: Tauri 2
- UI: React + TypeScript
- 코드 인덱싱: Rust + tree-sitter

### UI 구성 원칙

- 왼쪽: 세션/계획/타임라인
- 가운데: 작업 맥락 그래프
- 오른쪽: 선택 이벤트 상세, 명령 출력, 변경 정보

## 단계별 로드맵

## Phase 1. Session Watch Foundation

목표: 저장소 변화를 안정적으로 감지하고 UI에 전달한다.

완료 기준:

- runtime home을 입력하면 watch plan을 계산할 수 있다.
- watch 시작/중지 상태를 앱에서 확인할 수 있다.
- 변경 이벤트가 프론트엔드까지 전달된다.

현재 상태:

- watch 대상 계획과 watcher 등록 뼈대가 구현되어 있다.

남은 일:

- runtime home discovery 전략 구체화
- watch 이벤트 debounce 및 오류 상태 정리
- 프론트엔드 연결과 디버그 패널 정리

## Phase 2. Session Ingestion

목표: SQLite, history, rollout을 읽어 UI 친화적인 세션 이벤트로 정규화한다.

완료 기준:

- 최근 세션 목록을 읽을 수 있다.
- 활성 세션을 선택할 수 있다.
- rollout append를 이어 읽을 수 있다.
- user/assistant/tool/patch 수준 이벤트를 내부 모델로 변환할 수 있다.

핵심 산출물:

- session index reader
- rollout tailer
- event normalizer
- active session scorer

## Phase 3. Timeline UI

목표: “방금 무슨 일이 일어났는지”를 읽기 쉬운 흐름으로 보여준다.

완료 기준:

- 세션별 이벤트 목록 표시
- user message, assistant message, tool call, tool output, patch 이벤트 구분
- 최신 이벤트 자동 갱신
- 오류/끊김/재시도 상태 표시

## Phase 4. Context Graph

목표: 이벤트를 파일/디렉터리 중심 구조와 연결해 작업 맥락을 시각화한다.

완료 기준:

- workspace / directory / file 수준 그래프 표시
- 이벤트 선택 시 관련 노드 하이라이트
- 사용자가 관심 파일이나 디렉터리를 pin할 수 있음

초기 원칙:

- symbol 단위보다 file/directory 단위를 먼저 완성한다.
- “전체 아키텍처 지도”보다 “현재 작업 주변부”를 우선한다.

## Phase 5. Agent-Vis Bridge and MCP Visual Events

목표: 에이전트가 편집 전후에 시각적으로 표현 가능한 작업 의도와 결과를 MCP로 보낼 수 있게 한다.

완료 기준:

- MCP visual event 또는 placeholder tool 호출을 받을 수 있다.
- 데스크톱 앱이 해당 이벤트를 ingest할 수 있다.
- 타임라인과 그래프에서 편집 전/후 marker, group, badge, edge와 근거 이벤트를 함께 표시한다.

예시 이벤트:

- 편집 전 변경 단위 시작 marker
- 편집 후 변경 단위 완료 marker
- 여러 파일을 묶는 group 또는 relationship edge
- 위험 영역 badge
- 사용자 판단이 필요한 decision marker
- 외부 컨텍스트 source badge

상세 설계:

- [Agent-Vis Bridge Design](AGENT_VIS_BRIDGE.md)

## Phase 6. Workspace Indexing

목표: 정적인 코드 구조를 생성해 세션 이벤트와 연결한다.

완료 기준:

- 디렉터리/파일 기본 그래프 생성
- 일부 언어에서 import/symbol 추출
- 증분 재인덱싱 전략 정리

언어 지원 우선순위:

1. TypeScript / JavaScript
2. Rust
3. Python
4. 나머지 언어는 공통 추출 수준으로 확장

## 작업 스트림

### A. Runtime Home Discovery

해야 할 일:

- `~/.codex` 기본 경로 지원
- 앱별 runtime home adapter 구조 정의
- workspace와의 관련성으로 후보 우선순위 계산

### B. Session Model

해야 할 일:

- thread / rollout / cwd / title / activity 상태 모델 고정
- event ID, dedupe, ordering 규칙 정리
- 끊긴 라인, truncate, stale session 처리

### C. Frontend UX

해야 할 일:

- 활성 세션 카드
- timeline virtualization 필요 여부 판단
- 그래프와 상세 패널 선택 상태 동기화

### D. Graph Indexing

해야 할 일:

- 파일 시스템 스캔
- ignore 규칙 정리
- tree-sitter parser registry 정리

### E. Agent-Vis Bridge

해야 할 일:

- focus signal 추출 규칙 정의
- visual event taxonomy 안정화
- desktop ingest endpoint 형태 결정
- change cluster와 evidence 모델 정의

## 우선순위

가장 먼저 끝내야 하는 순서는 아래와 같다.

1. watch 이벤트를 프론트엔드까지 안정적으로 연결
2. SQLite/rollout 기반 세션 목록과 활성 세션 판별
3. timeline UI
4. file/directory 중심 그래프
5. Agent-Vis Bridge와 MCP visual event 연결
6. symbol 수준 확장

## 리스크와 대응

### 저장소 스키마 변경

리스크:

- Codex 저장 포맷이나 SQLite schema가 바뀔 수 있다.

대응:

- 필요한 컬럼만 probe
- reader를 느슨하게 설계
- fallback 경로를 유지

### 런타임 위치 다양성

리스크:

- Codex CLI, Codex App, Orca가 서로 다른 runtime home을 쓸 수 있다.

대응:

- discovery 계층을 분리
- adapter 기반 후보 탐색
- 사용자 override 허용

### 과도한 시각화 복잡도

리스크:

- 초기에 너무 많은 노드와 이벤트를 한 화면에 올리면 가치보다 소음이 커진다.

대응:

- active context 중심 기본값
- detail level 단계화
- pin/filter/search 우선 제공

## 문서 운영 원칙

- 구현이 바뀌면 이 문서도 같이 갱신한다.
- 자세한 프로토콜, 스키마, 실험 메모는 별도 문서로 분리한다.
- 이 문서는 “무엇을 왜 어떤 순서로 만들지”를 설명하는 데 집중한다.

## 다음 업데이트 때 반영할 항목

- 실제 runtime home discovery 규칙 확정본
- 세션 정규화 이벤트 타입 초안
- 프론트엔드 패널 구조 스크린샷 또는 와이어프레임
- 완료된 phase 체크 상태
