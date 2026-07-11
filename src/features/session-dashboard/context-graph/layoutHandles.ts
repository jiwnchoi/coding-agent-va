import {
  EDGE_COLLISION_PADDING,
  EDGE_COLLISION_PENALTY,
  EDGE_SIDES,
  type EdgeSide,
  HANDLE_PREFIXES,
  NODE_HEIGHT,
  NODE_WIDTH,
} from "./layoutConstants";
import type { ContextGraphModel, ContextGraphNode } from "./types";

export function assignClosestEdgeHandles(edge: ContextGraphModel["containsEdges"][number]) {
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
  return {
    x: position.x + NODE_WIDTH / 2,
    y: position.y + NODE_HEIGHT / 2,
  };
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

function lineIntersectsNode(
  sourcePoint: { x: number; y: number },
  targetPoint: { x: number; y: number },
  nodePosition: { x: number; y: number }
) {
  const left = nodePosition.x - EDGE_COLLISION_PADDING;
  const right = nodePosition.x + NODE_WIDTH + EDGE_COLLISION_PADDING;
  const top = nodePosition.y - EDGE_COLLISION_PADDING;
  const bottom = nodePosition.y + NODE_HEIGHT + EDGE_COLLISION_PADDING;

  if (
    Math.max(sourcePoint.x, targetPoint.x) < left ||
    Math.min(sourcePoint.x, targetPoint.x) > right ||
    Math.max(sourcePoint.y, targetPoint.y) < top ||
    Math.min(sourcePoint.y, targetPoint.y) > bottom
  ) {
    return false;
  }

  return (
    pointInRect(sourcePoint, left, right, top, bottom) ||
    pointInRect(targetPoint, left, right, top, bottom) ||
    segmentsIntersect(sourcePoint, targetPoint, { x: left, y: top }, { x: right, y: top }) ||
    segmentsIntersect(sourcePoint, targetPoint, { x: right, y: top }, { x: right, y: bottom }) ||
    segmentsIntersect(sourcePoint, targetPoint, { x: right, y: bottom }, { x: left, y: bottom }) ||
    segmentsIntersect(sourcePoint, targetPoint, { x: left, y: bottom }, { x: left, y: top })
  );
}

function pointInRect(
  point: { x: number; y: number },
  left: number,
  right: number,
  top: number,
  bottom: number
) {
  return point.x >= left && point.x <= right && point.y >= top && point.y <= bottom;
}

function segmentsIntersect(
  firstStart: { x: number; y: number },
  firstEnd: { x: number; y: number },
  secondStart: { x: number; y: number },
  secondEnd: { x: number; y: number }
) {
  const firstDirection = orientation(firstStart, firstEnd, secondStart);
  const secondDirection = orientation(firstStart, firstEnd, secondEnd);
  const thirdDirection = orientation(secondStart, secondEnd, firstStart);
  const fourthDirection = orientation(secondStart, secondEnd, firstEnd);

  return firstDirection * secondDirection < 0 && thirdDirection * fourthDirection < 0;
}

function orientation(
  firstPoint: { x: number; y: number },
  secondPoint: { x: number; y: number },
  thirdPoint: { x: number; y: number }
) {
  return (
    (secondPoint.y - firstPoint.y) * (thirdPoint.x - secondPoint.x) -
    (secondPoint.x - firstPoint.x) * (thirdPoint.y - secondPoint.y)
  );
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
