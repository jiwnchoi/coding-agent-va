// This file is generated from Rust types. Do not edit by hand.

export type AppTheme = "system" | "light" | "dark";

export type AppFont = "geist" | "system-sans" | "system-serif";

export type MonacoTheme = "system" | "light" | "dark";

export type RuntimeHomes = { claude: string; codex: string; pi: string };

export type DescriptionReasoning = "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max";

export type DescriptionProviderSettings = { model: string; reasoning: DescriptionReasoning };

export type DescriptionSettings = {
  codex: DescriptionProviderSettings;
  claude: DescriptionProviderSettings;
  pi: DescriptionProviderSettings;
};

export type AppSettings = {
  theme: AppTheme;
  font: AppFont;
  monacoTheme: MonacoTheme;
  showReadFiles: boolean;
  keyboardShortcuts: { [key in string]: string };
  runtimeHomes: RuntimeHomes;
  descriptions: DescriptionSettings;
};

export type LogLevel = "debug" | "info" | "warn" | "error";

export type LogEntry = {
  timestamp: string;
  level: LogLevel;
  message: string;
  context: { [key in string]: string } | null;
};

export type AgentSessionProvider = "codex" | "claude" | "pi";

export type AgentRuntimeSource = {
  provider: AgentSessionProvider;
  label: string;
  runtimeHome: string;
  available: boolean;
};

export type AgentSessionSummary = {
  id: string;
  provider: AgentSessionProvider;
  providerSessionId: string;
  providerLabel: string;
  title: string;
  transcriptPath: string;
  cwd: string | null;
  runtimeHome: string;
  updatedAtMs: number;
};

export type AgentSessionList = {
  sources: Array<AgentRuntimeSource>;
  sessions: Array<AgentSessionSummary>;
  hasMore: boolean;
  nextOffset: number;
};

export type AgentSessionTaskStatus = "pending" | "in_progress" | "completed";

export type AgentSessionTask = {
  id: string;
  nativeId: string | null;
  subject: string;
  description: string | null;
  activeForm: string | null;
  status: AgentSessionTaskStatus;
  dependsOn: Array<string>;
  position: number;
  summary: string | null;
  fileActivity: AgentSessionFileActivity;
  startEntryIndex: number;
  endEntryIndex: number;
};

export type AgentSessionDetails = {
  fileActivity: AgentSessionFileActivity;
  turns: Array<AgentSessionPromptTurn>;
};

export type AgentSessionPromptTurn = {
  id: string;
  prompts: Array<string>;
  summary: string | null;
  tasks: Array<AgentSessionTask>;
  fileActivity: AgentSessionFileActivity;
  startedAtMs: number;
  startEntryIndex: number;
  endEntryIndex: number;
};

export type SessionWatchTarget = {
  path: string;
  recursive: boolean;
  exists: boolean;
  reason: string;
};

export type SessionWatchPlan = {
  watchId: string;
  provider: AgentSessionProvider;
  runtimeHome: string;
  watchTargets: Array<SessionWatchTarget>;
};

export type SessionWatchRegistration = {
  watchId: string;
  provider: AgentSessionProvider;
  runtimeHome: string;
  watchTargets: Array<SessionWatchTarget>;
};

export type SessionWatchEventPayload = {
  watchId: string;
  provider: AgentSessionProvider;
  runtimeHome: string;
  changedPaths: Array<string>;
  eventTags: Array<string>;
  timestampMs: number;
};

export type AgentSessionFileActivity = {
  readFiles: Array<string>;
  editedFiles: Array<string>;
  impactedFiles: Array<string>;
  deletedFiles: Array<string>;
  impactedRelations: Array<AgentSessionImpactedFileRelation>;
};

export type AgentSessionImpactedFileRelation = {
  changedFile: string;
  impactedFile: string;
  importSpecifier: string;
};

export type AgentSessionFileDiff = {
  filePath: string;
  displayPath: string;
  originalContent: string;
  modifiedContent: string;
  diffBaseLabel: string;
  diffTargetLabel: string;
  fileMissing: boolean;
};

export type DescriptionGraphNode = { label: string; path: string; activities: Array<string> };

export type DescriptionGraphRelation = {
  sourcePath: string;
  targetPath: string;
  importSpecifier: string;
};

export type AgentSessionNodeDescriptionRequest = {
  provider: AgentSessionProvider;
  providerSessionId: string;
  transcriptPath: string;
  runtimeHome: string;
  model: string;
  reasoning: DescriptionReasoning;
  cwd: string;
  clickedNode: DescriptionGraphNode;
  relatedNodes: Array<DescriptionGraphNode>;
  relations: Array<DescriptionGraphRelation>;
};

export type AgentSessionNodeDescriptionResponse = { description: string; providerLabel: string };

export type AgentSessionNodeDescriptionStreamEvent =
  | { type: "started"; providerLabel: string; cached: boolean }
  | { type: "chunk"; text: string };

export type NodeKind = "repo" | "directory" | "file" | "symbol" | "external";

export type EdgeKind = "contains" | "imports" | "declares";

export type ArchitectureNode = {
  id: string;
  kind: NodeKind;
  label: string;
  path?: string;
  metadata?: { [key in string]: string };
};

export type ArchitectureEdge = {
  id: string;
  kind: EdgeKind;
  source: string;
  target: string;
  label?: string;
};

export type ArchitectureGraph = { nodes: Array<ArchitectureNode>; edges: Array<ArchitectureEdge> };
