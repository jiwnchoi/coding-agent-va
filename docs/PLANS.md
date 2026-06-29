# Codex 분석 세션 실시간 시각화 도구 개발 계획

## 1. 제품 목표

Codex가 코드베이스를 분석하고 수정하는 과정을 개발자가 실시간으로 이해하고 통제할 수 있게 해주는 데스크톱 시각화 도구를 만든다. 초기 버전은 Codex CLI나 Codex App을 사용자가 평소처럼 쓰는 흐름을 유지하면서, Codex runtime home에 저장되는 세션 기록과 MCP 기반 placeholder 신호를 코드 아키텍처 그래프 위에 시각화한다.

핵심 판단:

- 외부 JSON-RPC endpoint attach 전략은 MVP 기본 경로에서 제외한다.
- MVP의 1급 데이터 경로는 `CODEX_HOME` / `CODEX_SQLITE_HOME` 아래의 세션 저장소 watch이다.

### 핵심 가치

- **작업 맥락 가시화**: Codex가 어떤 파일, 폴더, 도구, 외부 컨텍스트를 보고 있는지 보여준다.
- **대화와 실행 연결**: Codex의 user/assistant message, command call, patch, tool call 기록을 실행 흐름과 연결한다.
- **평소 사용 방식 유지**: 사용자는 기존 Terminal, Codex CLI, Codex App, Orca 같은 환경을 계속 쓴다.
- **적응형 통제 단위**: 개발자와 코드베이스마다 다른 관심 단위를 파일, 폴더, 모듈, 기능 영역, 작업 단계로 전환 가능하게 한다.
- **문제 조기 발견**: 잘못된 방향의 분석, 과도한 컨텍스트 확장, 위험한 변경을 개발자가 빠르게 알아차리게 한다.
- **낮은 침습성**: 초기에는 Codex 동작을 제어하기보다 읽기 중심의 관찰자 UI로 시작한다.

## 2. 초기 범위

### 지원 대상

- 로컬 Codex CLI / Codex App / 앱 내장 Codex runtime 세션
- `CODEX_HOME` 아래의 rollout JSONL 세션 기록
- `CODEX_SQLITE_HOME` 또는 `CODEX_HOME` 아래의 `state_5.sqlite` 세션 인덱스
- `history.jsonl` 기반 사용자 입력 히스토리
- Orca처럼 앱이 별도 runtime home을 구성하는 환경
- MCP 서버를 통한 placeholder 시각화 이벤트
- 활성 Codex 세션 감지 후 자동 창 표시

### MVP에서 하지 않을 것

- 외부 JSON-RPC endpoint attach를 기본 동작으로 제공
- 모든 언어의 완전한 정적 분석
- 백만 줄 규모 코드베이스 전체 그래프 렌더링
- Codex 행동 자동 개입 또는 강제 제어
- 외부 앱 데이터의 본문 저장
- 다중 에이전트 일반화

## 3. 기술 선택

### 데스크톱 앱

**선택: Tauri 2 + React + TypeScript + Vite**

선택 이유:

- Tauri는 Rust 기반 네이티브 셸과 웹 프론트엔드를 결합해 가볍고 로컬 프로세스/파일 감시에 적합하다.
- runtime home discovery, SQLite read, JSONL tail, 파일 인덱싱, 창 자동 표시 같은 OS 인접 기능은 Tauri Rust 계층에서 처리하기 좋다.
- React/TypeScript는 그래프 UI, 실시간 이벤트 UI, 상태 모델링에 유리하다.
- Vite는 초기 개발 속도와 단순한 번들링에 적합하다.

참고:

- Tauri 공식 문서: https://v2.tauri.app/start/frontend/
- Tauri IPC 문서: https://v2.tauri.app/concept/inter-process-communication/

### 시각화

**선택: React Flow**

선택 이유:

- 노드/엣지 기반 인터랙티브 그래프 UI를 빠르게 구성할 수 있다.
- 코드 아키텍처 그래프, 부분 그래프, 이벤트 하이라이트를 같은 모델로 표현하기 쉽다.
- 커스텀 노드와 엣지를 통해 파일, 폴더, 모듈, MCP 리소스, 계획 단계를 각각 다른 형태로 렌더링할 수 있다.

참고:

- React Flow Learn: https://reactflow.dev/learn
- React Flow Terms: https://reactflow.dev/learn/concepts/terms-and-definitions

### 앱 상태 관리

**선택: Zustand + 명시적 이벤트 리듀서**

선택 이유:

- Codex 저장 이벤트는 append-only에 가깝고 순서가 중요하므로, 원본 수집과 UI 상태 반영을 분리해야 한다.
- Zustand는 가볍고 React Flow와 함께 쓰기 쉽다.
- 이벤트 리듀서를 별도로 두면 나중에 이벤트 재생, 세션 리플레이, 디버깅을 붙이기 쉽다.

### Codex 세션 저장소 통합

**선택: Tauri Rust backend에서 runtime home discovery + SQLite/JSONL watch**

초기 연결 전략:

