import type {
  ArchitectureGraph,
  RuntimeHomeCandidate,
  SessionVisualizationSnapshot,
  SessionWatchEventPayload,
  VisualizerBootstrap,
} from "@/types/visualization";

const workspacePath = "/Users/jiwnchoi/git/coding-agent-va";
const runtimeHome = "/Users/jiwnchoi/.codex";
const threadId = "current-working-prototype";

const pathFor = (relativePath: string) => `${workspacePath}/${relativePath}`;

export const mockBootstrap: VisualizerBootstrap = {
  workspacePath,
  runtimeHomeCandidates: [
    {
      path: runtimeHome,
      source: "default_home",
      exists: true,
      score: 50,
      artifactCount: 4,
      workspaceThreadCount: 0,
      reason: "Browser preview uses the current workspace prototype data.",
    } satisfies RuntimeHomeCandidate,
  ],
};

const graph: ArchitectureGraph = {
  nodes: [
    {
      id: `path:${workspacePath}`,
      kind: "repo",
      label: "coding-agent-va",
      path: workspacePath,
    },
    {
      id: `path:${pathFor("docs")}`,
      kind: "directory",
      label: "docs",
      path: pathFor("docs"),
    },
    {
      id: `path:${pathFor("app")}`,
      kind: "directory",
      label: "app",
      path: pathFor("app"),
    },
    {
      id: `path:${pathFor("app/src")}`,
      kind: "directory",
      label: "src",
      path: pathFor("app/src"),
    },
    {
      id: `path:${pathFor("app/src-tauri/src")}`,
      kind: "directory",
      label: "src-tauri/src",
      path: pathFor("app/src-tauri/src"),
    },
    {
      id: `path:${pathFor("docs/PLANS.md")}`,
      kind: "file",
      label: "PLANS.md",
      path: pathFor("docs/PLANS.md"),
      metadata: { extension: "md" },
    },
    {
      id: `path:${pathFor("docs/AGENT_VIS_BRIDGE.md")}`,
      kind: "file",
      label: "AGENT_VIS_BRIDGE.md",
      path: pathFor("docs/AGENT_VIS_BRIDGE.md"),
      metadata: { extension: "md" },
    },
    {
      id: `path:${pathFor("app/src-tauri/src/agent_visualization.rs")}`,
      kind: "file",
      label: "agent_visualization.rs",
      path: pathFor("app/src-tauri/src/agent_visualization.rs"),
      metadata: { extension: "rs", language: "rust" },
    },
    {
      id: `path:${pathFor("app/src/App.tsx")}`,
      kind: "file",
      label: "App.tsx",
      path: pathFor("app/src/App.tsx"),
      metadata: { extension: "tsx", language: "tsx" },
    },
    {
      id: `path:${pathFor("app/src/lib/visualizerApi.ts")}`,
      kind: "file",
      label: "visualizerApi.ts",
      path: pathFor("app/src/lib/visualizerApi.ts"),
      metadata: { extension: "ts", language: "typescript" },
    },
  ],
  edges: [
    {
      id: "contains:repo-docs",
      kind: "contains",
      source: `path:${workspacePath}`,
      target: `path:${pathFor("docs")}`,
    },
    {
      id: "contains:repo-app",
      kind: "contains",
      source: `path:${workspacePath}`,
      target: `path:${pathFor("app")}`,
    },
    {
      id: "contains:app-src",
      kind: "contains",
      source: `path:${pathFor("app")}`,
      target: `path:${pathFor("app/src")}`,
    },
    {
      id: "contains:docs-plans",
      kind: "contains",
      source: `path:${pathFor("docs")}`,
      target: `path:${pathFor("docs/PLANS.md")}`,
    },
    {
      id: "contains:docs-bridge",
      kind: "contains",
      source: `path:${pathFor("docs")}`,
      target: `path:${pathFor("docs/AGENT_VIS_BRIDGE.md")}`,
    },
    {
      id: "contains:tauri-bridge",
      kind: "contains",
      source: `path:${pathFor("app/src-tauri/src")}`,
      target: `path:${pathFor("app/src-tauri/src/agent_visualization.rs")}`,
    },
    {
      id: "contains:src-app",
      kind: "contains",
      source: `path:${pathFor("app/src")}`,
      target: `path:${pathFor("app/src/App.tsx")}`,
    },
    {
      id: "contains:src-api",
      kind: "contains",
      source: `path:${pathFor("app/src")}`,
      target: `path:${pathFor("app/src/lib/visualizerApi.ts")}`,
    },
    {
      id: "imports:app-api",
      kind: "imports",
      source: `path:${pathFor("app/src/App.tsx")}`,
      target: `path:${pathFor("app/src/lib/visualizerApi.ts")}`,
      label: "@/lib/visualizerApi",
    },
  ],
};

