import { hierarchy, tree } from "d3-hierarchy";

import type { ContextGraphModel, ContextGraphNode } from "./types";

const NODE_WIDTH = 220;
const NODE_HEIGHT = 72;
const HORIZONTAL_GAP = 96;
const VERTICAL_GAP = 42;

type LayoutTreeNode = {
  graphNode: ContextGraphNode | null;
  children: LayoutTreeNode[];
};

export function layoutContextGraphWithHierarchy(model: ContextGraphModel) {
  if (model.nodes.length === 0) {
    return model;
  }

  const layoutTree = buildLayoutTree(model);
  const root = hierarchy(layoutTree);
  const layout = tree<LayoutTreeNode>().nodeSize([
    NODE_HEIGHT + VERTICAL_GAP,
    NODE_WIDTH + HORIZONTAL_GAP,
  ]);
  const positionedRoot = layout(root);
  const positionByNodeId = new Map<string, { x: number; y: number }>();

  for (const hierarchyNode of positionedRoot.descendants()) {
    const graphNode = hierarchyNode.data.graphNode;

    if (!graphNode) {
      continue;
    }

    positionByNodeId.set(graphNode.id, {
      x: hierarchyNode.y,
      y: hierarchyNode.x,
    });
  }

  const normalizedPositionByNodeId = normalizePositions(positionByNodeId);

  return {
    ...model,
    nodes: model.nodes.map((node) => ({
      ...node,
      position: normalizedPositionByNodeId.get(node.id) ?? node.position,
    })),
  };
}

function buildLayoutTree(model: ContextGraphModel): LayoutTreeNode {
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

function normalizePositions(positionByNodeId: Map<string, { x: number; y: number }>) {
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
