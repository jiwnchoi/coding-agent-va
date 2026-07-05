import { Bot, Eye, FilePenLine, Link2, Trash2 } from "lucide-react";

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
  sources: AgentRuntimeSource[];
  sessions: AgentSessionSummary[];
};

export type SessionWatchRegistration = {
  watchId: string;
  provider: AgentSessionProvider;
  runtimeHome: string;
  watchTargets: SessionWatchTarget[];
  gitIndexPaths: string[];
};

export type SessionWatchTarget = {
  path: string;
  recursive: boolean;
  exists: boolean;
  reason: string;
};

export type AgentSessionFileActivity = {
  readFiles: string[];
  editedFiles: string[];
  impactedFiles: string[];
  deletedFiles: string[];
};

export type ActivitySectionKey = "read" | "edited" | "impacted" | "deleted";

export type ActivitySection = {
  key: ActivitySectionKey;
  title: string;
  icon: typeof Eye;
  files: string[];
};

export type SelectedActivityFile = {
  activityKey: ActivitySectionKey;
  filePath: string;
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

export function buildActivitySections(fileActivity: AgentSessionFileActivity): ActivitySection[] {
  return [
    { key: "read", title: "Read", icon: Eye, files: fileActivity.readFiles },
    { key: "edited", title: "Edited", icon: FilePenLine, files: fileActivity.editedFiles },
    { key: "impacted", title: "Impacted", icon: Link2, files: fileActivity.impactedFiles },
    { key: "deleted", title: "Deleted", icon: Trash2, files: fileActivity.deletedFiles },
  ];
}

export function formatRuntimeSources(sources: AgentRuntimeSource[]) {
  const availableSources = sources.filter((source) => source.available);

  if (availableSources.length === 0) {
    return "No agent runtime homes found";
  }

  return availableSources.map((source) => `${source.label}: ${source.runtimeHome}`).join(" · ");
}

export function providerIcon() {
  return Bot;
}