export function mockSnapshot(): SessionVisualizationSnapshot {
  const now = Date.now();

  return {
    runtimeHome,
    workspacePath,
    sourceMode: "browser_preview",
    eventChannel: "codex-session-watch-event",
    generatedAtMs: now,
    watchPlan: {
      watchId: `codex-session-watch:${runtimeHome}`,
      runtimeHome,
      watchTargets: [
        {
          path: `${runtimeHome}/state_5.sqlite`,
          recursive: false,
          exists: true,
          reason: "watch SQLite index updates",
        },
        {
          path: `${runtimeHome}/state_5.sqlite-wal`,
          recursive: false,
          exists: true,
          reason: "watch SQLite WAL updates",
        },
        {
          path: `${runtimeHome}/history.jsonl`,
          recursive: false,
          exists: true,
          reason: "watch prompt history updates",
        },
        {
          path: `${runtimeHome}/sessions`,
          recursive: true,
          exists: true,
          reason: "watch rollout session trees",
        },
      ],
    },
    sessions: [
      {
        id: threadId,
        title: "Implement Codex analysis visualizer prototype",
        cwd: workspacePath,
        rolloutPath: `${runtimeHome}/sessions/current/rollout-current-working-prototype.jsonl`,
        createdAtMs: now - 28 * 60 * 1000,
        updatedAtMs: now,
        source: "browser_preview",
        modelProvider: "openai",
        gitBranch: "codex/agent-visualizer-prototype",
        preview: "Working prototype for the plan and bridge docs.",
        status: "active",
        relevanceScore: 1_000_000,
      },
    ],
    activeSessionId: threadId,
    events: [
      {
        id: `${threadId}:0`,
        threadId,
        turnId: "turn-0",
        kind: "user_message",
        timestampMs: now - 24 * 60 * 1000,
        title: "User objective",
        summary:
          "Implement docs/PLANS.md and docs/AGENT_VIS_BRIDGE.md as a working prototype, verify it in the browser, and open a PR.",
        source: "goal",
        pathMentions: ["docs/PLANS.md", "docs/AGENT_VIS_BRIDGE.md"],
        rawType: "goal",
        evidence: "docs/PLANS.md docs/AGENT_VIS_BRIDGE.md working prototype",
      },
      {
        id: `${threadId}:1`,
        threadId,
        turnId: "turn-1",
        kind: "tool_call",
        timestampMs: now - 18 * 60 * 1000,
        title: "Plan docs read",
        summary:
          "Mapped the MVP into runtime discovery, session ingestion, timeline, graph, visual events, clusters, and workspace indexing.",
        source: "filesystem",
        pathMentions: ["docs/PLANS.md", "docs/AGENT_VIS_BRIDGE.md"],
        rawType: "response_item",
        evidence: "docs/PLANS.md docs/AGENT_VIS_BRIDGE.md",
      },
      {
        id: `${threadId}:2`,
        threadId,
        turnId: "turn-2",
        kind: "patch",
        timestampMs: now - 8 * 60 * 1000,
        title: "Bridge backend added",
        summary:
          "Added session discovery, rollout normalization, focus scoring, MCP-style visual events, and rule-based clusters.",
        source: "patch",
        pathMentions: ["app/src-tauri/src/agent_visualization.rs", "app/src-tauri/src/lib.rs"],
        rawType: "response_item",
        evidence: "app/src-tauri/src/agent_visualization.rs app/src-tauri/src/lib.rs",
      },
      {
        id: `${threadId}:3`,
        threadId,
        turnId: "turn-3",
        kind: "patch",
        timestampMs: now - 4 * 60 * 1000,
        title: "Observer UI added",
        summary:
          "Replaced the placeholder with a dense three-pane observer for sessions, timeline, context graph, clusters, and evidence.",
        source: "patch",
        pathMentions: ["app/src/App.tsx", "app/src/lib/visualizerApi.ts"],
        rawType: "response_item",
        evidence: "app/src/App.tsx app/src/lib/visualizerApi.ts",
      },
      {
        id: `${threadId}:4`,
        threadId,
        turnId: "turn-4",
        kind: "watch",
        timestampMs: now - 90 * 1000,
        title: "Runtime watch ready",
        summary:
          "The watcher plan covers state_5.sqlite, state_5.sqlite-wal, history.jsonl, and rollout session trees.",
        source: "watch",
        pathMentions: ["state_5.sqlite", "history.jsonl", "sessions/rollout-current.jsonl"],
        rawType: "watch",
        evidence: "state_5.sqlite history.jsonl sessions rollout",
      },
    ],
    focusSignals: [
      {
        id: "focus:docs-plans",
        threadId,
        turnId: "turn-1",
        kind: "context_focus",
        path: pathFor("docs/PLANS.md"),
        source: "user_message",
        score: 0.58,
        timestampMs: now - 18 * 60 * 1000,
        evidence: "Plan establishes MVP scope.",
        evidenceEventId: `${threadId}:1`,
        nodeId: `path:${pathFor("docs/PLANS.md")}`,
      },
      {
        id: "focus:docs-bridge",
        threadId,
        turnId: "turn-1",
        kind: "context_focus",
        path: pathFor("docs/AGENT_VIS_BRIDGE.md"),
        source: "user_message",
        score: 0.62,
        timestampMs: now - 18 * 60 * 1000,
        evidence: "Bridge taxonomy defines focus signals, visual events, and clusters.",
        evidenceEventId: `${threadId}:1`,
        nodeId: `path:${pathFor("docs/AGENT_VIS_BRIDGE.md")}`,
      },
      {
        id: "focus:backend",
        threadId,
        turnId: "turn-2",
        kind: "edit_focus",
        path: pathFor("app/src-tauri/src/agent_visualization.rs"),
        source: "patch",
        score: 0.95,
        timestampMs: now - 8 * 60 * 1000,
        evidence: "Bridge backend added.",
        evidenceEventId: `${threadId}:2`,
        nodeId: `path:${pathFor("app/src-tauri/src/agent_visualization.rs")}`,
      },
      {
        id: "focus:ui",
        threadId,
        turnId: "turn-3",
        kind: "edit_focus",
        path: pathFor("app/src/App.tsx"),
        source: "patch",
        score: 0.9,
        timestampMs: now - 4 * 60 * 1000,
        evidence: "Observer UI added.",
        evidenceEventId: `${threadId}:3`,
        nodeId: `path:${pathFor("app/src/App.tsx")}`,
      },
    ],
    visualAgentEvents: [
      {
        id: "visual:planning-docs",
        threadId,
        phase: "checkpoint",
        kind: "external_context_marker",
        label: "Planning docs used as bridge input",
        visualTargetHints: ["docs/PLANS.md", "docs/AGENT_VIS_BRIDGE.md"],
        visualStyle: "badge",
        summary: "The UI is grounded in the plan and bridge taxonomy.",
        relatedHints: [],
        metadata: {},
        timestampMs: now - 18 * 60 * 1000,
      },
      {
        id: "visual:prototype-boundary",
        threadId,
        phase: "after_edit",
        kind: "change_boundary",
        label: "Current workspace change unit",
        visualTargetHints: [
          "app/src-tauri/src/agent_visualization.rs",
          "app/src/App.tsx",
          "app/src/lib/visualizerApi.ts",
        ],
        visualStyle: "group",
        summary: "Backend bridge and frontend observer belong to one feature cluster.",
        relatedHints: ["docs/AGENT_VIS_BRIDGE.md"],
        metadata: {},
        timestampMs: now - 4 * 60 * 1000,
      },
      {
        id: "visual:watch-risk",
        threadId,
        phase: "checkpoint",
        kind: "risk_marker",
        label: "Runtime data may be stale",
        visualTargetHints: ["state_5.sqlite", "sessions/**/*.jsonl"],
        visualStyle: "badge",
        summary:
          "The app shows diagnostics when active API sessions are not present in local rollout files.",
        relatedHints: [],
        metadata: {},
        timestampMs: now - 90 * 1000,
      },
    ],
    changeClusters: [
      {
        id: "cluster:prototype",
        threadId,
        turnIds: ["turn-1", "turn-2", "turn-3"],
        title: "Implementation change cluster",
        intent: "feature",
        status: "complete",
        nodeIds: [
          `path:${pathFor("app/src-tauri/src/agent_visualization.rs")}`,
          `path:${pathFor("app/src/App.tsx")}`,
          `path:${pathFor("app/src/lib/visualizerApi.ts")}`,
        ],
        focusSignalIds: ["focus:backend", "focus:ui", "focus:docs-bridge"],
        visualAgentEventIds: [
          "visual:planning-docs",
          "visual:prototype-boundary",
          "visual:watch-risk",
        ],
        evidenceEventIds: [`${threadId}:1`, `${threadId}:2`, `${threadId}:3`],
        summary:
          "Rule-based cluster from same-session path mentions, patch signals, and visual event hints.",
      },
    ],
    graph,
    diagnostics: [
      "Browser preview mode. Tauri builds load the live runtime home and current workspace directly.",
    ],
  };
}

let mockWatchCursor = 0;

export function nextMockWatchEvent(): SessionWatchEventPayload {
  const changedPaths = [
    `${runtimeHome}/state_5.sqlite-wal`,
    `${runtimeHome}/sessions/current/rollout-current-working-prototype.jsonl`,
    `${runtimeHome}/history.jsonl`,
  ];
  const changedPath = changedPaths[mockWatchCursor % changedPaths.length];
  mockWatchCursor += 1;

  return {
    watchId: `codex-session-watch:${runtimeHome}`,
    runtimeHome,
    changedPaths: [changedPath],
    eventTags: ["browser_preview_tick"],
    timestampMs: Date.now(),
  };
}
