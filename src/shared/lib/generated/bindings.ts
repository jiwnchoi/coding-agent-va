// This file is generated from Rust types. Do not edit by hand.

export type AppTheme = "system" | "light" | "dark";

export type AppFont = "geist" | "system-sans" | "system-serif";

export type MonacoTheme = "system" | "light" | "dark";

export type RuntimeHomes = { claude: string; codex: string; pi: string };

export type AppSettings = {
  theme: AppTheme;
  font: AppFont;
  monacoTheme: MonacoTheme;
  hideCommittedFiles: boolean;
  keyboardShortcuts: { [key in string]: string };
  runtimeHomes: RuntimeHomes;
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
  gitIndexPaths: Array<string>;
};

export type SessionWatchRegistration = {
  watchId: string;
  provider: AgentSessionProvider;
  runtimeHome: string;
  watchTargets: Array<SessionWatchTarget>;
  gitIndexPaths: Array<string>;
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
  isTracked: boolean;
};

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