1. 로컬 환경에서 Codex runtime home 후보를 찾는다.
2. 각 후보에서 `state_5.sqlite`, `history.jsonl`, `sessions/YYYY/MM/DD/rollout-*.jsonl`을 확인한다.
3. `state_5.sqlite`의 `threads.rollout_path`를 세션의 source of truth로 사용한다.
4. `rollout_path`가 존재하면 해당 JSONL을 tail하며 새 line을 내부 `SessionEvent`로 normalize한다.
5. `state_5.sqlite`가 없거나 stale이면 `sessions/**/rollout-*.jsonl` glob으로 fallback한다.
6. `history.jsonl`은 빠른 사용자 입력 감지와 세션 후보 보조 정보로만 사용한다.
7. 앱별 terminal history나 hook 파일은 optional adapter로 둔다.

이 전략은 “이미 실행 중인 프로세스에 붙는다”가 아니라 “이미 실행 중인 Codex가 쓰고 있는 저장소를 따라간다”는 모델이다.

## 4. Codex Runtime Home

### 용어

- **`CODEX_HOME`**: Codex가 config, history, session rollout, 일부 SQLite 파일을 저장하는 기본 home.
- **`CODEX_SQLITE_HOME`**: Codex SQLite DB를 별도 위치에 둘 때 사용하는 home. 없으면 보통 `CODEX_HOME`을 따른다.
- **runtime home**: 제품 내부에서 `CODEX_HOME`과 `CODEX_SQLITE_HOME`을 합쳐 부르는 관찰 단위.
- **rollout**: 한 Codex thread/session의 append-only JSONL 기록.
- **thread id / session id**: 대화 세션 식별자. rollout filename과 `threads.id`에 들어간다.

### 기본 위치

일반 Codex CLI는 보통 다음 위치를 쓴다.

```text
~/.codex/
  history.jsonl
  state_5.sqlite
  logs_2.sqlite
  sessions/YYYY/MM/DD/rollout-YYYY-MM-DDTHH-MM-SS-<thread-id>.jsonl
```

하지만 앱이 Codex runtime을 내장하거나 sandbox하면 별도 위치를 쓸 수 있다. 예를 들어 Orca 환경에서는 다음과 같은 위치가 확인되었다.

```text
~/Library/Application Support/orca/codex-runtime-home/home/
  history.jsonl
  state_5.sqlite
  logs_2.sqlite
  sessions/YYYY/MM/DD/rollout-YYYY-MM-DDTHH-MM-SS-<thread-id>.jsonl

~/Library/Application Support/orca/
  terminal-history/<worktree-id@@terminal-id>/checkpoint.json
  terminal-history/<worktree-id@@terminal-id>/output.log
  agent-hooks/last-status.json
```

따라서 구현은 `~/.codex`를 하드코딩하면 안 된다. 반드시 runtime home discovery 계층을 둔다.

### Runtime Home Discovery 우선순위

1. **명시 설정**
   - 사용자가 앱 설정에서 추가한 runtime home.
   - shell installer가 기록한 runtime home.
   - wrapper가 넘긴 `CODEX_HOME`, `CODEX_SQLITE_HOME`.

2. **프로세스 환경**
   - 실행 중인 `codex` 프로세스의 argv와 environment를 읽는다.
   - 가능한 경우 `CODEX_HOME`, `CODEX_SQLITE_HOME`, cwd, parent terminal을 추출한다.
   - macOS에서는 권한/보안 정책 때문에 모든 프로세스 env를 읽지 못할 수 있으므로 실패 가능성을 정상 흐름으로 다룬다.

3. **앱별 adapter**
   - Orca: `~/Library/Application Support/orca/codex-runtime-home/home`
   - Codex desktop app이 별도 app support directory를 쓰는 경우 추가 adapter로 확장한다.
   - adapter는 “후보 디렉터리 탐색 + marker 파일 검증”만 담당한다.

4. **기본 fallback**
   - `~/.codex`
   - `~/.codex/sqlite` 같은 과거/변형 위치

후보 검증 기준:

- `state_5.sqlite` 또는 `sessions/`가 존재한다.
- `history.jsonl`이 존재하면 신뢰도를 높인다.
- `sessions/**/rollout-*.jsonl` 중 최근 수정 파일이 있으면 활성 후보로 올린다.
- `state_5.sqlite`의 `threads.cwd`가 현재 workspace root와 맞으면 우선순위를 높인다.

## 5. 대화 기록 스키마

### `state_5.sqlite`

`state_5.sqlite`는 세션 인덱스다. MVP에서는 `threads` 테이블만 읽는다.

중요 컬럼:

