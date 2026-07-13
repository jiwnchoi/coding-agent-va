import type { ArchitectureGraph, ArchitectureNode } from "./types";

export function collectVisibleNodeIds({
  activeFilePaths,
  architectureGraph,
  fileNodeIdByPathKey,
  includeEntireWorkspace,
  parentByChildId,
}: {
  activeFilePaths: Set<string>;
  architectureGraph: ArchitectureGraph;
  fileNodeIdByPathKey: Map<string, string>;
  includeEntireWorkspace: boolean;
  parentByChildId: Map<string, string>;
}) {
  const visibleNodeIds = new Set<string>();

  if (includeEntireWorkspace) {
    for (const node of architectureGraph.nodes) {
      if (isIndexableGraphNode(node)) {
        visibleNodeIds.add(node.id);
      }
    }
    return visibleNodeIds;
  }

  for (const filePath of activeFilePaths) {
    const fileNodeId = fileNodeIdByPathKey.get(filePath);

    if (!fileNodeId) {
      continue;
    }

    let currentNodeId: string | undefined = fileNodeId;
    while (currentNodeId) {
      visibleNodeIds.add(currentNodeId);
      currentNodeId = parentByChildId.get(currentNodeId);
    }
  }

  if (visibleNodeIds.size === 0) {
    for (const node of architectureGraph.nodes) {
      if (node.kind === "repo") {
        visibleNodeIds.add(node.id);
      }
    }
  }

  return visibleNodeIds;
}

export function isVisibleGraphNode(node: ArchitectureNode) {
  return isRenderableNode(node);
}

function isRenderableNode(node: ArchitectureNode) {
  return node.kind === "directory" || node.kind === "file";
}

function isIndexableGraphNode(node: ArchitectureNode) {
  return node.kind === "repo" || isRenderableNode(node);
}
