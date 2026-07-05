import { Eye, FilePenLine, Link2, Trash2 } from "lucide-react";

export type CodexSessionSummary = {
  id: string;
  title: string;
  rolloutPath: string;
  cwd: string | null;
  updatedAtMs: number;
};

export type CodexSessionList = {
  runtimeHome: string;
  sessions: CodexSessionSummary[];
};

export type SessionWatchRegistration = {
  watchId: string;
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

export type CodexSessionFileActivity = {
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

export type CodexSessionFileDiff = {
  filePath: string;
  displayPath: string;
  originalContent: string;
  modifiedContent: string;
  diffBaseLabel: string;
  diffTargetLabel: string;
  fileMissing: boolean;
  isTracked: boolean;
};

export function buildActivitySections(fileActivity: CodexSessionFileActivity): ActivitySection[] {
  return [
    { key: "read", title: "Read", icon: Eye, files: fileActivity.readFiles },
    { key: "edited", title: "Edited", icon: FilePenLine, files: fileActivity.editedFiles },
    { key: "impacted", title: "Impacted", icon: Link2, files: fileActivity.impactedFiles },
    { key: "deleted", title: "Deleted", icon: Trash2, files: fileActivity.deletedFiles },
  ];
}
