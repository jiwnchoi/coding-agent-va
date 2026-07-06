import type { ContextGraphModel, ContextGraphNode } from "./types";

export type LayoutTreeNode = {
  graphNode: ContextGraphNode | null;
  children: LayoutTreeNode[];
};

export function buildLayoutTree(model: ContextGraphModel): LayoutTreeNode {
  const nodeById = new Map(model.nodes.map((node) => [node.id, node]));
  const parentIdByChildId = new Map<string, string>();
  const childrenByParentId = new Map<string, ContextGraphNode[]>();

  for (const edge of model.containsEdges) {
    const parent = nodeById.get(edge.source);
    const child = nodeById.get(edge.target);

    if (!parent || !child) {
      continue;
    }

    parentIdByChildId.set(child.id, parent.id);
    childrenByParentId.set(parent.id, [...(childrenByParentId.get(parent.id) ?? []), child]);
  }

  const roots = model.nodes
    .filter((node) => !parentIdByChildId.has(node.id))
    .sort(compareFileTreeNodes);

  if (roots.length === 1) {
    return toLayoutTreeNode(roots[0], childrenByParentId);
  }

  return {
    graphNode: null,
    children: roots.map((node) => toLayoutTreeNode(node, childrenByParentId)),
  };
}

export function normalizePositions(positionByNodeId: Map<string, { x: number; y: number }>) {
  const positions = [...positionByNodeId.values()];
  const minX = Math.min(...positions.map((position) => position.x));
  const minY = Math.min(...positions.map((position) => position.y));
  const normalizedPositionByNodeId = new Map<string, { x: number; y: number }>();

  for (const [nodeId, position] of positionByNodeId) {
    normalizedPositionByNodeId.set(nodeId, {
      x: position.x - minX,
      y: position.y - minY,
    });
  }

  return normalizedPositionByNodeId;
}

function toLayoutTreeNode(
  graphNode: ContextGraphNode,
  childrenByParentId: Map<string, ContextGraphNode[]>
): LayoutTreeNode {
  return {
    graphNode,
    children: [...(childrenByParentId.get(graphNode.id) ?? [])]
      .sort(compareFileTreeNodes)
      .map((child) => toLayoutTreeNode(child, childrenByParentId)),
  };
}

function compareFileTreeNodes(left: ContextGraphNode, right: ContextGraphNode) {
  const kindComparison = nodeKindRank(left) - nodeKindRank(right);

  if (kindComparison !== 0) {
    return kindComparison;
  }

  return left.data.displayPath.localeCompare(right.data.displayPath, undefined, {
    numeric: true,
    sensitivity: "base",
  });
}

function nodeKindRank(node: ContextGraphNode) {
  switch (node.data.kind) {
    case "repo":
      return 0;
    case "directory":
      return 1;
    case "file":
      return 2;
  }
}
