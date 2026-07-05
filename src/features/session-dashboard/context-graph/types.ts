import type { Node, Edge } from "@xyflow/react";

import type {
  ArchitectureEdge,
  ArchitectureGraph,
  ArchitectureNode,
  NodeKind,
} from "@/lib/generated/bindings";
import type {
  ActivitySectionKey,
  AgentSessionFileActivity,
  AgentSessionImpactedFileRelation,
} from "@/lib/session-watch";

export type { ArchitectureEdge, ArchitectureGraph, ArchitectureNode };
export type ArchitectureNodeKind = NodeKind;

export type ContextGraphNodeKind = "repo" | "directory" | "file";
export type ContextGraphEdgeKind = "contains" | "impact";

export type ContextGraphNodeData = {
  label: string;
  path: string;
  displayPath: string;
  kind: ContextGraphNodeKind;
  activities: ActivitySectionKey[];
  isSelected: boolean;
  isPinned: boolean;
  childActivityCount: number;
};

export type ContextGraphEdgeData = {
  kind: ContextGraphEdgeKind;
  importSpecifier?: string;
  isHighlighted: boolean;
};

export type ContextGraphNode = Node<ContextGraphNodeData, "contextGraphNode">;
export type ContextGraphEdge = Edge<ContextGraphEdgeData>;

export type ContextGraphModel = {
  nodes: ContextGraphNode[];
  containsEdges: ContextGraphEdge[];
  impactEdges: ContextGraphEdge[];
  activity: AgentSessionFileActivity;
  impactedRelations: AgentSessionImpactedFileRelation[];
};

export type ContextGraphBuildOptions = {
  architectureGraph: ArchitectureGraph | null;
  fileActivity: AgentSessionFileActivity;
  includeEntireWorkspace: boolean;
  pinnedFilePaths: string[];
  selectedFilePath: string;
  workspacePath: string | null;
};
