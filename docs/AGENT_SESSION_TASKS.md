# Agent Session Tasks

## 문서 목적

이 문서는 Codex와 Claude Code가 제공하는 구조화된 실행 Task를 provider에 독립적인
모델로 정규화하기 위한 실제 세션 분석 결과를 기록한다.

Task source는 다음 두 가지로 제한한다.

- Codex: rollout의 `update_plan`
- Claude Code: `~/.claude/tasks/<sessionId>` Task store

이외의 데이터는 공통 Task 모델의 입력으로 사용하지 않는다.

## 검증 환경

- Codex CLI `0.144.5`
- Claude Code transcript version `2.1.214`
- 검증일: 2026-07-18

CLI 버전에 따라 transcript 형식이 달라질 수 있으므로 Codex parser는 아래의 구형 및 현재
형식을 함께 지원해야 한다.

## Codex `update_plan`

### 현재 rollout 형식

현재 Codex는 JavaScript orchestration source 안에서 `update_plan`을 호출한다.

```json
{
  "type": "response_item",
  "payload": {
    "type": "custom_tool_call",
    "name": "exec",
    "call_id": "call_example",
    "input": "const r = await tools.update_plan({plan:[{step:\"Inspect API\",status:\"in_progress\"},{step:\"Implement parser\",status:\"pending\"}]}); text(r);"
  }
}
```

Codex UI의 **Updated plan** 표시는 이 호출을 렌더링한 결과다. `Updated plan`이라는 별도
이벤트가 rollout에 저장되는 것은 아니다.

tool output은 빈 객체 `{}`일 수 있으므로 output을 Task source로 사용하면 안 된다. Task
source of truth는 `tools.update_plan(...)`의 입력이다.

### 구형 rollout 형식

구형 Codex rollout에는 JSON arguments를 가진 직접 함수 호출이 존재한다.

```json
{
  "type": "response_item",
  "payload": {
    "type": "function_call",
    "name": "update_plan",
    "call_id": "call_example",
    "arguments": "{\"explanation\":\"...\",\"plan\":[{\"step\":\"Inspect API\",\"status\":\"in_progress\"}]}"
  }
}
```

### 실시간 JSON 출력

`codex exec --json`은 Task를 `todo_list` item으로 변환해 출력하기도 한다.

```json
{
  "type": "item.started",
  "item": {
    "type": "todo_list",
    "items": [
      {
        "text": "Inspect API",
        "completed": false
      }
    ]
  }
}
```

이 표현은 `pending`과 `in_progress`를 모두 `completed: false`로 축약하므로 저장 rollout의
원본 `update_plan`보다 정보가 적다.

### Parser 규칙

1. 현재 형식의 `custom_tool_call(name = "exec")`에서 `tools.update_plan` 호출을 찾는다.
2. JavaScript를 실행하거나 `eval`하지 않는다.
3. JavaScript/TypeScript AST를 사용하고 객체, 배열, 문자열 literal만 허용한다.
4. 구형 `function_call(name = "update_plan")`의 JSON arguments도 지원한다.
5. 여러 호출이 있으면 마지막 호출을 현재 Task snapshot으로 사용한다.
6. `call_id`를 snapshot revision으로 사용한다.
7. 각 호출을 시간순으로 보존하면 Task 상태 변경 이력을 재생할 수 있다.

### 검증 샘플

File diff를 session replay 방식으로 전환하는 구현 세션에서 다음 Task snapshot을
확인했다.

```json
{
  "revisionId": "call_TkEVkoIpMBWjEZf0EV5DMGp6",
  "tasks": [
    {
      "subject": "현재 diff API와 Git 의존성/테스트 구조 확인",
      "status": "in_progress"
    },
    {
      "subject": "session transcript replay 모델과 provider별 parser 구현",
      "status": "pending"
    },
    {
      "subject": "Rust command/API 타입 및 프론트 호출/UI 연결",
      "status": "pending"
    },
    {
      "subject": "Git 기반 activity/cache 의존성 정리",
      "status": "pending"
    },
    {
      "subject": "테스트 및 just check 실행",
      "status": "pending"
    }
  ]
}
```

Codex `update_plan`에는 Task native ID, 상세 설명, 실행 중 문구, dependency가 없다.

## Claude Code Task store

### 현재 snapshot

Claude Code Task의 현재 snapshot은 session ID별 directory에 저장된다.

```text
~/.claude/tasks/<sessionId>/<taskId>.json
```

Task JSON schema는 다음과 같다.

