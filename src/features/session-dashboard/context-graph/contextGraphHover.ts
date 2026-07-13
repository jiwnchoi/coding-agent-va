import type { ContextGraphEdge } from "./types";

export type ContextGraphHoverIndex = {
  edgesByNodeId: Map<string, ContextGraphEdge[]>;
  impactNodeIdsByNodeId: Map<string, Set<string>>;
  parentByChildId: Map<string, string>;
};

export function buildContextGraphHoverIndex(
  containsEdges: ContextGraphEdge[],
  impactEdges: ContextGraphEdge[]
): ContextGraphHoverIndex {
  const edgesByNodeId = new Map<string, ContextGraphEdge[]>();
  const impactNodeIdsByNodeId = new Map<string, Set<string>>();
  const parentByChildId = new Map<string, string>();

  for (const edge of containsEdges) {
    parentByChildId.set(edge.target, edge.source);
    addEdgeByNodeId(edgesByNodeId, edge);
  }

  for (const edge of impactEdges) {
    addEdgeByNodeId(edgesByNodeId, edge);
    addRelatedNodeId(impactNodeIdsByNodeId, edge.source, edge.target);
    addRelatedNodeId(impactNodeIdsByNodeId, edge.target, edge.source);
  }

  return { edgesByNodeId, impactNodeIdsByNodeId, parentByChildId };
}

/**
 * Returns the hovered node, its impact-related files, and the hierarchy
 * ancestors needed to keep those files in context.
 */
export function collectHoverRelatedNodeIds(hoveredNodeId: string, index: ContextGraphHoverIndex) {
  const relatedNodeIds = new Set([
    hoveredNodeId,
    ...(index.impactNodeIdsByNodeId.get(hoveredNodeId) ?? []),
  ]);

  for (const nodeId of relatedNodeIds) {
    let parentId = index.parentByChildId.get(nodeId);

    while (parentId) {
      relatedNodeIds.add(parentId);
      parentId = index.parentByChildId.get(parentId);
    }
  }

  return relatedNodeIds;
}

export function collectHoverRelatedEdgeIds(
  relatedNodeIds: Set<string>,
  index: ContextGraphHoverIndex
) {
  const relatedEdgeIds = new Set<string>();

  for (const nodeId of relatedNodeIds) {
    for (const edge of index.edgesByNodeId.get(nodeId) ?? []) {
      if (relatedNodeIds.has(edge.source) && relatedNodeIds.has(edge.target)) {
        relatedEdgeIds.add(edge.id);
      }
    }
  }

  return relatedEdgeIds;
}

function addEdgeByNodeId(edgesByNodeId: Map<string, ContextGraphEdge[]>, edge: ContextGraphEdge) {
  edgesByNodeId.set(edge.source, [...(edgesByNodeId.get(edge.source) ?? []), edge]);
  edgesByNodeId.set(edge.target, [...(edgesByNodeId.get(edge.target) ?? []), edge]);
}

function addRelatedNodeId(
  impactNodeIdsByNodeId: Map<string, Set<string>>,
  nodeId: string,
  relatedNodeId: string
) {
  const relatedNodeIds = impactNodeIdsByNodeId.get(nodeId) ?? new Set<string>();
  relatedNodeIds.add(relatedNodeId);
  impactNodeIdsByNodeId.set(nodeId, relatedNodeIds);
}