```sql
CREATE TABLE threads (
  id TEXT PRIMARY KEY,
  rollout_path TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  source TEXT NOT NULL,
  model_provider TEXT NOT NULL,
  cwd TEXT NOT NULL,
  title TEXT NOT NULL,
  sandbox_policy TEXT NOT NULL,
  approval_mode TEXT NOT NULL,
  tokens_used INTEGER NOT NULL DEFAULT 0,
  has_user_event INTEGER NOT NULL DEFAULT 0,
  archived INTEGER NOT NULL DEFAULT 0,
  git_sha TEXT,
  git_branch TEXT,
  git_origin_url TEXT,
  cli_version TEXT NOT NULL DEFAULT '',
  first_user_message TEXT NOT NULL DEFAULT '',
  model TEXT,
  reasoning_effort TEXT,
  thread_source TEXT,
  preview TEXT NOT NULL DEFAULT '',
  recency_at INTEGER NOT NULL DEFAULT 0,
  recency_at_ms INTEGER NOT NULL DEFAULT 0
);
```

사용 방식:

- `id`: 내부 세션 id. rollout filename의 마지막 id와 대체로 일치한다.
- `rollout_path`: 실제 대화 JSONL 경로. 가장 중요한 연결점.
- `cwd`: 사용자가 Codex를 실행한 workspace.
- `title`: 첫 사용자 입력에서 생성된 세션 제목.
- `updated_at`, `updated_at_ms`, `recency_at_ms`: 최근 세션 정렬과 활성 감지에 사용.
- `archived`: archived 세션은 기본 목록에서 숨긴다.
- `git_branch`, `git_sha`, `git_origin_url`: repo graph와 세션 연결에 사용.

읽기 쿼리 예시:

```sql
SELECT
  id,
  rollout_path,
  cwd,
  title,
  updated_at_ms,
  recency_at_ms,
  git_branch,
  git_sha
FROM threads
WHERE archived = 0
ORDER BY recency_at_ms DESC, updated_at_ms DESC
LIMIT 50;
```

주의:

- SQLite WAL 모드일 수 있으므로 `state_5.sqlite-wal` 수정도 watch해야 한다.
- DB 파일을 직접 장시간 lock하지 않는다.
- read-only connection을 짧게 열고 닫는다.
- schema version은 바뀔 수 있으므로 필요한 컬럼 존재 여부를 시작 시 probe한다.

### Rollout JSONL

rollout 파일은 append-only JSONL이다.

경로 패턴:

```text
$CODEX_HOME/sessions/YYYY/MM/DD/rollout-YYYY-MM-DDTHH-MM-SS-<thread-id>.jsonl
```

line 기본 구조:

```ts
type RolloutLine = {
  timestamp: string;
  type:
    | "session_meta"
    | "turn_context"
    | "world_state"
    | "response_item"
    | "event_msg"
    | "compacted";
  payload: Record<string, unknown>;
};
```

대표 payload:

```ts
type SessionMetaPayload = {
  id: string;
  timestamp: string;
  cwd?: string;
  cli_version?: string;
  originator?: string;
  source?: string;
  thread_source?: string;
  model_provider?: string;
  git?: {
    branch?: string;
    commit_hash?: string;
    repository_url?: string;
  };
};

type TurnContextPayload = {
  turn_id: string;
  cwd: string;
  model?: string;
  effort?: string;
  approval_policy?: string;
  sandbox_policy?: unknown;
  workspace_roots?: string[];
  timezone?: string;
  current_date?: string;
  summary?: string;
};

type EventMsgPayload =
  | { type: "user_message"; message: string; images?: unknown[] }
  | { type: "agent_message"; message: string; phase?: "analysis" | "commentary" | "final" }
  | { type: "task_started" }
  | { type: "task_complete"; last_agent_message?: string }
  | { type: "patch_apply_end"; success: boolean; stdout?: string; stderr?: string }
  | { type: "token_count"; info: unknown }
  | { type: "context_compacted"; message?: string }
  | { type: string; [key: string]: unknown };

type ResponseItemPayload =
  | {
      type: "message";
      role: "user" | "assistant" | "system";
      content: Array<{ type: string; text?: string; [key: string]: unknown }>;
      phase?: "analysis" | "commentary" | "final";
      internal_chat_message_metadata_passthrough?: { turn_id?: string };
    }
  | {
      type: "function_call" | "custom_tool_call";
      name?: string;
      call_id: string;
      arguments?: string;
      input?: string;
      internal_chat_message_metadata_passthrough?: { turn_id?: string };
    }
  | {
      type: "function_call_output" | "custom_tool_call_output";
      call_id: string;
      output?: string;
      result?: unknown;
      internal_chat_message_metadata_passthrough?: { turn_id?: string };
    }
  | {
      type: "reasoning";
      summary?: unknown[];
      encrypted_content?: string;
      internal_chat_message_metadata_passthrough?: { turn_id?: string };
    }
  | { type: string; [key: string]: unknown };
```

저장되는 것:

- user message / assistant message
- commentary/final assistant output
- shell/function/custom tool call과 output
- patch apply 결과
- token count, compacted marker
- 일부 turn context와 workspace metadata

저장되지 않거나 제한적인 것:

- reasoning 원문은 보통 암호화되거나 요약만 저장된다.
- streaming delta는 최종 message/output으로 합쳐져 저장되는 경우가 많다.
- 실시간 UI 전용 이벤트는 rollout에 남지 않을 수 있다.
- terminal alternate screen의 시각 상태는 rollout이 아니라 앱별 terminal history에 남을 수 있다.