```json
{
  "id": "1",
  "subject": "Add Rust plan types + Cargo dependency",
  "description": "Add AgentSessionPlanStatus and related types",
  "activeForm": "Adding Rust plan types + Cargo dependency",
  "status": "pending",
  "blocks": [],
  "blockedBy": []
}
```

화면의 `◻` 표시는 `status: "pending"`이고 표시 문구는 `subject`다. `activeForm`은 실행
중인 Task를 표시할 때 사용할 수 있다.

### Task history

Task 생성 및 변경 이력은 transcript의 `TaskCreate`와 `TaskUpdate` tool call에 남는다.

```json
{
  "type": "tool_use",
  "name": "TaskCreate",
  "input": {
    "subject": "Add Rust plan types + Cargo dependency",
    "description": "...",
    "activeForm": "Adding Rust plan types + Cargo dependency"
  }
}
```

생성 결과에는 할당된 native Task ID가 포함된다.

```json
{
  "toolUseResult": {
    "task": {
      "id": "1",
      "subject": "Add Rust plan types + Cargo dependency"
    }
  }
}
```

Claude Code가 worktree로 구현을 이어가면 transcript의 project directory가 달라질 수 있다.
여러 transcript는 같은 `sessionId`와 Task store를 공유한다. Task history를 재구성할 때는
하나의 transcript path만 보지 말고 동일 `sessionId`를 가진 모든 project transcript를
찾아야 한다.

### Parser 규칙

1. 현재 상태는 `~/.claude/tasks/<sessionId>/*.json`을 읽는다.
2. `.lock`은 Task 파일이 아니므로 제외한다.
3. Task ID는 문자열이므로 숫자로 변환해 정렬한다.
4. Task 파일 변경을 watch하며, 쓰기 도중 JSON parse 실패는 짧게 재시도한다.
5. 변경 이력이 필요하면 동일 `sessionId`의 `TaskCreate`와 `TaskUpdate`를 시간순 replay한다.
6. `blockedBy`를 공통 모델의 dependency로 사용한다.
7. `blocks`는 dependency의 역방향이므로 공통 모델에 중복 저장하지 않고 계산한다.

### 검증 샘플

실제 구현 세션에서 7개의 Task JSON과 7개의 `TaskCreate` 호출을 확인했다.

```text
1. Add Rust plan types + Cargo dependency
2. Implement activity/plan_mode.rs parser
3. Wire provider trait + Claude protocol override
4. Add plan cache + invalidation
5. Add get_agent_session_plan Tauri command + wiring
6. Regenerate TS bindings + Rust tests
7. Frontend data layer for plan
```

검증 시점에는 7개 Task가 모두 `pending` 상태였다.

## 공통 Task 모델

Rust 타입을 source of truth로 사용할 때 다음 모델을 권장한다.

```rust
#[derive(Clone, Copy, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum AgentSessionTaskStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Clone, Copy, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum AgentSessionTaskSource {
    CodexUpdatePlan,
    ClaudeTasks,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionTask {
    pub id: String,
    pub native_id: Option<String>,
    pub subject: String,
    pub description: Option<String>,
    pub active_form: Option<String>,
    pub status: AgentSessionTaskStatus,
    pub depends_on: Vec<String>,
    pub position: usize,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionTaskList {
    pub provider: AgentSessionProvider,
    pub provider_session_id: String,
    pub source: AgentSessionTaskSource,
    pub revision_id: Option<String>,
    pub explanation: Option<String>,
    pub tasks: Vec<AgentSessionTask>,
    pub updated_at_ms: u64,
}
```

### Provider 매핑

| 공통 필드 | Codex `update_plan` | Claude Task JSON |
| --- | --- | --- |
| `nativeId` | 없음 | `id` |
| `subject` | `step` | `subject` |
| `description` | 없음 | `description` |
| `activeForm` | 없음 | `activeForm` |
| `status` | `status` | `status` |
| `dependsOn` | 빈 배열 | `blockedBy` |
| `position` | Plan 배열 index | 숫자 Task ID 순서 |
| `revisionId` | `call_id` | 최신 Task event 또는 snapshot revision |
| `explanation` | `explanation` | 없음 |

### 공통 ID

Claude Task는 native ID가 있지만 Codex Task는 없다. UI와 cache에서 사용할 공통 ID는
provider와 session namespace를 포함해야 한다.

```text
Claude: claude:<sessionId>:<nativeTaskId>
Codex:  codex:<sessionId>:<normalizedStepHash>:<duplicateIndex>
```

