import { hierarchy, tree } from "d3-hierarchy";

import { HORIZONTAL_GAP, NODE_HEIGHT, NODE_WIDTH, VERTICAL_GAP } from "./layoutConstants";
import { assignClosestEdgeHandles, assignImpactEdgeHandles } from "./layoutHandles";
import { distributeImpactEdgeTargets } from "./layoutImpactLanes";
import { buildLayoutTree, type LayoutTreeNode, normalizePositions } from "./layoutTree";
import type { ContextGraphModel } from "./types";

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
  const nodes = model.nodes.map((node) => ({
    ...node,
    position: normalizedPositionByNodeId.get(node.id) ?? node.position,
  }));
  const positionById = new Map(nodes.map((node) => [node.id, node.position]));

  return {
    ...model,
    nodes,
    containsEdges: model.containsEdges.map((edge) => assignClosestEdgeHandles(edge, positionById)),
    impactEdges: distributeImpactEdgeTargets(
      model.impactEdges.map((edge) => assignImpactEdgeHandles(edge, positionById, nodes)),
      positionById
    ),
  };
}