### `history.jsonl`

사용자 입력 히스토리다.

```ts
type HistoryLine = {
  session_id: string;
  ts: number;
  text: string;
};
```

사용 방식:

- 새 user prompt가 들어왔는지 빠르게 감지한다.
- `session_id`로 rollout/state row와 연결한다.
- UI에서 세션 제목이 비어 있을 때 fallback title로 사용한다.

주의:

- assistant 응답이나 tool output은 여기에 없다.
- 완전한 대화 source of truth로 쓰면 안 된다.

### `logs_2.sqlite`

runtime log DB다. MVP의 대화 source로 사용하지 않는다.

용도:

- 디버깅용 runtime 로그
- 오류/경고 이벤트 보조 표시

비권장:

- 세션 목록 source
- 대화 내용 source
- 활성 turn 판단 source

### 앱별 terminal history

Orca 같은 앱은 별도의 terminal capture를 저장할 수 있다.

예:

```text
~/Library/Application Support/orca/terminal-history/.../checkpoint.json
~/Library/Application Support/orca/terminal-history/.../output.log
```

사용 방식:

- 사용자가 실제로 본 terminal screen을 복원하는 optional adapter.
- Codex TUI의 진행 상태, 경고, 화면 텍스트를 보조적으로 표시.

주의:

- ANSI escape sequence와 private UI state가 섞인다.
- 대화 스키마가 아니라 terminal rendering artifact다.
- 개인정보 노출 범위가 넓으므로 기본 저장/업로드 금지.

## 6. Watch 전략

### Watch 대상

각 runtime home마다 다음 파일/디렉터리를 watch한다.

```text
$CODEX_SQLITE_HOME/state_5.sqlite
$CODEX_SQLITE_HOME/state_5.sqlite-wal
$CODEX_HOME/history.jsonl
$CODEX_HOME/sessions/
$CODEX_HOME/sessions/YYYY/MM/DD/rollout-*.jsonl
```

앱별 adapter가 있으면 추가한다.

```text
~/Library/Application Support/orca/agent-hooks/last-status.json
~/Library/Application Support/orca/terminal-history/
```

### 세션 발견 알고리즘

1. runtime home 후보를 discovery한다.
2. 각 후보의 `state_5.sqlite`를 읽어 최근 `threads` row를 가져온다.
3. `threads.rollout_path`가 존재하는 row를 `SessionCandidate`로 만든다.
4. 파일이 실제로 존재하지 않으면 `sessions/**/rollout-*.jsonl` glob fallback으로 찾는다.
5. `history.jsonl`의 최근 `session_id`가 DB에 없으면 orphan candidate로 등록한다.
6. 같은 `threadId`가 여러 runtime home에서 발견되면 최근 `rollout_path` mtime이 큰 쪽을 선택한다.
7. `cwd`가 현재 열려 있는 workspace root와 일치하거나 하위 경로면 우선순위를 높인다.

```ts
type RuntimeHome = {
  codexHome: string;
  sqliteHome: string;
  source: "user_config" | "process_env" | "app_adapter" | "default";
  confidence: number;
};

type SessionCandidate = {
  threadId: string;
  runtimeHome: RuntimeHome;
  rolloutPath: string;
  cwd?: string;
  title?: string;
  updatedAtMs?: number;
  recencyAtMs?: number;
  source: "state_db" | "history" | "glob" | "app_adapter";
};
```

### Rollout tail 알고리즘

1. 세션 후보가 선택되면 rollout 파일 offset을 0 또는 마지막 저장 offset으로 초기화한다.
2. 초기 load에서는 전체 JSONL을 읽고 normalize한다.
3. 이후 파일 수정 이벤트가 오면 마지막 offset부터 새 bytes만 읽는다.
4. 마지막 line이 newline으로 끝나지 않으면 buffer에 보관하고 다음 이벤트에서 이어 붙인다.
5. JSON parse 실패는 즉시 fatal로 보지 않고 incomplete write 가능성으로 debounce한다.
6. 파일 size가 offset보다 작아지면 rotation/truncate로 보고 offset을 0으로 재설정한다.
7. normalize한 이벤트는 `threadId + lineNumber` 또는 `threadId + timestamp + hash`로 dedupe한다.

```ts
type RolloutTailState = {
  path: string;
  threadId: string;
  offset: number;
  lineNumber: number;
  partialLine: string;
  lastMtimeMs: number;
};
```

### SQLite watch 알고리즘

1. `state_5.sqlite`와 `state_5.sqlite-wal` 수정 이벤트를 모두 감지한다.
2. 이벤트를 200-500ms debounce한다.
3. read-only connection으로 최근 thread 목록만 짧게 조회한다.
4. 신규 `rollout_path`가 생기면 tailer를 시작한다.
5. 기존 row의 `updated_at_ms`나 `recency_at_ms`가 바뀌면 활성 후보 점수를 갱신한다.
6. DB lock이나 busy 오류는 재시도하고, 실패 상태를 UI에 낮은 심각도로 표시한다.

### 활성 세션 판단

