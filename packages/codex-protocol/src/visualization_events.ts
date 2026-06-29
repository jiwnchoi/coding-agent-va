export type ArchitectureNode = {
  id: string;
  kind: "repo" | "directory" | "file" | "symbol" | "external" | "plan";
  label: string;
  path?: string;
  metadata?: Record<string, unknown>;
};

export type VisualizationEvent = {
  id: string;
  source: "codex_app_server" | "mcp_placeholder" | "graph_indexer";
  threadId?: string;
  turnId?: string;
  kind:
    | "session_detected"
    | "plan_updated"
    | "context_focus"
    | "file_changed"
    | "command_executed"
    | "external_context_used"
    | "placeholder";
  title: string;
  nodeIds: string[];
  timestamp: number;
  payload: Record<string, unknown>;
};

export type PlaceholderVisualizationInput = {
  threadId?: string;
  turnId?: string;
  label: string;
  kind?: "context_focus" | "plan_link" | "external_context" | "risk" | "note";
  nodeIds?: string[];
  metadata?: Record<string, unknown>;
};