Codex step 문구가 바뀌면 ID도 바뀔 수 있다. 서로 다른 `update_plan` snapshot 사이에서
Task identity를 유지하려면 exact text, position, 유사도 등을 이용한 reconciliation이 별도로
필요하다. provider가 제공하지 않는 dependency나 identity를 parser가 추측해서는 안 된다.

## 조회 우선순위

### Codex

1. 마지막 `tools.update_plan(...)`
2. 마지막 직접 `function_call(name = "update_plan")`
3. Task가 없으면 빈 Task list

### Claude Code

1. `~/.claude/tasks/<sessionId>/*.json` 현재 snapshot
2. 현재 snapshot이 없고 history가 필요하면 동일 session ID의 `TaskCreate`/`TaskUpdate` replay
3. Task가 없으면 빈 Task list

## Watch와 cache 고려사항

- Codex Task는 rollout append로 갱신되므로 기존 session tree watch에 포함된다.
- Claude Task는 `projects/` 밖의 `tasks/`에 있으므로 `~/.claude/tasks`를 별도로 watch해야
  한다.
- Codex cache revision은 마지막 `update_plan.call_id`와 rollout mtime을 사용할 수 있다.
- Claude cache revision은 Task directory 내 JSON 파일의 이름, mtime, content를 기반으로
  계산할 수 있다.
- 하나의 Claude session이 여러 project/worktree transcript에 나타날 수 있으므로
  transcript path를 Task cache identity로 사용하면 안 된다.

## 최종 결론

- Codex의 `update_plan.plan[]`과 Claude의 Task store만 공통 Task 모델의 source로 사용한다.
- Codex UI의 `Updated plan`과 Claude UI의 `◻` 목록은 구조화된 Task를 렌더링한 결과다.
- provider별 원본에 없는 ID, 설명, 실행 문구, dependency는 parser가 추측하지 않는다.

## Prompt 및 Task별 시각화 구간

세션 전체 파일 활동을 하나의 그래프로 합치지 않고 다음 실행 구간으로 나눈다.

1. user message가 시작되면 새로운 Prompt turn을 만든다.
2. 다음 user message 직전까지 발생한 파일 활동을 해당 Prompt turn에 귀속한다.
3. turn 내부의 마지막 assistant text만 Prompt 종료 summary로 표시한다.
4. 중간 assistant text와 tool call은 타임라인 근거로만 사용하고 메시지 목록에는 직접 표시하지 않는다.
5. Task가 snapshot에 처음 생성된 시점부터 `completed`로 전환될 때까지 발생한 파일 활동은 Prompt와 해당 Task 양쪽에 귀속한다. 여러 Task가 한 snapshot에서 함께 완료되면 각 Task가 공유 lifecycle 구간의 활동을 모두 가진다.
6. Task가 완료되기 전에 기록된 마지막 assistant text가 있으면 Task summary로 사용한다.
7. 원본에 Task summary가 없으면 별도의 LLM 추론으로 생성하지 않는다.
8. Prompt를 선택하면 turn 전체 파일 활동을, Task를 선택하면 Task 실행 구간의 파일 활동만 context graph에 전달한다.
9. `AGENTS.md` 지침이나 XML 형태의 environment context처럼 runtime이 주입한 metadata user message는 Prompt turn에서 제외한다.
10. user prompt와 assistant summary는 첫 줄의 짧은 preview를 기본으로 표시하고 사용자가 클릭하면 전체 내용을 펼친다.
11. Task는 세션 전체에서 공통 ID로 추적하며 최초 생성 Prompt에 한 번만 표시한다. 이후 snapshot은 append하지 않고 상태, summary, 파일 활동을 갱신한다.
12. Task 파일 활동 구간은 최초 생성부터 완료 전환까지다. Prompt interrupt가 발생해도 진행 중인 Task의 구간은 끊지 않는다.
13. Prompt summary는 다음 실제 user prompt 직전의 마지막 agent action이 assistant text일 때만 사용한다. 마지막 action이 tool call인 interrupt turn에서는 앞선 중간 assistant text를 summary로 승격하지 않는다.
14. 사이드 패널은 Prompt tracking과 Task tracking을 별도 탭으로 제공한다. Prompt 탭은 user prompt와 종료 summary만 표시하고, Task 탭은 Task 상태와 Task summary만 표시한다.

Codex는 각 `update_plan` snapshot을 시간순으로 replay하여 활성 Task를 결정한다. Claude Code는
transcript의 `TaskCreate`와 `TaskUpdate`를 replay하고 Task store snapshot을 현재 상태와 상세 필드
보정에 사용한다. 구조화된 Task source가 없는 provider는 Prompt 단위 구간만 제공한다.