MVP의 활성 세션은 다음 점수로 판단한다.

- rollout mtime이 최근 2분 이내: +5
- `state_5.sqlite.threads.recency_at_ms`가 최근 2분 이내: +4
- `history.jsonl`에 같은 `session_id`의 최근 user prompt가 있음: +3
- rollout에 `task_started` 후 아직 `task_complete`가 없음: +3
- cwd가 현재 workspace와 일치: +2
- terminal/process watcher에서 `codex` 프로세스가 같은 cwd로 실행 중: +2
- archived thread: -10

가장 높은 점수의 세션을 primary active session으로 표시한다. 동점이면 최근 rollout mtime을 우선한다.

### Normalize 전략

원본 rollout line은 그대로 내부 store에 넣지 않는다. 먼저 UI 친화 이벤트로 변환한다.

```ts
type SessionEvent = {
  id: string;
  threadId: string;
  turnId?: string;
  timestamp: number;
  source: "rollout" | "history" | "mcp_placeholder" | "terminal_history";
  kind:
    | "session_meta"
    | "turn_context"
    | "user_message"
    | "agent_message"
    | "tool_call"
    | "tool_output"
    | "command_executed"
    | "patch_applied"
    | "file_mentioned"
    | "task_started"
    | "task_completed"
    | "context_compacted"
    | "placeholder";
  title: string;
  nodeIds: string[];
  payload: Record<string, unknown>;
};
```

파일 경로 추출 규칙:

- shell command arguments에서 repo-relative path 후보를 추출한다.
- `function_call_output` 안의 command output에서 `Read <file>`, `Edited <file>`, `Update File` 같은 known pattern을 추출한다.
- patch apply 결과와 `apply_patch` input에서 변경 파일을 추출한다.
- user/assistant message의 inline path는 약한 신호로만 사용한다.
- 존재하지 않는 path는 바로 graph node로 만들지 않고 unresolved mention으로 둔다.

## 7. MCP 통합

**선택: 별도 TypeScript MCP 서버 패키지**

MCP 서버의 역할 구분:

- **MCP Visualization Server**: Codex가 MCP tool로 호출하는 외부 tool server이다. `visualization.placeholder` 같은 tool schema를 노출하고, Codex 세션 안에서 “이 지점을 시각화해 달라”는 명시적 신호를 받는다.
- **Desktop Ingest Endpoint**: 데스크톱 앱이 MCP 서버로부터 placeholder payload를 전달받는 로컬 수신부이다. MCP 서버가 아니며, Tauri backend 안의 localhost HTTP/WebSocket/unix socket endpoint 또는 sidecar stdout bridge로 구현한다.
- **Storage Watcher**: Codex 대화/실행 기록을 `CODEX_HOME`에서 읽는 관찰자이다. MCP 서버와 별도이며, 네트워크 서버가 아니다.

초기 목적:

- Codex 세션에서 시각화 도구로 placeholder 이벤트를 보낼 수 있는 통로를 만든다.
- MCP 서버가 받은 tool 호출 payload를 Desktop Ingest Endpoint로 forward한다.
- rollout watcher가 자동으로 알 수 없는 고수준 의도, 리스크, focus 정보를 Codex가 자발적으로 보낼 수 있게 한다.
- 실제 payload grammar는 나중에 정하고, MVP에서는 고정 schema의 `visualization.placeholder` tool만 제공한다.

초기 MCP tool 예시:

```json
{
  "name": "visualization.placeholder",
  "arguments": {
    "threadId": "019f1388-8d3f-7030-9166-02a1efcf7430",
    "turnId": "019f1388-ba35-7aa2-9e0e-244304d94106",
    "label": "Agent inspected architecture context",
    "kind": "context_focus",
    "metadata": {
      "path": "docs/PLANS.md"
    }
  }
}
```

참고:

- MCP 공식 SDK 목록: https://modelcontextprotocol.io/docs/sdk
- MCP TypeScript SDK: https://github.com/modelcontextprotocol/typescript-sdk

## 8. Repo Scaffold 계획

```text
.
├── apps/
│   └── desktop/
│       ├── src/
│       │   ├── app/
│       │   ├── components/
│       │   ├── features/
│       │   │   ├── architecture-graph/
│       │   │   ├── codex-session/
│       │   │   ├── event-timeline/
│       │   │   └── plan-tracker/
│       │   ├── stores/
│       │   └── types/
│       ├── src-tauri/
│       │   ├── src/
│       │   │   ├── runtime_home.rs
│       │   │   ├── session_index.rs
│       │   │   ├── rollout_tailer.rs
│       │   │   ├── session_detector.rs
│       │   │   ├── event_bus.rs
│       │   │   └── main.rs
│       │   └── tauri.conf.json
│       ├── index.html
│       ├── package.json
│       └── vite.config.ts
├── packages/
│   ├── codex-protocol/
│   │   └── src/
│   │       ├── rollout_schema.ts
│   │       ├── session_events.ts
│   │       ├── visualization_events.ts
│   │       └── index.ts
│   ├── mcp-visualization-server/
│   │   └── src/
│   │       ├── server.ts
│   │       └── placeholder_tool.ts
│   └── graph-indexer/
│       └── src/
│           ├── filesystem_index.ts
│           ├── dependency_index.ts
│           └── graph_model.ts
├── docs/
│   ├── PLANS.md
│   ├── ARCHITECTURE.md
│   └── EVENT_GRAMMAR.md
├── package.json
├── pnpm-workspace.yaml
└── turbo.json
```

