import { EDGE_COLLISION_PADDING, NODE_HEIGHT, NODE_WIDTH } from "./layoutConstants";
import type { ContextGraphNode } from "./types";

export type LayoutPoint = { x: number; y: number };

export function contextGraphBounds(nodes: ContextGraphNode[]) {
  const minX = Math.min(...nodes.map((node) => node.position.x));
  const minY = Math.min(...nodes.map((node) => node.position.y));
  const maxX = Math.max(
    ...nodes.map((node) => node.position.x + numericDimension(node.style?.width, NODE_WIDTH))
  );
  const maxY = Math.max(
    ...nodes.map((node) => node.position.y + numericDimension(node.style?.height, NODE_HEIGHT))
  );

  return { x: minX, y: minY, width: maxX - minX, height: maxY - minY };
}

export function nodeCenter(position: LayoutPoint) {
  return { x: position.x + NODE_WIDTH / 2, y: position.y + NODE_HEIGHT / 2 };
}

export function lineIntersectsNode(
  source: LayoutPoint,
  target: LayoutPoint,
  position: LayoutPoint
) {
  const left = position.x - EDGE_COLLISION_PADDING;
  const right = position.x + NODE_WIDTH + EDGE_COLLISION_PADDING;
  const top = position.y - EDGE_COLLISION_PADDING;
  const bottom = position.y + NODE_HEIGHT + EDGE_COLLISION_PADDING;

  if (
    Math.max(source.x, target.x) < left ||
    Math.min(source.x, target.x) > right ||
    Math.max(source.y, target.y) < top ||
    Math.min(source.y, target.y) > bottom
  ) {
    return false;
  }

  return (
    pointInRect(source, left, right, top, bottom) ||
    pointInRect(target, left, right, top, bottom) ||
    segmentsIntersect(source, target, { x: left, y: top }, { x: right, y: top }) ||
    segmentsIntersect(source, target, { x: right, y: top }, { x: right, y: bottom }) ||
    segmentsIntersect(source, target, { x: right, y: bottom }, { x: left, y: bottom }) ||
    segmentsIntersect(source, target, { x: left, y: bottom }, { x: left, y: top })
  );
}

export function segmentsIntersect(
  firstStart: LayoutPoint,
  firstEnd: LayoutPoint,
  secondStart: LayoutPoint,
  secondEnd: LayoutPoint
) {
  const firstDirection = orientation(firstStart, firstEnd, secondStart);
  const secondDirection = orientation(firstStart, firstEnd, secondEnd);
  const thirdDirection = orientation(secondStart, secondEnd, firstStart);
  const fourthDirection = orientation(secondStart, secondEnd, firstEnd);

  return firstDirection * secondDirection < 0 && thirdDirection * fourthDirection < 0;
}

function pointInRect(point: LayoutPoint, left: number, right: number, top: number, bottom: number) {
  return point.x >= left && point.x <= right && point.y >= top && point.y <= bottom;
}

function orientation(firstPoint: LayoutPoint, secondPoint: LayoutPoint, thirdPoint: LayoutPoint) {
  return (
    (secondPoint.y - firstPoint.y) * (thirdPoint.x - secondPoint.x) -
    (secondPoint.x - firstPoint.x) * (thirdPoint.y - secondPoint.y)
  );
}

function numericDimension(value: string | number | undefined, fallback: number) {
  const dimension = typeof value === "number" ? value : Number.parseFloat(value ?? "");
  return Number.isFinite(dimension) ? dimension : fallback;
}
