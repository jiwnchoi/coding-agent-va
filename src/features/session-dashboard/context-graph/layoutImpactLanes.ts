import {
  type EdgeSide,
  IMPACT_EDGE_LANE_OFFSET_STEP,
  NODE_HEIGHT,
  NODE_WIDTH,
} from "./layoutConstants";
import { toNodeCenter } from "./layoutHandles";
import type { ContextGraphEdgeData, ContextGraphModel } from "./types";

type EdgeEndpoint = "source" | "target";
type EdgeOffsets = {
  sourceOffset?: { x: number; y: number };
  targetOffset?: { x: number; y: number };
};

export function distributeImpactEdgeLanes(
  edges: ContextGraphModel["impactEdges"],
  positionById: Map<string, { x: number; y: number }>
) {
  const laneOffsetByEdgeId = new Map<string, EdgeOffsets>();
  distributeEndpoint(edges, positionById, laneOffsetByEdgeId, "source");
  distributeEndpoint(edges, positionById, laneOffsetByEdgeId, "target");

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

function distributeEndpoint(
  edges: ContextGraphModel["impactEdges"],
  positionById: Map<string, { x: number; y: number }>,
  laneOffsetByEdgeId: Map<string, EdgeOffsets>,
  endpoint: EdgeEndpoint
) {
  const edgesByHandle = new Map<string, ContextGraphModel["impactEdges"]>();
  for (const edge of edges) {
    const key = `${edge[endpoint]}:${edge[`${endpoint}Handle`] ?? ""}`;
    edgesByHandle.set(key, [...(edgesByHandle.get(key) ?? []), edge]);
  }

  for (const groupedEdges of edgesByHandle.values()) {
    const side = sideFromHandleId(groupedEdges[0]?.[`${endpoint}Handle`]);
    if (groupedEdges.length < 2 || !side) {
      continue;
    }

    const oppositeEndpoint = endpoint === "source" ? "target" : "source";
    const sortedEdges = [...groupedEdges].sort((left, right) =>
      compareCentersForSide(
        side,
        centerForEndpoint(left, oppositeEndpoint, positionById),
        centerForEndpoint(right, oppositeEndpoint, positionById)
      )
    );

    for (const [index, edge] of sortedEdges.entries()) {
      const endpointOffset = offsetAlongSide(side, laneOffset(side, index, sortedEdges.length));
      const oppositeSide = sideFromHandleId(edge[`${oppositeEndpoint}Handle`]);
      const oppositeOffset = oppositeSide
        ? offsetAlongSide(oppositeSide, laneOffset(oppositeSide, index, sortedEdges.length))
        : undefined;
      const previousOffsets = laneOffsetByEdgeId.get(edge.id);

      laneOffsetByEdgeId.set(edge.id, {
        ...previousOffsets,
        [`${oppositeEndpoint}Offset`]:
          previousOffsets?.[`${oppositeEndpoint}Offset`] ?? oppositeOffset,
        [`${endpoint}Offset`]: endpointOffset,
      });
    }
  }
}

function laneOffset(side: EdgeSide, index: number, laneCount: number) {
  const borderLength = side === "left" || side === "right" ? NODE_HEIGHT : NODE_WIDTH;
  const cornerClearance = side === "left" || side === "right" ? 12 : 16;
  const maximumOffset = borderLength / 2 - cornerClearance;
  const step = Math.min(
    IMPACT_EDGE_LANE_OFFSET_STEP,
    laneCount > 1 ? (maximumOffset * 2) / (laneCount - 1) : 0
  );

  return (index - (laneCount - 1) / 2) * step;
}

function compareCentersForSide(
  side: EdgeSide,
  leftCenter: { x: number; y: number },
  rightCenter: { x: number; y: number }
) {
  if (side === "left" || side === "right") {
    return leftCenter.y - rightCenter.y || leftCenter.x - rightCenter.x;
  }

  return leftCenter.x - rightCenter.x || leftCenter.y - rightCenter.y;
}

function centerForEndpoint(
  edge: ContextGraphModel["impactEdges"][number],
  endpoint: EdgeEndpoint,
  positionById: Map<string, { x: number; y: number }>
) {
  const position = positionById.get(edge[endpoint]);

  return position ? toNodeCenter(position) : { x: 0, y: 0 };
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