### 패키지 역할

- `app`: 실제 Tauri 데스크톱 앱과 React UI
- `packages/codex-protocol`: rollout line, 내부 이벤트, 시각화 이벤트 타입 정의
- `packages/mcp-visualization-server`: Codex가 호출할 MCP 서버, 초기에는 placeholder만 제공
- `packages/graph-indexer`: 파일 트리와 의존관계를 부분 그래프로 변환하는 인덱서

## 9. 시스템 아키텍처

```text
Codex CLI / Codex App / Orca-managed Codex
        │
        │ writes session state
        ▼
Runtime Home Discovery
        │
        ├── state_5.sqlite ───────► Session Index Reader
        ├── history.jsonl ────────► Prompt History Reader
        └── sessions/**/*.jsonl ──► Rollout Tailer
                                  │
                                  │ normalized events
                                  ▼
Tauri Rust Backend ───────────► Event Bus
        │                         │
        │                         ▼
        │                  Frontend Event Store ───► Timeline
        │                         │                 Plan Tracker
        │                         │                 Diff / Output Panel
        │                         ▼
        │                  Architecture Graph View
        │
        ▲ desktop ingest endpoint
        │
MCP Visualization Server
        ▲
        │ MCP tool call
        │
Codex Session
```

### 주요 데이터 흐름

1. 데스크톱 앱이 시작되면 runtime home 후보를 탐색한다.
2. `state_5.sqlite`에서 최근 thread와 `rollout_path`를 읽는다.
3. `history.jsonl`로 최근 user prompt와 orphan session 후보를 보강한다.
4. 활성 세션 후보의 rollout JSONL을 tail한다.
5. rollout line을 내부 `SessionEvent`로 normalize한다.
6. 그래프 인덱서가 현재 repo의 파일/폴더 구조를 기본 그래프로 만든다.
7. MCP Visualization Server가 받은 placeholder tool call을 Desktop Ingest Endpoint로 전달한다.
8. rollout 이벤트와 MCP placeholder 이벤트가 관련 노드와 엣지를 하이라이트한다.
9. 사용자는 파일/폴더/모듈/계획 단위로 그래프 detail level을 바꾼다.

## 10. 핵심 도메인 모델

### Session

```ts
type Session = {
  threadId: string;
  runtimeHome: RuntimeHome;
  rolloutPath: string;
  cwd?: string;
  title?: string;
  status: "unknown" | "idle" | "active" | "completed" | "stale" | "error";
  activeTurnId?: string;
  lastEventAt?: number;
};
```

### PlanStep

rollout에는 structured plan update가 항상 저장된다고 보장할 수 없다. MVP에서는 assistant message에서 plan-like text를 추출하거나 MCP placeholder가 명시적으로 보낸 plan metadata를 사용한다.

```ts
type PlanStep = {
  id: string;
  turnId?: string;
  text: string;
  status: "pending" | "inProgress" | "completed" | "unknown";
  relatedNodeIds: string[];
  source: "assistant_message" | "mcp_placeholder" | "manual";
};
```

### ArchitectureNode

```ts
type ArchitectureNode = {
  id: string;
  kind: "repo" | "directory" | "file" | "symbol" | "external" | "plan";
  label: string;
  path?: string;
  metadata?: Record<string, unknown>;
};
```

### VisualizationEvent

```ts
type VisualizationEvent = {
  id: string;
  source: "rollout" | "history" | "mcp_placeholder" | "graph_indexer" | "terminal_history";
  threadId?: string;
  turnId?: string;
  kind:
    | "session_detected"
    | "plan_updated"
    | "context_focus"
    | "file_read"
    | "file_changed"
    | "command_executed"
    | "external_context_used"
    | "placeholder";
  title: string;
  nodeIds: string[];
  timestamp: number;
  payload: Record<string, unknown>;
};
```

## 11. 그래프 시각화 전략

### Level of Detail

대규모 코드베이스 전체를 한 번에 보여주지 않는다. 기본 원칙은 “Codex가 현재 보고 있거나 바꾸는 영역 + 주변 맥락”만 보여주는 것이다.

초기 detail level:

1. **Workspace**: repo root와 상위 디렉터리
2. **Directory**: 활성 파일 주변 폴더와 sibling 구조
3. **File**: Codex가 읽거나 수정한 파일
4. **Symbol**: 이후 정적 분석이 준비되면 함수/클래스/컴포넌트 단위
5. **Plan**: 계획 단계와 관련 파일/명령/변경 연결

### 관심 영역 계산

초기 MVP는 다음 신호로 관심 영역을 정한다.

