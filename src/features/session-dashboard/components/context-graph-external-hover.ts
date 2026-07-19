import { useCallback, useMemo } from "react";

import { useContextGraphHover } from "@/features/session-dashboard/components/context-graph-interaction";
import {
  collectHoverRelatedEdgeIds,
  collectHoverRelatedNodeIds,
  type ContextGraphHoverIndex,
} from "@/features/session-dashboard/context-graph/contextGraphHover";
import { normalizeWorkspacePath } from "@/features/session-dashboard/context-graph/contextGraphPaths";
import styles from "@/features/session-dashboard/context-graph/ContextGraphView.module.css";
import type {
  ContextGraphEdge,
  ContextGraphNode,
} from "@/features/session-dashboard/context-graph/types";

export function useExternalGraphState(
  nodes: ContextGraphNode[],
  edges: ContextGraphEdge[],
  hoveredFilePaths: string[] | null,
  workspacePath: string,
  hoverIndex: ContextGraphHoverIndex
) {
  const hoveredNode = useMemo(
    () => findHoveredGraphNode(nodes, hoveredFilePaths, workspacePath),
    [hoveredFilePaths, nodes, workspacePath]
  );
  return useMemo(
    () => buildExternalGraphHoverState(nodes, edges, hoveredFilePaths, hoveredNode, hoverIndex),
    [edges, hoveredFilePaths, hoveredNode, hoverIndex, nodes]
  );
}

function buildExternalGraphHoverState(
  nodes: ContextGraphNode[],
  edges: ContextGraphEdge[],
  hoveredFilePaths: string[] | null,
  hoveredNode: ContextGraphNode | null | undefined,
  hoverIndex: ContextGraphHoverIndex
) {
  if (hoveredFilePaths === null) return { nodes, edges };
  const relatedNodeIds = hoveredNode
    ? collectHoverRelatedNodeIds(hoveredNode.id, hoverIndex)
    : new Set<string>();
  const relatedEdgeIds = collectHoverRelatedEdgeIds(relatedNodeIds, hoverIndex);
  return {
    nodes: nodes.map((node) => ({
      ...node,
      className:
        `${node.className ?? ""} ${relatedNodeIds.has(node.id) ? styles.externalRelated : styles.externalDimmed}`.trim(),
      style: { ...node.style, opacity: relatedNodeIds.has(node.id) ? 1 : 0.3 },
    })),
    edges: edges.map((edge) => ({
      ...edge,
      className:
        `${edge.className ?? ""} ${relatedEdgeIds.has(edge.id) ? styles.externalRelated : styles.externalDimmed}`.trim(),
      style: { ...edge.style, opacity: relatedEdgeIds.has(edge.id) ? 1 : 0.3 },
    })),
  };
}

export function useGraphNodeHoverCallbacks(
  handleNodeMouseEnter: (event: React.MouseEvent, node: ContextGraphNode) => void,
  handleNodeMouseLeave: () => void,
  onGraphHoverFilePaths: (filePaths: string[] | null) => void,
  nodes: ContextGraphNode[],
  hoverIndex: ContextGraphHoverIndex
) {
  const handleGraphNodeMouseEnter = useCallback(
    (event: React.MouseEvent, node: ContextGraphNode) => {
      handleNodeMouseEnter(event, node);
      if (node.data.kind !== "file" || node.data.activities.length === 0) {
        onGraphHoverFilePaths(null);
        return;
      }
      const relatedNodeIds = collectHoverRelatedNodeIds(node.id, hoverIndex);
      onGraphHoverFilePaths([
        node.data.displayPath,
        ...nodes
          .filter(
            (relatedNode) =>
              relatedNode.id !== node.id &&
              relatedNode.data.kind === "file" &&
              relatedNodeIds.has(relatedNode.id)
          )
          .map((relatedNode) => relatedNode.data.displayPath),
      ]);
    },
    [handleNodeMouseEnter, hoverIndex, nodes, onGraphHoverFilePaths]
  );
  const handleGraphNodeMouseLeave = useCallback(() => {
    handleNodeMouseLeave();
    onGraphHoverFilePaths(null);
  }, [handleNodeMouseLeave, onGraphHoverFilePaths]);
  return { handleGraphNodeMouseEnter, handleGraphNodeMouseLeave };
}

export function useSessionGraphHover(
  shellRef: React.RefObject<HTMLDivElement | null>,
  hoverIndex: ContextGraphHoverIndex,
  isPanningRef: React.MutableRefObject<boolean>,
  onGraphHoverFilePaths: (filePaths: string[] | null) => void,
  nodes: ContextGraphNode[]
) {
  const { handleNodeMouseEnter, handleNodeMouseLeave, pinHover, releaseHover } =
    useContextGraphHover(shellRef, hoverIndex, isPanningRef);
  return {
    ...useGraphNodeHoverCallbacks(
      handleNodeMouseEnter,
      handleNodeMouseLeave,
      onGraphHoverFilePaths,
      nodes,
      hoverIndex
    ),
    pinHover,
    releaseHover,
  };
}

export function findHoveredGraphNode(
  nodes: ContextGraphNode[],
  hoveredFilePaths: string[] | null,
  workspacePath: string
) {
  if (!hoveredFilePaths || hoveredFilePaths.length === 0) return undefined;
  const normalizedPath = normalizeWorkspacePath(hoveredFilePaths[0], workspacePath);
  return (
    nodes.find(
      (node) =>
        node.data.kind === "file" &&
        normalizeWorkspacePath(node.data.displayPath, workspacePath) === normalizedPath
    ) ?? null
  );
}
