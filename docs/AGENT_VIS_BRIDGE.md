# Agent-Vis Bridge Design

## 목적

Agent-Vis Bridge는 Codex 세션에서 발생하는 작업 흐름을 시각화 UI가 이해할 수 있는 이벤트로 바꾸는 계층이다. 이 bridge는 단일 입력에 의존하지 않는다. 정적 코드 그래프, 세션 텍스트와 툴 이벤트, 그리고 에이전트가 편집 전후에 MCP로 보내는 시각화 이벤트를 합쳐서 LLM의 코드 변경 흐름을 해석한다.

핵심 목표는 “어떤 파일이 바뀌었는가”를 넘어서 “에이전트가 지금 어떤 변경 단위를 시작했고, 편집 후 어떤 변경 단위를 끝냈는가”를 화면 위에 표현하는 것이다.

## 설계 원칙

- 세션 로그와 코드 그래프에서 안정적으로 복원 가능한 정보는 MCP로 보내지 않는다.
- MCP는 에이전트가 편집 전후에 남기는 시각화 가능한 의도와 결과만 담는다.
- MCP payload는 화면의 노드, 엣지, 영역, badge, timeline marker로 표현할 수 있어야 한다.
- Tree-sitter 그래프는 의미 판단 엔진이 아니라 코드 위치를 고정하는 좌표계로 사용한다.
- 세션 텍스트와 툴 이벤트는 focus, read/edit 흐름, patch 근거의 1급 데이터로 사용한다.
- UI에는 항상 판단의 근거 이벤트를 함께 전달한다.
- Bridge는 관찰자 계층이다. MVP에서는 Codex 행동을 제어하지 않는다.

## 입력 소스

### 1. Static Code Graph

정적 코드 그래프는 Tree-sitter 기반 인덱서가 만든다.

담당 정보:

- 파일과 디렉터리 구조
- top-level symbol
- import/export 관계
- patch hunk가 속한 symbol 또는 파일 영역
- 변경 파일 주변의 구조적 근접성

사용하지 않는 영역:

- LLM 의도 판단
- 기능 단위 확정
- 실제 런타임 영향 확정

Tree-sitter 그래프는 “어디를 바꿨는가”를 코드 구조 위에 앵커링하는 데 집중한다.

### 2. Session Text and Tool Events

세션 텍스트와 툴 이벤트는 runtime home 내부 세션 저장소에서 수집한다.

담당 정보:

- 현재 보고 있는 파일과 심볼
- 실제 수정한 파일
- 읽기, 검색, 수정 순서
- command 실행 기록
- patch 대상과 hunk
- assistant message 안의 자연어 작업 흐름
- 파일 mention과 반복 등장 빈도

특히 `focus`는 MCP가 아니라 이 계층에서 추출한다.

초기 focus 분류:

- `view_focus`: 최근 읽거나 검색한 파일/심볼
- `edit_focus`: patch나 write가 적용된 파일/심볼
- `context_focus`: 비교, 참고, 검색 결과로 잠깐 등장한 파일/심볼

### 3. MCP Visual Events

MCP는 실제 에이전트가 편집하거나 중요한 작업 경계를 넘기기 전후에 호출한다. 목적은 자연어 기록을 늘리는 것이 아니라, UI가 바로 그릴 수 있는 변경 의도와 결과를 남기는 것이다.

담당 정보:

- 편집 전에 시작되는 변경 단위
- 편집 후 실제로 완료된 변경 단위
- 여러 파일이나 노드가 한 작업 영역으로 묶이는 이유
- 위험하거나 주의가 필요한 노드/영역
- 사용자 판단이 필요한 분기점
- 외부 컨텍스트 사용을 나타내는 시각적 source marker

MCP에 넣지 않는 정보:

- 현재 focus 파일/심볼
- 읽은 파일 목록
- 수정한 파일 목록
- patch 대상 파일
- command 실행 기록
- 단순 파일 경로 mention
- import/dependency 관계
- UI에서 표현할 수 없는 장문 reasoning
- 사후 분석용 메모나 일반 설명

## Bridge 파이프라인

```text
Runtime Home
  ├─ state_5.sqlite
  ├─ history.jsonl
  └─ sessions/**/*.jsonl
          │
          ▼
Session Ingest
          │
          ├─ message/tool/patch events
          ├─ focus signals
          └─ file mentions
          │
          ▼
Agent-Vis Bridge ◄──── Tree-sitter Code Graph
          ▲
          │
MCP Visual Events
          │
          ▼
Visualization Events
          │
          ├─ Timeline
          ├─ Context Graph
          └─ Change Clusters
```