- `function_call`의 command arguments에 포함된 파일 경로
- `function_call_output`에 포함된 `Read`, `Edited`, `Update File`, `Delete File`, `Add File` 경로
- `patch_apply_end`와 apply patch input에 포함된 변경 파일
- `turn_context.cwd`와 `workspace_roots`
- user/assistant message의 repo-relative path mention
- MCP placeholder event의 `metadata.path` 또는 `metadata.nodeIds`
- 사용자가 직접 pin한 노드

### 외부 컨텍스트 표현

Slack, Notion, Web, MCP 리소스는 본문을 저장하지 않고 “어떤 source에서 어떤 종류의 context를 가져왔다” 수준으로 표시한다.

예시:

```ts
type ExternalContextNode = {
  id: string;
  kind: "external";
  provider: "slack" | "notion" | "web" | "mcp" | "unknown";
  label: string;
  redacted: true;
};
```

## 12. Codex 감지 및 자동 창 표시

### 감지 방식

목표는 사용자가 이미 Codex CLI나 Codex App으로 작업 중인 세션을 감지해 observer UI를 띄우는 것이다. 데스크톱 앱이 새 Codex 세션을 만드는 것은 MVP의 기본 흐름이 아니다.

1. 앱 실행 시 runtime home 후보를 discovery한다.
2. 각 runtime home에서 `state_5.sqlite`와 `sessions/`를 확인한다.
3. 최근 thread의 `rollout_path`, `cwd`, `title`, `updated_at_ms`를 읽는다.
4. rollout file mtime과 최근 line을 확인한다.
5. 활성 점수 알고리즘으로 primary active session을 고른다.
6. active session이 있고 자동 표시 설정이 켜져 있으면 main window를 show/focus한다.
7. 이후 SQLite/JSONL 변경을 watch하며 session list와 timeline을 갱신한다.

### 창 표시 정책

- 활성 세션 감지 시 hidden 상태의 main window를 show/focus한다.
- 사용자가 “자동 표시 끄기”를 설정하면 background tray mode로 유지한다.
- 너무 잦은 focus steal을 막기 위해 session당 최초 1회만 자동 focus한다.
- terminal history만 감지되고 rollout이 없으면 focus하지 않고 tray notification만 표시한다.

## 13. UI 구성

### Main Layout

```text
┌──────────────────────────────────────────────────────────────┐
│ Top Bar: active session, cwd, runtime home, watch state        │
├───────────────┬──────────────────────────────┬───────────────┤
│ Plan Tracker  │ Architecture Graph           │ Event Detail   │
│ Timeline      │                              │ Diff / Output  │
└───────────────┴──────────────────────────────┴───────────────┘
```

### 주요 패널

- **Architecture Graph**: 코드 구조와 현재 관심 영역 하이라이트
- **Plan Tracker**: assistant message 또는 MCP placeholder에서 추출한 계획 단계 표시
- **Event Timeline**: message/tool/command/file/MCP 이벤트 순서 표시
- **Diff Panel**: patch와 command output 기반 변경 사항 표시
- **Context Panel**: 외부 MCP/context 사용 여부 표시
- **Runtime Panel**: 감지한 `CODEX_HOME`, DB, rollout path, watch 상태 표시

## 14. 이벤트 Grammar 초기안

MVP에서는 rollout watcher 이벤트를 직접 UI에 매핑하고, MCP는 placeholder만 보낸다. 이후 Codex가 명시적으로 사고/작업 상태를 시각화하기 위한 grammar를 정의한다.

### MVP placeholder

```ts
type PlaceholderVisualizationInput = {
  threadId?: string;
  turnId?: string;
  label: string;
  kind?: "context_focus" | "plan_link" | "external_context" | "risk" | "note";
  nodeIds?: string[];
  metadata?: Record<string, unknown>;
};
```

### 이후 확장 후보

- `focus_nodes`: 특정 코드 영역에 attention 부여
- `link_plan_to_code`: 계획 단계와 코드 노드 연결
- `mark_risk`: 위험하거나 불확실한 영역 표시
- `summarize_subgraph`: 부분 그래프 설명 생성
- `external_context_used`: 외부 도구/context 사용 기록
- `decision_point`: 개발자 판단이 필요한 지점 표시

## 15. 개발 마일스톤

### Milestone 0: 문서와 스캐폴드

- `docs/PLANS.md` 작성
- pnpm workspace 구성
- Tauri + React + TypeScript 앱 생성
- 기본 lint/typecheck/build 스크립트 구성
- 빈 shell UI 실행 확인

### Milestone 1: Runtime Home Discovery

- `~/.codex` 기본 후보 탐색
- Orca runtime home adapter 구현
- user-configured runtime home 추가 UI
- `CODEX_HOME` / `CODEX_SQLITE_HOME` 분리 모델 구현
- runtime home confidence scoring 구현

### Milestone 2: Session Index Reader

- `state_5.sqlite` read-only query 구현
- `threads` schema probe 구현
- 최근 session list UI 표시
- `rollout_path`, `cwd`, `title`, `updated_at_ms` 매핑
- WAL 수정 이벤트 debounce 구현

