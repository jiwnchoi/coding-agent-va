import type { ActivitySectionKey } from "@/features/session-dashboard/lib/session-watch";

import { displayPathForNode } from "./contextGraphPaths";
import styles from "./ContextGraphView.module.css";
import type { ArchitectureNode, ContextGraphNode, ContextGraphNodeData } from "./types";

const CONTEXT_NODE_TYPE = "contextGraphNode" as const;

export function toContextGraphNode({
  activities,
  childActivityCount,
  hasDirectFiles,
  isPinned,
  isSelected,
  node,
  workspacePath,
}: {
  activities: ActivitySectionKey[];
  childActivityCount: number;
  hasDirectFiles: boolean;
  isPinned: boolean;
  isSelected: boolean;
  node: ArchitectureNode;
  workspacePath: string;
}): ContextGraphNode {
  return {
    id: node.id,
    position: { x: 0, y: 0 },
    type: CONTEXT_NODE_TYPE,
    className:
      node.kind === "directory"
        ? styles.directoryNodeWrapper
        : isSelected
          ? styles.selectedNode
          : undefined,
    data: {
      activities,
      childActivityCount,
      displayPath: displayPathForNode(node, workspacePath),
      hasDirectFiles,
      isPinned,
      isSelected,
      kind: node.kind as ContextGraphNodeData["kind"],
      label: node.label,
      language: node.metadata?.language ?? "",
      path: node.path ?? "",
    },
  };
}
