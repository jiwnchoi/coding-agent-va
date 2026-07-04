import { Eye, FilePenLine, Trash2 } from "lucide-react";

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
};

export type CodexSessionFileActivity = {
  readFiles: string[];
  editedFiles: string[];
  deletedFiles: string[];
};

export type ActivitySection = {
  key: string;
  title: string;
  icon: typeof Eye;
  files: string[];
};

export type SelectedActivityFile = {
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
    { key: "deleted", title: "Deleted", icon: Trash2, files: fileActivity.deletedFiles },
  ];
}
