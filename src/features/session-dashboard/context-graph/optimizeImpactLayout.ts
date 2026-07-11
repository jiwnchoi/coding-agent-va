import {
  lineIntersectsNode,
  nodeCenter,
  segmentsIntersect,
  type LayoutPoint,
} from "./layoutGeometry";
import type { ContextGraphModel, ContextGraphNode } from "./types";

const NODE_COLLISION_COST = 10_000;
const EDGE_CROSSING_COST = 2_500;
const EDGE_LENGTH_COST = 0.01;
const MAX_OPTIMIZATION_PASSES = 6;

/**
 * Reorders files inside each directory without moving the directory hierarchy.
 * Pair swaps are accepted only when they reduce edge/node intersections, then
 * edge crossings, and finally total edge length.
 */
export function optimizeImpactLayout(
  nodes: ContextGraphNode[],
  impactEdges: ContextGraphModel["impactEdges"],
  filesByFolderId: Map<string, ContextGraphNode[]>
) {
  if (impactEdges.length === 0) {
    return nodes;
  }

  const positionById = new Map(nodes.map((node) => [node.id, { ...node.position }]));
  const fileNodes = nodes.filter((node) => node.data.kind === "file");
  let currentScore = scoreLayout(positionById, fileNodes, impactEdges);

  for (let pass = 0; pass < MAX_OPTIMIZATION_PASSES; pass += 1) {
    let bestSwap: { leftId: string; rightId: string; score: number } | null = null;

    for (const siblings of filesByFolderId.values()) {
      for (let leftIndex = 0; leftIndex < siblings.length - 1; leftIndex += 1) {
        for (let rightIndex = leftIndex + 1; rightIndex < siblings.length; rightIndex += 1) {
          const left = siblings[leftIndex];
          const right = siblings[rightIndex];
          swapPositions(positionById, left.id, right.id);
          const candidateScore = scoreLayout(positionById, fileNodes, impactEdges);
          swapPositions(positionById, left.id, right.id);

          if (candidateScore < (bestSwap?.score ?? currentScore)) {
            bestSwap = { leftId: left.id, rightId: right.id, score: candidateScore };
          }
        }
      }
    }

    if (!bestSwap) {
      break;
    }

    swapPositions(positionById, bestSwap.leftId, bestSwap.rightId);
    currentScore = bestSwap.score;
  }

  return nodes.map((node) => ({
    ...node,
    position: positionById.get(node.id) ?? node.position,
  }));
}

function scoreLayout(
  positionById: Map<string, LayoutPoint>,
  fileNodes: ContextGraphNode[],
  edges: ContextGraphModel["impactEdges"]
) {
  const segments = edges.flatMap((edge) => {
    const source = positionById.get(edge.source);
    const target = positionById.get(edge.target);

    return source && target
      ? [{ edge, source: nodeCenter(source), target: nodeCenter(target) }]
      : [];
  });
  let nodeCollisions = 0;
  let edgeCrossings = 0;
  let totalLength = 0;

  for (const segment of segments) {
    totalLength += Math.hypot(
      segment.target.x - segment.source.x,
      segment.target.y - segment.source.y
    );

    for (const node of fileNodes) {
      if (node.id === segment.edge.source || node.id === segment.edge.target) {
        continue;
      }

      const position = positionById.get(node.id);
      if (position && lineIntersectsNode(segment.source, segment.target, position)) {
        nodeCollisions += 1;
      }
    }
  }

  for (let leftIndex = 0; leftIndex < segments.length - 1; leftIndex += 1) {
    for (let rightIndex = leftIndex + 1; rightIndex < segments.length; rightIndex += 1) {
      const left = segments[leftIndex];
      const right = segments[rightIndex];
      const sharesEndpoint =
        left.edge.source === right.edge.source ||
        left.edge.source === right.edge.target ||
        left.edge.target === right.edge.source ||
        left.edge.target === right.edge.target;

      if (
        !sharesEndpoint &&
        segmentsIntersect(left.source, left.target, right.source, right.target)
      ) {
        edgeCrossings += 1;
      }
    }
  }

  return (
    nodeCollisions * NODE_COLLISION_COST +
    edgeCrossings * EDGE_CROSSING_COST +
    totalLength * EDGE_LENGTH_COST
  );
}

function swapPositions(positionById: Map<string, LayoutPoint>, leftId: string, rightId: string) {
  const left = positionById.get(leftId);
  const right = positionById.get(rightId);

  if (left && right) {
    positionById.set(leftId, right);
    positionById.set(rightId, left);
  }
}