### Milestone 3: Rollout Tailer

- JSONL 전체 load와 incremental tail 구현
- partial line buffer 처리
- truncate/rotation 처리
- rollout line parser와 `SessionEvent` normalize 구현
- history offset 저장과 replay 지원

### Milestone 4: 세션 감지와 자동 창 표시

- 활성 세션 scoring 구현
- 자동 show/focus 정책 구현
- 사용자 설정으로 자동 표시 on/off 지원
- 최근 active session selector 구현
- Runtime Panel에 watch 상태 표시

### Milestone 5: 이벤트 타임라인

- user/agent message 표시
- function/custom tool call 표시
- command output 요약 표시
- patch apply 결과 표시
- event detail drawer 구현

### Milestone 6: 계획/파일 신호 추출

- assistant message에서 plan-like block 추출
- apply patch input/output에서 변경 파일 추출
- command arguments에서 file mention 추출
- path resolver와 unresolved mention 모델 구현
- plan step과 이벤트 timeline 연결

### Milestone 7: 기본 코드 그래프

- repo file tree indexer 구현
- directory/file 노드 렌더링
- 읽은 파일과 변경 파일 하이라이트
- 관심 영역만 렌더링하는 subgraph projection 구현

### Milestone 8: MCP placeholder 서버

- TypeScript MCP server 패키지 생성
- `visualization.placeholder` tool 구현
- Desktop Ingest Endpoint 구현
- tool 호출 payload를 Desktop Ingest Endpoint로 forward
- placeholder 이벤트를 그래프/timeline에 표시

### Milestone 9: Adaptive control 단위

- Workspace / Directory / File / Plan detail level 전환
- 사용자 pin/focus 노드 지원
- 코드베이스 규모에 따른 노드 축약 규칙 구현
- session별 보기 설정 저장

## 16. 기술 리스크와 대응

### Runtime home 다양성

- `~/.codex`만 가정하지 않는다.
- discovery adapter를 확장 가능하게 만든다.
- UI에서 현재 감지한 runtime home과 신뢰도를 노출한다.

### SQLite schema 변화

- 필요한 컬럼을 시작 시 probe한다.
- 필수 컬럼이 없으면 glob fallback으로 동작한다.
- DB read 실패는 watch 전체 실패가 아니라 해당 source degrade로 처리한다.

### JSONL schema 변화

- `type`과 `payload.type`을 모두 unknown-safe하게 파싱한다.
- 모르는 event는 raw event로 timeline에 표시하되 graph 반영은 하지 않는다.
- parser fixture를 실제 rollout 샘플로 유지한다.

### 실시간성 한계

- rollout은 저장 시점 기준이다. streaming delta 수준의 완전한 실시간 UI는 보장하지 않는다.
- MVP는 “수 초 이내 near-real-time”을 목표로 한다.
- 더 빠른 신호가 필요하면 shell integration 또는 MCP placeholder를 사용한다.

### terminal history 파싱 취약성

- terminal history는 optional adapter로만 둔다.
- ANSI parsing 실패가 핵심 timeline을 깨지 않게 한다.
- 기본 source of truth는 rollout JSONL이다.

### 대규모 코드베이스 성능

- 전체 그래프를 렌더링하지 않는다.
- file tree index와 visible subgraph를 분리한다.
- React Flow에는 현재 관심 영역만 전달한다.

### 개인정보와 외부 컨텍스트

- rollout에는 대화 본문과 tool output이 포함될 수 있으므로 로컬 처리 원칙을 기본값으로 둔다.
- Slack/Notion/Web 본문은 별도 저장하지 않는다.
- external node에는 provider, resource type, timestamp, redacted label만 저장한다.
- 상세 저장은 명시적 opt-in으로만 확장한다.

## 17. Open Questions

- Codex desktop app 또는 다른 host app의 runtime home adapter를 어디까지 기본 제공할 것인가?
- shell installer가 `CODEX_HOME` discovery 정보를 어디에 기록할 것인가?
- terminal/process watcher를 MVP에 포함할 것인가, storage watcher 이후로 미룰 것인가?
- plan extraction을 heuristic으로 시작할 것인가, MCP placeholder를 통해 명시적 plan metadata를 받게 할 것인가?
- 정적 분석은 언어별 parser/tree-sitter를 쓸 것인가, 우선 파일/폴더/변경 중심으로 유지할 것인가?
- 개발자가 개입할 수 있는 action은 알림/하이라이트/pin 수준에서 시작할 것인가, turn steer까지 제공할 것인가?

## 18. 당장 다음 액션

1. `pnpm` workspace와 Tauri desktop app scaffold를 생성한다.
2. `app/src-tauri/src/runtime_home.rs`에 runtime home discovery 초안을 만든다.
3. `session_index.rs`에서 `state_5.sqlite`의 `threads` reader를 구현한다.
4. `rollout_tailer.rs`에서 JSONL parser와 incremental tailer를 구현한다.
5. React UI에 runtime home, session list, active rollout timeline을 표시한다.
6. MCP placeholder server를 별도 패키지로 추가한다.