## 핵심 출력 모델

### FocusSignal

`FocusSignal`은 세션 텍스트와 툴 이벤트에서 추출한다.

```ts
type FocusSignal = {
  id: string;
  threadId: string;
  turnId?: string;
  kind: "view_focus" | "edit_focus" | "context_focus";
  path?: string;
  symbol?: string;
  source:
    | "tool_call"
    | "tool_output"
    | "patch"
    | "assistant_message"
    | "user_message";
  score: number;
  timestamp: number;
  evidence: string;
};
```

초기 스코어링 예시:

- patch 대상 파일: 높은 `edit_focus`
- command 인자에 반복 등장한 파일: 높은 `view_focus`
- 검색 결과에 한 번 등장한 파일: 낮은 `context_focus`
- assistant가 수정 대상으로 언급한 파일: 중간 또는 높은 `edit_focus`

### VisualAgentEvent

`VisualAgentEvent`는 MCP에서 들어오는 에이전트 선언 이벤트다. 에이전트는 편집 전에는 “이 변경 단위를 시작한다”는 이벤트를 보내고, 편집 후에는 “이 변경 단위가 이렇게 끝났다”는 이벤트를 보낸다.

```ts
type VisualAgentEvent = {
  id: string;
  threadId?: string;
  turnId?: string;
  phase: "before_edit" | "after_edit" | "checkpoint";
  kind:
    | "change_boundary"
    | "relationship"
    | "risk_marker"
    | "decision_marker"
    | "external_context_marker";
  label: string;
  visualTargetHints?: string[];
  visualStyle?: "highlight" | "group" | "badge" | "edge" | "timeline_marker";
  summary?: string;
  relatedHints?: string[];
  metadata?: Record<string, unknown>;
};
```

`visualTargetHints`와 `relatedHints`는 파일 목록의 source of truth가 아니다. Bridge가 세션 이벤트와 코드 그래프를 통해 최종 노드 매핑을 보강할 때 참고하는 힌트다.

### ChangeCluster

`ChangeCluster`는 사용자에게 보여줄 작업 단위다. 파일 이벤트를 그대로 나열하는 대신, 하나의 의도나 계획 단계에 묶일 수 있는 변경을 그룹화한다.

```ts
type ChangeCluster = {
  id: string;
  threadId: string;
  turnIds: string[];
  title: string;
  intent?: "bugfix" | "feature" | "refactor" | "cleanup" | "investigation";
  status: "forming" | "active" | "complete" | "stale";
  nodeIds: string[];
  focusSignalIds: string[];
  visualAgentEventIds: string[];
  evidenceEventIds: string[];
  summary?: string;
};
```

## Change Cluster 생성 규칙

초기 버전은 규칙 기반으로 시작한다.

강한 묶음 신호:

- 같은 turn 안에서 함께 patch된 파일
- 같은 MCP `change_boundary` 또는 `relationship` 근처에서 등장한 파일
- 같은 assistant message에서 수정 이유와 함께 언급된 파일
- patch hunk가 서로 import graph상 가까운 symbol에 속함

중간 묶음 신호:

- 같은 command sequence에서 반복적으로 읽힌 파일
- 같은 디렉터리 또는 feature 폴더에 속한 파일
- 테스트 파일과 구현 파일의 이름 또는 경로가 대응됨

약한 묶음 신호:

- 검색 결과에 함께 등장
- 같은 세션 안에서 멀리 떨어져 한 번씩 언급
- 단순 import 관계만 존재

## MCP Event Timing

MCP 호출은 에이전트 작업의 경계에 맞춘다.

### Before Edit

편집 전에 호출한다. UI는 이 이벤트를 timeline marker와 context graph의 pending group으로 표시할 수 있다.

예:

```json
{
  "phase": "before_edit",
  "kind": "change_boundary",
  "label": "Move graph indexing into Tauri Rust backend",
  "visualStyle": "group",
  "visualTargetHints": ["src-tauri/src/indexer"]
}
```

### After Edit

편집 후에 호출한다. UI는 이 이벤트를 completed group, changed area badge, timeline summary로 표시할 수 있다.

예:

```json
{
  "phase": "after_edit",
  "kind": "change_boundary",
  "label": "Rust tree-sitter indexer added",
  "visualStyle": "group",
  "visualTargetHints": ["src-tauri/src/indexer", "src-tauri/Cargo.toml"],
  "summary": "The TypeScript graph-indexer package was replaced by a Rust backend indexer."
}
```

