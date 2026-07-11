import {
  EDGE_COLLISION_PENALTY,
  EDGE_SIDES,
  type EdgeSide,
  HANDLE_PREFIXES,
  NODE_HEIGHT,
  NODE_WIDTH,
} from "./layoutConstants";
import { lineIntersectsNode, nodeCenter } from "./layoutGeometry";
import type { ContextGraphModel, ContextGraphNode } from "./types";

export function assignContainsEdgeHandles(edge: ContextGraphModel["containsEdges"][number]) {
  return {
    ...edge,
    sourceHandle: handleId(HANDLE_PREFIXES.source, "right"),
    targetHandle: handleId(HANDLE_PREFIXES.target, "left"),
  };
}

export function assignImpactEdgeHandles(
  edge: ContextGraphModel["impactEdges"][number],
  positionById: Map<string, { x: number; y: number }>,
  nodes: ContextGraphNode[]
) {
  const sourcePosition = positionById.get(edge.source);
  const targetPosition = positionById.get(edge.target);

  if (!sourcePosition || !targetPosition) {
    return edge;
  }

  let bestCandidate: {
    score: number;
    sourceSide: EdgeSide;
    targetSide: EdgeSide;
  } | null = null;

  for (const sourceSide of EDGE_SIDES) {
    for (const targetSide of EDGE_SIDES) {
      const sourcePoint = sidePoint(sourcePosition, sourceSide);
      const targetPoint = sidePoint(targetPosition, targetSide);
      const collisionCount = countLineNodeCollisions(sourcePoint, targetPoint, nodes, edge);
      const directionalPenalty = sideDirectionPenalty(
        sourceSide,
        targetSide,
        sourcePoint,
        targetPoint
      );
      const score =
        Math.hypot(targetPoint.x - sourcePoint.x, targetPoint.y - sourcePoint.y) +
        collisionCount * EDGE_COLLISION_PENALTY +
        directionalPenalty;

      if (!bestCandidate || score < bestCandidate.score) {
        bestCandidate = {
          score,
          sourceSide,
          targetSide,
        };
      }
    }
  }

  if (!bestCandidate) {
    return edge;
  }

  return {
    ...edge,
    sourceHandle: handleId(HANDLE_PREFIXES.source, bestCandidate.sourceSide),
    targetHandle: handleId(HANDLE_PREFIXES.target, bestCandidate.targetSide),
  };
}

function sidePoint(position: { x: number; y: number }, side: EdgeSide) {
  switch (side) {
    case "top":
      return { x: position.x + NODE_WIDTH / 2, y: position.y };
    case "right":
      return { x: position.x + NODE_WIDTH, y: position.y + NODE_HEIGHT / 2 };
    case "bottom":
      return { x: position.x + NODE_WIDTH / 2, y: position.y + NODE_HEIGHT };
    case "left":
      return { x: position.x, y: position.y + NODE_HEIGHT / 2 };
  }
}

export function toNodeCenter(position: { x: number; y: number }) {
  return nodeCenter(position);
}

function countLineNodeCollisions(
  sourcePoint: { x: number; y: number },
  targetPoint: { x: number; y: number },
  nodes: ContextGraphNode[],
  edge: ContextGraphModel["impactEdges"][number]
) {
  let collisionCount = 0;

  for (const node of nodes) {
    if (node.id === edge.source || node.id === edge.target) {
      continue;
    }

    if (lineIntersectsNode(sourcePoint, targetPoint, node.position)) {
      collisionCount += 1;
    }
  }

  return collisionCount;
}

function sideDirectionPenalty(
  sourceSide: EdgeSide,
  targetSide: EdgeSide,
  sourcePoint: { x: number; y: number },
  targetPoint: { x: number; y: number }
) {
  const deltaX = targetPoint.x - sourcePoint.x;
  const deltaY = targetPoint.y - sourcePoint.y;
  let penalty = 0;

  if ((sourceSide === "right" && deltaX < 0) || (sourceSide === "left" && deltaX > 0)) {
    penalty += 80;
  }

  if ((sourceSide === "bottom" && deltaY < 0) || (sourceSide === "top" && deltaY > 0)) {
    penalty += 80;
  }

  if ((targetSide === "right" && deltaX > 0) || (targetSide === "left" && deltaX < 0)) {
    penalty += 80;
  }

  if ((targetSide === "bottom" && deltaY > 0) || (targetSide === "top" && deltaY < 0)) {
    penalty += 80;
  }

  return penalty;
}

function handleId(prefix: (typeof HANDLE_PREFIXES)[keyof typeof HANDLE_PREFIXES], side: EdgeSide) {
  return `${prefix}-${side}`;
}
