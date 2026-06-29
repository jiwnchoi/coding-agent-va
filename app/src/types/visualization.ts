export type NodeKind = "repo" | "directory" | "file" | "symbol" | "external";

export type EdgeKind = "contains" | "imports" | "declares";

export type ArchitectureNode = {
  id: string;
  kind: NodeKind;
  label: string;
  path?: string;
  metadata?: Record<string, string>;
};

export type ArchitectureEdge = {
  id: string;
  kind: EdgeKind;
  source: string;
  target: string;
  label?: string;
};

export type ArchitectureGraph = {
  nodes: ArchitectureNode[];
  edges: ArchitectureEdge[];
};

export type RuntimeHomeCandidate = {
  path: string;
  source: string;
  exists: boolean;
  score: number;
  artifactCount: number;
  workspaceThreadCount: number;
  reason: string;
};

export type VisualizerBootstrap = {
  workspacePath: string;
  runtimeHomeCandidates: RuntimeHomeCandidate[];
};

export type SessionWatchTarget = {
  path: string;
  recursive: boolean;
  exists: boolean;
  reason: string;
};

export type SessionWatchPlan = {
  watchId: string;
  runtimeHome: string;
  watchTargets: SessionWatchTarget[];
};

export type CodexSessionSummary = {
  id: string;
  title: string;
  cwd: string;
  rolloutPath?: string;
  createdAtMs: number;
  updatedAtMs: number;
  source: string;
  modelProvider: string;
  gitBranch?: string;
  preview: string;
  status: string;
  relevanceScore: number;
};

export type SessionEventKind =
  | "user_message"
  | "assistant_message"
  | "tool_call"
  | "tool_output"
  | "command"
  | "patch"
  | "watch"
  | "system"
  | "error";

export type NormalizedSessionEvent = {
  id: string;
  threadId: string;
  turnId?: string;
  kind: SessionEventKind;
  timestampMs: number;
  title: string;
  summary: string;
  source: string;
  pathMentions: string[];
  command?: string;
  rawType?: string;
  evidence: string;
};

export type FocusKind = "view_focus" | "edit_focus" | "context_focus";

export type FocusSignal = {
  id: string;
  threadId: string;
  turnId?: string;
  kind: FocusKind;
  path?: string;
  symbol?: string;
  source: string;
  score: number;
  timestampMs: number;
  evidence: string;
  evidenceEventId: string;
  nodeId?: string;
};

export type VisualPhase = "before_edit" | "after_edit" | "checkpoint";

export type VisualAgentEventKind =
  | "change_boundary"
  | "relationship"
  | "risk_marker"
  | "decision_marker"
  | "external_context_marker";

export type VisualStyle = "highlight" | "group" | "badge" | "edge" | "timeline_marker";

export type VisualAgentEvent = {
  id: string;
  threadId?: string;
  turnId?: string;
  phase: VisualPhase;
  kind: VisualAgentEventKind;
  label: string;
  visualTargetHints: string[];
  visualStyle?: VisualStyle;
  summary?: string;
  relatedHints: string[];
  metadata: Record<string, unknown>;
  timestampMs: number;
};

export type ChangeCluster = {
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

export type SessionVisualizationSnapshot = {
  runtimeHome: string;
  workspacePath: string;
  sourceMode: string;
  eventChannel: string;
  generatedAtMs: number;
  watchPlan?: SessionWatchPlan;
  sessions: CodexSessionSummary[];
  activeSessionId?: string;
  events: NormalizedSessionEvent[];
  focusSignals: FocusSignal[];
  visualAgentEvents: VisualAgentEvent[];
  changeClusters: ChangeCluster[];
  graph: ArchitectureGraph;
  diagnostics: string[];
};

export type SessionWatchEventPayload = {
  watchId: string;
  runtimeHome: string;
  changedPaths: string[];
  eventTags: string[];
  timestampMs: number;
};
