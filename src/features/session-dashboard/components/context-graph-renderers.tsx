import { BaseEdge, type EdgeProps, Handle, Position, type NodeProps } from "@xyflow/react";
import { memo } from "react";

import styles from "@/features/session-dashboard/context-graph/ContextGraphView.module.css";
import { fileIconForLanguage } from "@/features/session-dashboard/context-graph/fileIcon";
import type {
  ContextGraphEdge,
  ContextGraphNode,
  ContextGraphNodeData,
} from "@/features/session-dashboard/context-graph/types";
import type { ActivitySectionKey } from "@/features/session-dashboard/lib/session-watch";
import { cn } from "@/shared/lib/utils";

const EDGE_ENDPOINT_GAP = 1;
const IMPACT_EDGE_HEAD_LENGTH = 8;
const IMPACT_EDGE_HEAD_HALF_WIDTH = 4;
const handlePositions = [
  ["top", Position.Top],
  ["right", Position.Right],
  ["bottom", Position.Bottom],
  ["left", Position.Left],
] as const;

export const nodeTypes = { contextGraphNode: memo(ContextGraphNodeComponent) };
export const edgeTypes = {
  contextGraphContainsEdge: memo(ContextGraphContainsEdgeComponent),
  contextGraphImpactEdge: memo(ContextGraphImpactEdgeComponent),
};

export function GraphEmptyState({ title, description }: { title: string; description: string }) {
  return (
    <div className="absolute inset-0 z-[2] flex flex-col items-center justify-center p-8 text-center">
      <p className="text-sm font-medium">{title}</p>
      <p className="text-muted-foreground mt-1 max-w-md text-sm">{description}</p>
    </div>
  );
}

export function resolvePrimaryActivity(
  activities: ContextGraphNodeData["activities"]
): ActivitySectionKey {
  return activities[0] ?? "impacted";
}

function isHoverableNode(activities: ContextGraphNodeData["activities"]) {
  return activities.length > 0;
}
function primaryActivityClass(activities: ContextGraphNodeData["activities"]) {
  if (activities.length === 0) return "";

  switch (resolvePrimaryActivity(activities)) {
    case "edited":
      return styles.editedNode;
    case "deleted":
      return styles.deletedNode;
    case "impacted":
      return activities.includes("read") ? styles.readImpactedNode : styles.unreadImpactedNode;
    case "read":
      return "";
  }
}

function ContextGraphNodeComponent({ data }: NodeProps<ContextGraphNode>) {
  const isFile = data.kind === "file";
  return (
    <div
      className={cn(
        styles.node,
        "text-card-foreground border-border relative h-full w-full border",
        data.kind === "directory" && styles.directoryNode,
        data.kind === "directory" && !data.hasDirectFiles && styles.filelessDirectoryNode,
        isFile && "bg-card rounded-md px-2 py-1.5",
        isFile && isHoverableNode(data.activities) && styles.hoverableNode,
        isFile && primaryActivityClass(data.activities),
        isFile && data.isSelected && styles.selectedNode
      )}>
      {isFile ? (
        <>
          <>
            {handlePositions.map(([side, position]) => (
              <GraphHandle key={`target-${side}`} side={side} position={position} type="target" />
            ))}
          </>
          {handlePositions.map(([side, position]) => (
            <GraphHandle key={`source-${side}`} side={side} position={position} type="source" />
          ))}
        </>
      ) : (
        <>
          <GraphHandle side="left" position={Position.Left} type="target" />
          <GraphHandle side="right" position={Position.Right} type="source" />
        </>
      )}
      {isFile ? (
        <div className="flex h-full items-center gap-2">
          <img
            alt=""
            aria-hidden="true"
            className="size-4 shrink-0"
            src={fileIconForLanguage(data.language)}
          />
          <p className="min-w-0 flex-1 truncate text-xs leading-4 font-medium">{data.label}</p>
        </div>
      ) : (
        <p
          className={cn(
            styles.directoryLabel,
            !data.hasDirectFiles && styles.filelessDirectoryLabel
          )}>
          {data.label}
        </p>
      )}
    </div>
  );
}

