export type PlaceholderVisualizationInput = {
  threadId?: string;
  turnId?: string;
  label: string;
  kind?: "context_focus" | "plan_link" | "external_context" | "risk" | "note";
  nodeIds?: string[];
  metadata?: Record<string, unknown>;
};

export type VisualizationEvent = {
  id: string;
  source: "mcp_placeholder";
  threadId?: string;
  turnId?: string;
  kind: "placeholder";
  title: string;
  nodeIds: string[];
  timestamp: number;
  payload: Record<string, unknown>;
};

export function createPlaceholderEvent(input: PlaceholderVisualizationInput): VisualizationEvent {
  return {
    id: `placeholder:${input.threadId ?? "unknown"}:${input.label}`,
    source: "mcp_placeholder",
    threadId: input.threadId,
    turnId: input.turnId,
    kind: "placeholder",
    title: input.label,
    nodeIds: input.nodeIds ?? [],
    timestamp: Date.now(),
    payload: input.metadata ?? {},
  };
}
