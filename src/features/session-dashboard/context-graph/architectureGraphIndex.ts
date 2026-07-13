import { displayPathForNode, normalizeWorkspacePath } from "./contextGraphPaths";
import { isVisibleGraphNode } from "./contextGraphVisibility";
import type { ArchitectureEdge, ArchitectureGraph, ArchitectureNode } from "./types";

export type ArchitectureGraphIndex = {
  containsEdges: ArchitectureEdge[];
  fileNodeIdByPathKey: Map<string, string>;
  nodeById: Map<string, ArchitectureNode>;
  parentByChildId: Map<string, string>;
  visibleNodes: ArchitectureNode[];
  workspacePath: string;
};

const indexes = new WeakMap<ArchitectureGraph, ArchitectureGraphIndex>();

export function getArchitectureGraphIndex(
  architectureGraph: ArchitectureGraph,
  workspacePath: string
) {
  const cached = indexes.get(architectureGraph);
  if (cached?.workspacePath === workspacePath) return cached;

  const containsEdges = architectureGraph.edges.filter((edge) => edge.kind === "contains");
  const visibleNodes = architectureGraph.nodes.filter(isVisibleGraphNode);
  const fileNodeIdByPathKey = new Map<string, string>();
  for (const node of visibleNodes) {
    if (node.kind !== "file" || !node.path) continue;
    fileNodeIdByPathKey.set(displayPathForNode(node, workspacePath), node.id);
    fileNodeIdByPathKey.set(normalizeWorkspacePath(node.path, workspacePath), node.id);
  }
  const index = {
    containsEdges,
    fileNodeIdByPathKey,
    nodeById: new Map(architectureGraph.nodes.map((node) => [node.id, node])),
    parentByChildId: new Map(containsEdges.map((edge) => [edge.target, edge.source])),
    visibleNodes,
    workspacePath,
  };
  indexes.set(architectureGraph, index);
  return index;
}
