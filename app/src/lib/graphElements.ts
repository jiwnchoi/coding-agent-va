import type { Edge, Node } from "@xyflow/react";

import type { ArchitectureNode, SessionVisualizationSnapshot } from "@/types/visualization";

export type GraphNodeData = {
  label: string;
  kind: string;
  path?: string;
  focusScore: number;
  selected: boolean;
  pinned: boolean;
};

export type GraphElements = {
  nodes: Node<GraphNodeData>[];
  edges: Edge[];
};

const graphNodeColors: Record<string, string> = {
  repo: "#111827",
  directory: "#2563eb",
  file: "#0891b2",
  symbol: "#7c3aed",
  external: "#b45309",
};

export function buildGraphElements(
  snapshot: SessionVisualizationSnapshot,
  highlightedNodeIds: Set<string>,
  pinnedNodeIds: Set<string>
): GraphElements {
  const nodeById = new Map(snapshot.graph.nodes.map((node) => [node.id, node]));
  const focusByNodeId = new Map<string, number>();
  for (const signal of snapshot.focusSignals) {
    if (signal.nodeId) {
      focusByNodeId.set(
        signal.nodeId,
        Math.max(focusByNodeId.get(signal.nodeId) ?? 0, signal.score)
      );
    }
  }

  const nodes = snapshot.graph.nodes
    .filter((node) => shouldShowNode(node, highlightedNodeIds, pinnedNodeIds, focusByNodeId))
    .slice(0, 80)
    .map((node): Node<GraphNodeData> => {
      const lane = laneForNode(node.kind);
      const y = 70 + lane.index * 34 + indexInLane(snapshot.graph.nodes, node, node.kind) * 92;
      const focusScore = focusByNodeId.get(node.id) ?? 0;
      const selected = highlightedNodeIds.has(node.id);
      const pinned = pinnedNodeIds.has(node.id);
      const color = graphNodeColors[node.kind] ?? "#4b5563";

      return {
        id: node.id,
        type: "default",
        position: { x: lane.x, y },
        data: {
          label: node.label,
          kind: node.kind,
          path: node.path,
          focusScore,
          selected,
          pinned,
        },
        style: {
          width: node.kind === "repo" ? 170 : 190,
          minHeight: 48,
          borderRadius: 8,
          border: selected
            ? `2px solid ${color}`
            : `1px solid ${focusScore > 0 ? color : "#d1d5db"}`,
          background: selected ? "#ecfeff" : pinned ? "#fffbeb" : "#ffffff",
          boxShadow: focusScore > 0.75 ? "0 8px 22px rgba(8, 145, 178, 0.14)" : "none",
          color: "#111827",
          fontSize: 12,
        },
      };
    });

  const visibleNodeIds = new Set(nodes.map((node) => node.id));
  const edges = snapshot.graph.edges
    .filter((edge) => visibleNodeIds.has(edge.source) && visibleNodeIds.has(edge.target))
    .slice(0, 120)
    .map((edge): Edge => {
      const source = nodeById.get(edge.source);
      const color = source ? (graphNodeColors[source.kind] ?? "#94a3b8") : "#94a3b8";
      return {
        id: edge.id,
        source: edge.source,
        target: edge.target,
        label: edge.kind === "imports" ? edge.label : undefined,
        animated: edge.kind === "imports",
        style: {
          stroke: edge.kind === "contains" ? "#cbd5e1" : color,
          strokeWidth: edge.kind === "contains" ? 1 : 1.8,
        },
      };
    });

  return { nodes, edges };
}

function shouldShowNode(
  node: ArchitectureNode,
  highlightedNodeIds: Set<string>,
  pinnedNodeIds: Set<string>,
  focusByNodeId: Map<string, number>
) {
  if (node.kind === "repo" || node.kind === "directory") {
    return true;
  }
  return (
    highlightedNodeIds.has(node.id) ||
    pinnedNodeIds.has(node.id) ||
    (focusByNodeId.get(node.id) ?? 0) > 0
  );
}

function laneForNode(kind: string) {
  switch (kind) {
    case "repo":
      return { x: 40, index: 0 };
    case "directory":
      return { x: 270, index: 0 };
    case "file":
      return { x: 520, index: 0 };
    case "symbol":
      return { x: 780, index: 0 };
    default:
      return { x: 780, index: 3 };
  }
}

function indexInLane(nodes: ArchitectureNode[], node: ArchitectureNode, kind: string) {
  return nodes
    .filter((candidate) => candidate.kind === kind)
    .findIndex((candidate) => candidate.id === node.id);
}
