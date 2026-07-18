import { invoke } from "@tauri-apps/api/core";
import {
  array,
  boolean,
  nullable,
  number,
  object,
  optional,
  parse as parseValue,
  picklist,
  record,
  string,
  type BaseIssue,
  type BaseSchema,
  type InferOutput,
} from "valibot";

import type {
  AgentSessionFileDiff,
  AgentSessionDetails,
  AgentSessionList,
  ArchitectureGraph,
  AppSettings,
} from "@/shared/lib/generated/bindings";

const provider = picklist(["codex", "claude", "pi"]);
const sessionListSchema = object({
  sources: array(
    object({ provider, label: string(), runtimeHome: string(), available: boolean() })
  ),
  sessions: array(
    object({
      id: string(),
      provider,
      providerSessionId: string(),
      providerLabel: string(),
      title: string(),
      transcriptPath: string(),
      cwd: nullable(string()),
      runtimeHome: string(),
      updatedAtMs: number(),
    })
  ),
  hasMore: boolean(),
  nextOffset: number(),
});
const activitySchema = object({
  readFiles: array(string()),
  editedFiles: array(string()),
  impactedFiles: array(string()),
  deletedFiles: array(string()),
  impactedRelations: array(
    object({ changedFile: string(), impactedFile: string(), importSpecifier: string() })
  ),
});
const taskStatus = picklist(["pending", "in_progress", "completed"]);
const taskSchema = object({
  id: string(),
  nativeId: nullable(string()),
  subject: string(),
  description: nullable(string()),
  activeForm: nullable(string()),
  status: taskStatus,
  dependsOn: array(string()),
  position: number(),
  summary: nullable(string()),
  fileActivity: activitySchema,
});
const detailsSchema = object({
  fileActivity: activitySchema,
  turns: array(
    object({
      id: string(),
      prompts: array(string()),
      summary: nullable(string()),
      tasks: array(taskSchema),
      fileActivity: activitySchema,
      startedAtMs: number(),
    })
  ),
});
const diffSchema = object({
  filePath: string(),
  displayPath: string(),
  originalContent: string(),
  modifiedContent: string(),
  diffBaseLabel: string(),
  diffTargetLabel: string(),
  fileMissing: boolean(),
  isTracked: boolean(),
});
const graphSchema = object({
  nodes: array(
    object({
      id: string(),
      kind: picklist(["repo", "directory", "file", "symbol", "external"]),
      label: string(),
      path: optional(string()),
      metadata: optional(record(string(), string())),
    })
  ),
  edges: array(
    object({
      id: string(),
      kind: picklist(["contains", "imports", "declares"]),
      source: string(),
      target: string(),
      label: optional(string()),
    })
  ),
});
const reasoning = picklist(["none", "minimal", "low", "medium", "high", "xhigh", "max"]);
const settingsSchema = object({
  theme: picklist(["system", "light", "dark"]),
  font: picklist(["geist", "system-sans", "system-serif"]),
  monacoTheme: picklist(["system", "light", "dark"]),
  showReadFiles: boolean(),
  keyboardShortcuts: record(string(), string()),
  runtimeHomes: object({ claude: string(), codex: string(), pi: string() }),
  descriptions: object({
    codex: object({ model: string(), reasoning }),
    claude: object({ model: string(), reasoning }),
    pi: object({ model: string(), reasoning }),
  }),
});

export const queryKeys = {
  sessions: (runtimeHomes: Record<string, string>) => ["agent-sessions", runtimeHomes] as const,
  sessionDetails: (sessionId: string) => ["session-details", sessionId] as const,
  fileDiff: (cwd: string | null, filePath: string) => ["session-file-diff", cwd, filePath] as const,
  workspaceGraph: (workspacePath: string) => ["workspace-graph", workspacePath] as const,
};

export async function listAgentSessions(
  runtimeHomes: Record<string, string>,
  offset: number,
  limit: number
): Promise<AgentSessionList> {
  return parse(
    sessionListSchema,
    await invoke<unknown>("list_agent_sessions", { runtimeHomes, offset, limit })
  );
}
export async function getAgentSessionDetails(args: {
  provider: string;
  providerSessionId: string;
  transcriptPath: string;
  runtimeHome: string;
  cwd: string | null;
}): Promise<AgentSessionDetails> {
  return parse(detailsSchema, await invoke<unknown>("get_agent_session_details", args));
}
export async function getAgentSessionFileDiff(args: {
  filePath: string;
  cwd: string | null;
}): Promise<AgentSessionFileDiff> {
  return parse(diffSchema, await invoke<unknown>("get_agent_session_file_diff", args));
}
export async function indexWorkspaceGraph(workspacePath: string): Promise<ArchitectureGraph> {
  return parse(graphSchema, await invoke<unknown>("index_workspace_graph", { workspacePath }));
}
export async function loadAppSettings(): Promise<AppSettings> {
  return parse(settingsSchema, await invoke<unknown>("load_app_settings"));
}
export function saveAppSettings(settings: AppSettings) {
  return invoke<void>("save_app_settings", { settings });
}

function parse<TSchema extends BaseSchema<unknown, unknown, BaseIssue<unknown>>>(
  schema: TSchema,
  input: unknown
): InferOutput<TSchema> {
  return parseValue(schema, input);
}