### Checkpoint

편집이 없더라도 작업 경계가 바뀌거나 사용자의 결정이 필요한 순간에 호출한다. UI는 timeline marker, graph badge, decision marker로 표시한다.

예:

```json
{
  "phase": "checkpoint",
  "kind": "decision_marker",
  "label": "Use watchexec only for Codex session watch",
  "visualStyle": "timeline_marker"
}
```

## MCP Visual Taxonomy

MCP는 아래 이벤트부터 시작한다.

### `change_boundary`

하나의 변경 단위가 시작되거나 끝났음을 표시한다.

예:

```json
{
  "phase": "before_edit",
  "kind": "change_boundary",
  "label": "Add watchexec session watcher",
  "visualStyle": "group",
  "visualTargetHints": ["src-tauri/src/session_watch.rs", "src-tauri/src/lib.rs"]
}
```

### `relationship`

여러 노드가 같은 작업 영역으로 묶이는 이유를 시각적 edge나 group label로 표시한다.

예:

```json
{
  "phase": "after_edit",
  "kind": "relationship",
  "label": "Watcher command wiring",
  "visualStyle": "edge",
  "visualTargetHints": ["src-tauri/src/session_watch.rs", "src-tauri/src/lib.rs"],
  "summary": "The watcher implementation and Tauri command registration belong to the same feature."
}
```

### `risk_marker`

주의가 필요한 변경 지점을 badge나 highlighted area로 표시한다.

예:

```json
{
  "phase": "before_edit",
  "kind": "risk_marker",
  "label": "Watcher path dedupe may affect tests",
  "visualStyle": "badge",
  "visualTargetHints": ["src-tauri/src/session_watch.rs"]
}
```

### `decision_marker`

사용자 결정이나 방향 정정이 필요한 지점을 timeline marker로 표시한다.

예:

```json
{
  "phase": "checkpoint",
  "kind": "decision_marker",
  "label": "Watcher scope corrected to Codex session artifacts",
  "visualStyle": "timeline_marker"
}
```

### `external_context_marker`

외부 컨텍스트 사용을 source badge나 external node로 표시한다. 본문 저장은 하지 않는다.

예:

```json
{
  "phase": "checkpoint",
  "kind": "external_context_marker",
  "label": "Used project planning docs",
  "visualStyle": "badge",
  "visualTargetHints": ["docs/PLANS.md"]
}
```

## 시각화 매핑

Timeline:

- 원본 message/tool/patch 이벤트를 시간순으로 보여준다.
- MCP visual event는 편집 전/후 marker로 구분해 보여준다.
- ChangeCluster는 여러 이벤트 위에 얹히는 그룹 레이블로 표시한다.

Context Graph:

- `FocusSignal`은 노드 하이라이트 강도를 결정한다.
- `ChangeCluster`는 관련 노드를 하나의 작업 영역으로 묶는다.
- `VisualAgentEvent`는 그래프 노드의 group, edge, badge, marker로 표시한다.

Detail Panel:

- cluster summary
- 관련 파일/심볼
- 근거 이벤트 목록
- MCP visual event
- unresolved assumptions

## 구현 순서

1. 세션 이벤트에서 path, command, patch를 안정적으로 추출한다.
2. Tree-sitter 그래프에 patch hunk와 path mention을 매핑한다.
3. `FocusSignal` 모델과 스코어링을 구현한다.
4. MCP placeholder를 `VisualAgentEvent` taxonomy로 확장한다.
5. `ChangeCluster`를 규칙 기반으로 생성한다.
6. Timeline과 Context Graph에 cluster와 근거 이벤트를 표시한다.

## 열린 질문

- MCP visual event를 현재 placeholder tool 위에 얹을지, 별도 tool로 분리할지 결정해야 한다.
- `relatedHints`에 path-like string을 허용할지, graph node id만 허용할지 정해야 한다.
- session text parser가 assistant message를 얼마나 깊게 자연어 분석할지 정해야 한다.
- cluster title은 MCP label을 우선할지, session parser가 생성한 요약을 우선할지 정해야 한다.

## 현재 결정 사항

- focus는 MCP가 아니라 세션 텍스트와 툴 이벤트에서 추출한다.
- MCP는 편집 전후에 에이전트가 직접 남기는 시각화 가능한 의도와 결과만 담는다.
- Tree-sitter 그래프는 변경 위치와 구조 근접성을 제공하는 좌표계로 사용한다.
- 최종 UI 출력은 항상 evidence를 함께 가진다.
