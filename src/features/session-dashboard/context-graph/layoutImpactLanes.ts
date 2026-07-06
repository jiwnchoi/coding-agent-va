import { type EdgeSide, IMPACT_EDGE_LANE_OFFSET_STEP } from "./layoutConstants";
import { toNodeCenter } from "./layoutHandles";
import type { ContextGraphEdgeData, ContextGraphModel } from "./types";

export function distributeImpactEdgeTargets(
  edges: ContextGraphModel["impactEdges"],
  positionById: Map<string, { x: number; y: number }>
) {
  const edgesByTargetHandle = new Map<string, ContextGraphModel["impactEdges"]>();

  for (const edge of edges) {
    const key = `${edge.target}:${edge.targetHandle ?? ""}`;
    edgesByTargetHandle.set(key, [...(edgesByTargetHandle.get(key) ?? []), edge]);
  }

  const laneOffsetByEdgeId = new Map<
    string,
    {
      sourceOffset?: { x: number; y: number };
      targetOffset?: { x: number; y: number };
    }
  >();

  for (const targetEdges of edgesByTargetHandle.values()) {
    if (targetEdges.length < 2) {
      continue;
    }

    const targetSide = sideFromHandleId(targetEdges[0].targetHandle);

    if (!targetSide) {
      continue;
    }

    const sortedTargetEdges = [...targetEdges].sort((left, right) => {
      const leftCenter = centerForEdgeSource(left, positionById);
      const rightCenter = centerForEdgeSource(right, positionById);

      return compareSourceCentersForTargetSide(targetSide, leftCenter, rightCenter);
    });

    for (const [index, edge] of sortedTargetEdges.entries()) {
      const offset = (index - (sortedTargetEdges.length - 1) / 2) * IMPACT_EDGE_LANE_OFFSET_STEP;
      const sourceSide = sideFromHandleId(edge.sourceHandle);

      laneOffsetByEdgeId.set(edge.id, {
        sourceOffset: sourceSide ? offsetAlongSide(sourceSide, offset) : undefined,
        targetOffset: offsetAlongSide(targetSide, offset),
      });
    }
  }

  return edges.map((edge) => {
    const edgeData: ContextGraphEdgeData = edge.data ?? {
      kind: "impact",
      isHighlighted: false,
    };

    return {
      ...edge,
      data: {
        ...edgeData,
        sourceOffset: laneOffsetByEdgeId.get(edge.id)?.sourceOffset,
        targetOffset: laneOffsetByEdgeId.get(edge.id)?.targetOffset,
      },
    };
  });
}

function compareSourceCentersForTargetSide(
  targetSide: EdgeSide,
  leftCenter: { x: number; y: number },
  rightCenter: { x: number; y: number }
) {
  if (targetSide === "left" || targetSide === "right") {
    return leftCenter.y - rightCenter.y || leftCenter.x - rightCenter.x;
  }

  return leftCenter.x - rightCenter.x || leftCenter.y - rightCenter.y;
}

function centerForEdgeSource(
  edge: ContextGraphModel["impactEdges"][number],
  positionById: Map<string, { x: number; y: number }>
) {
  const sourcePosition = positionById.get(edge.source);

  return sourcePosition ? toNodeCenter(sourcePosition) : { x: 0, y: 0 };
}

function sideFromHandleId(handleIdValue?: string | null): EdgeSide | null {
  if (!handleIdValue) {
    return null;
  }

  const side = handleIdValue.replace(/^(source|target)-/, "");

  return isEdgeSide(side) ? side : null;
}

function isEdgeSide(value: string): value is EdgeSide {
  return value === "top" || value === "right" || value === "bottom" || value === "left";
}

function offsetAlongSide(side: EdgeSide, offset: number) {
  if (side === "left" || side === "right") {
    return { x: 0, y: offset };
  }

  return { x: offset, y: 0 };
}