function GraphHandle({
  position,
  side,
  type,
}: {
  position: Position;
  side: (typeof handlePositions)[number][0];
  type: "source" | "target";
}) {
  return (
    <Handle
      id={`${type}-${side}`}
      className={cn(styles.handle, "pointer-events-none border-0 bg-transparent opacity-0")}
      type={type}
      position={position}
    />
  );
}

function ContextGraphContainsEdgeComponent({
  markerEnd,
  sourcePosition,
  sourceX,
  sourceY,
  targetPosition,
  targetX,
  targetY,
}: EdgeProps<ContextGraphEdge>) {
  const sourcePoint = moveOutsideNode({ x: sourceX, y: sourceY }, sourcePosition);
  const targetPoint = moveOutsideNode({ x: targetX, y: targetY }, targetPosition);
  const bendX = (sourcePoint.x + targetPoint.x) / 2;
  const edgePath = [
    `M ${sourcePoint.x},${sourcePoint.y}`,
    `L ${bendX},${sourcePoint.y}`,
    `L ${bendX},${targetPoint.y}`,
    `L ${targetPoint.x},${targetPoint.y}`,
  ].join(" ");
  return <BaseEdge path={edgePath} interactionWidth={0} markerEnd={markerEnd} />;
}

function ContextGraphImpactEdgeComponent({
  data,
  sourcePosition,
  sourceX,
  sourceY,
  targetPosition,
  targetX,
  targetY,
}: EdgeProps<ContextGraphEdge>) {
  const sourcePoint = moveOutsideNode(
    { x: sourceX + (data?.sourceOffset?.x ?? 0), y: sourceY + (data?.sourceOffset?.y ?? 0) },
    sourcePosition
  );
  const targetPoint = moveOutsideNode(
    { x: targetX + (data?.targetOffset?.x ?? 0), y: targetY + (data?.targetOffset?.y ?? 0) },
    targetPosition
  );
  const distance = Math.max(
    36,
    Math.min(120, Math.hypot(targetPoint.x - sourcePoint.x, targetPoint.y - sourcePoint.y) * 0.34)
  );
  const sourceControl = controlPointForPosition(sourcePoint, sourcePosition, distance);
  const targetControl = controlPointForPosition(targetPoint, targetPosition, distance);
  const edgePath = [
    `M ${sourcePoint.x},${sourcePoint.y}`,
    `C ${sourceControl.x},${sourceControl.y}`,
    `${targetControl.x},${targetControl.y}`,
    `${targetPoint.x},${targetPoint.y}`,
  ].join(" ");
  return (
    <>
      <BaseEdge path={edgePath} interactionWidth={0} />
      <polygon
        className={styles.impactEdgeHead}
        points={edgeHeadPoints(targetPoint, targetControl)}
      />
    </>
  );
}

function edgeHeadPoints(tip: { x: number; y: number }, precedingPoint: { x: number; y: number }) {
  const deltaX = tip.x - precedingPoint.x,
    deltaY = tip.y - precedingPoint.y,
    length = Math.hypot(deltaX, deltaY) || 1;
  const directionX = deltaX / length,
    directionY = deltaY / length,
    baseX = tip.x - directionX * IMPACT_EDGE_HEAD_LENGTH,
    baseY = tip.y - directionY * IMPACT_EDGE_HEAD_LENGTH;
  const normalX = -directionY * IMPACT_EDGE_HEAD_HALF_WIDTH,
    normalY = directionX * IMPACT_EDGE_HEAD_HALF_WIDTH;
  return [
    `${tip.x},${tip.y}`,
    `${baseX + normalX},${baseY + normalY}`,
    `${baseX - normalX},${baseY - normalY}`,
  ].join(" ");
}
function moveOutsideNode(point: { x: number; y: number }, position: Position) {
  return controlPointForPosition(point, position, EDGE_ENDPOINT_GAP);
}
function controlPointForPosition(
  point: { x: number; y: number },
  position: Position,
  distance: number
) {
  switch (position) {
    case Position.Top:
      return { x: point.x, y: point.y - distance };
    case Position.Right:
      return { x: point.x + distance, y: point.y };
    case Position.Bottom:
      return { x: point.x, y: point.y + distance };
    case Position.Left:
      return { x: point.x - distance, y: point.y };
  }
}
