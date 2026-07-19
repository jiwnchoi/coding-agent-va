import { Bot, Eye, FilePenLine, Link2, Trash2 } from "lucide-react";

import type { AgentRuntimeSource, AgentSessionFileActivity } from "@/shared/lib/generated/bindings";

export type {
  AgentRuntimeSource,
  AgentSessionFileActivity,
  AgentSessionFileDiff,
  AgentSessionImpactedFileRelation,
  AgentSessionList,
  AgentSessionProvider,
  AgentSessionSummary,
  SessionWatchEventPayload,
  SessionWatchPlan,
  SessionWatchRegistration,
  SessionWatchTarget,
} from "@/shared/lib/generated/bindings";

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

export type SessionScopeSelection = {
  turnId: string;
  taskId: string | null;
  startEntryIndex: number;
  endEntryIndex: number;
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
