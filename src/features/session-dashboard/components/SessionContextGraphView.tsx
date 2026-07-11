import {
  BaseEdge,
  type EdgeProps,
  Handle,
  Position,
  ReactFlow,
  type NodeProps,
  type ReactFlowInstance,
} from "@xyflow/react";
import { useEffect, useMemo, useState } from "react";

import styles from "@/features/session-dashboard/context-graph/ContextGraphView.module.css";
import { fileIconForLanguage } from "@/features/session-dashboard/context-graph/fileIcon";
import type {
  ContextGraphEdge,
  ContextGraphNode,
  ContextGraphNodeData,
} from "@/features/session-dashboard/context-graph/types";
import { useAnimatedContextGraphNodes } from "@/features/session-dashboard/context-graph/useAnimatedContextGraphNodes";
import { useSessionContextGraph } from "@/features/session-dashboard/context-graph/useSessionContextGraph";
import {
  type ActivitySectionKey,
  type AgentSessionFileActivity,
  type AgentSessionSummary,
  type SelectedActivityFile,
} from "@/features/session-dashboard/lib/session-watch";
import { cn } from "@/shared/lib/utils";

const nodeTypes = {
  contextGraphNode: ContextGraphNodeComponent,
};
const edgeTypes = {
  contextGraphContainsEdge: ContextGraphContainsEdgeComponent,
  contextGraphImpactEdge: ContextGraphImpactEdgeComponent,
};
const FIT_VIEW_OPTIONS = {
  padding: 0.08,
} as const;
const FIT_VIEW_ANIMATION_DURATION = 420;
const EMPTY_PINNED_FILE_PATHS: string[] = [];
const handlePositions = [
  ["top", Position.Top],
  ["right", Position.Right],
  ["bottom", Position.Bottom],
  ["left", Position.Left],
] as const;

export function SessionContextGraphView({
  fileActivity,
  isFileActivityLoading,
  selectedActivityFile,
  selectedSession,
  onSelectFile,
}: {
  fileActivity: AgentSessionFileActivity;
  isFileActivityLoading: boolean;
  selectedActivityFile: SelectedActivityFile | null;
  selectedSession: AgentSessionSummary | null;
  onSelectFile: (selection: SelectedActivityFile) => void;
}) {
  const selectedFilePath = selectedActivityFile?.filePath ?? "";
  const { contextGraph, errorMessage, isLoading } = useSessionContextGraph({
    fileActivity,
    includeEntireWorkspace: false,
    pinnedFilePaths: EMPTY_PINNED_FILE_PATHS,
    selectedFilePath,
    selectedSession,
  });
  const [reactFlowInstance, setReactFlowInstance] = useState<ReactFlowInstance<
    ContextGraphNode,
    ContextGraphEdge
  > | null>(null);
  const sessionId = selectedSession?.id ?? "";
  const nodes = contextGraph.nodes;
  const { isGraphSwitch, nodes: animatedNodes } = useAnimatedContextGraphNodes(nodes, sessionId);
  const edges = useMemo(() => {
    const nodeById = new Map(contextGraph.nodes.map((node) => [node.id, node]));
    const folderEdges = contextGraph.containsEdges.filter(
      (edge) =>
        nodeById.get(edge.source)?.data.kind === "directory" &&
        nodeById.get(edge.target)?.data.kind === "directory"
    );

    return [...folderEdges, ...contextGraph.impactEdges];
  }, [contextGraph]);

  useEffect(() => {
    if (!reactFlowInstance || nodes.length === 0) {
      return;
    }

    const currentReactFlowInstance = reactFlowInstance;
    let animationFrameId: number | null = null;

    function fitGraph() {
      if (animationFrameId !== null) {
        cancelAnimationFrame(animationFrameId);
      }

      animationFrameId = requestAnimationFrame(() => {
        void currentReactFlowInstance.fitView({
          ...FIT_VIEW_OPTIONS,
          duration:
            isGraphSwitch || window.matchMedia("(prefers-reduced-motion: reduce)").matches
              ? 0
              : FIT_VIEW_ANIMATION_DURATION,
        });
        animationFrameId = null;
      });
    }

    function handleVisibilityChange() {
      if (!document.hidden) {
        fitGraph();
      }
    }

    fitGraph();
    document.addEventListener("visibilitychange", handleVisibilityChange);

    return () => {
      if (animationFrameId !== null) {
        cancelAnimationFrame(animationFrameId);
      }
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [isGraphSwitch, nodes, reactFlowInstance]);

  function handleNodeClick(node: ContextGraphNode) {
    if (node.data.kind !== "file") {
      return;
    }

    const activityKey = resolvePrimaryActivity(node.data.activities);
    onSelectFile({ activityKey, filePath: node.data.displayPath });
  }

  const isGraphLoading = isFileActivityLoading || isLoading;

  return (
    <section className="h-full min-h-0 w-full">
      <div
        className={cn(
          styles.shell,
          isGraphSwitch && styles.motionDisabled,
          "relative h-full min-h-0 w-full overflow-hidden bg-transparent"
        )}>
        {selectedSession?.cwd ? (
          <ReactFlow
            className="h-full w-full"
            nodes={animatedNodes}
            edges={edges}
            edgeTypes={edgeTypes}
            nodeTypes={nodeTypes}
            nodesDraggable={false}
            nodesConnectable={false}
            elementsSelectable={false}
            fitView
            fitViewOptions={FIT_VIEW_OPTIONS}
            minZoom={0.2}
            maxZoom={1.8}
            zoomOnDoubleClick={false}
            proOptions={{ hideAttribution: true }}
            onInit={setReactFlowInstance}
            onNodeClick={(_event, node) => handleNodeClick(node as ContextGraphNode)}
          />
        ) : (
          <GraphEmptyState
            title="No workspace path"
            description="This session does not expose a workspace yet, so the file tree cannot be indexed."
          />
        )}

        {isGraphLoading ? (
          <div
            className={cn(
              styles.status,
              "border-border text-muted-foreground absolute top-17 left-4 z-[5] rounded-full border px-3 py-[0.45rem] text-xs"
            )}>
            Indexing and laying out...
          </div>
        ) : null}
        {errorMessage ? (
          <div
            className={cn(
              styles.error,
              "text-destructive absolute top-17 left-4 z-[5] rounded-full border px-3 py-[0.45rem] text-xs"
            )}>
            {errorMessage}
          </div>
        ) : null}
        {selectedSession?.cwd && nodes.length === 0 && !isGraphLoading ? (
          <GraphEmptyState
            title="No graph nodes"
            description="No session files matched the indexed workspace graph yet."
          />
        ) : null}
      </div>
    </section>
  );
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
        isFile && primaryActivityClass(data.activities),
        isFile && data.isSelected && styles.selectedNode
      )}>
      {handlePositions.map(([side, position]) => (
        <Handle
          key={`target-${side}`}
          id={`target-${side}`}
          className={cn(styles.handle, "pointer-events-none border-0 bg-transparent opacity-0")}
          type="target"
          position={position}
        />
      ))}
      {handlePositions.map(([side, position]) => (
        <Handle
          key={`source-${side}`}
          id={`source-${side}`}
          className={cn(styles.handle, "pointer-events-none border-0 bg-transparent opacity-0")}
          type="source"
          position={position}
        />
      ))}
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

function ContextGraphContainsEdgeComponent({
  markerEnd,
  sourceX,
  sourceY,
  targetX,
  targetY,
}: EdgeProps<ContextGraphEdge>) {
  const bendX = (sourceX + targetX) / 2;
  const edgePath = [
    `M ${sourceX},${sourceY}`,
    `L ${bendX},${sourceY}`,
    `L ${bendX},${targetY}`,
    `L ${targetX},${targetY}`,
  ].join(" ");

  return <BaseEdge className={styles.enteringContainsEdge} path={edgePath} markerEnd={markerEnd} />;
}

function ContextGraphImpactEdgeComponent({
  data,
  markerEnd,
  sourcePosition,
  sourceX,
  sourceY,
  targetPosition,
  targetX,
  targetY,
}: EdgeProps<ContextGraphEdge>) {
  const sourcePoint = {
    x: sourceX + (data?.sourceOffset?.x ?? 0),
    y: sourceY + (data?.sourceOffset?.y ?? 0),
  };
  const targetPoint = {
    x: targetX + (data?.targetOffset?.x ?? 0),
    y: targetY + (data?.targetOffset?.y ?? 0),
  };
  const controlDistance = Math.max(
    36,
    Math.min(120, Math.hypot(targetPoint.x - sourcePoint.x, targetPoint.y - sourcePoint.y) * 0.34)
  );
  const sourceControl = controlPointForPosition(sourcePoint, sourcePosition, controlDistance);
  const targetControl = controlPointForPosition(targetPoint, targetPosition, controlDistance);
  const edgePath = [
    `M ${sourcePoint.x},${sourcePoint.y}`,
    `C ${sourceControl.x},${sourceControl.y}`,
    `${targetControl.x},${targetControl.y}`,
    `${targetPoint.x},${targetPoint.y}`,
  ].join(" ");

  return (
    <BaseEdge
      className={styles.enteringImpactEdge}
      path={edgePath}
      markerEnd={markerEnd}
      pathLength={1}
    />
  );
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

function GraphEmptyState({ title, description }: { title: string; description: string }) {
  return (
    <div className="absolute inset-0 z-[2] flex flex-col items-center justify-center p-8 text-center">
      <p className="text-sm font-medium">{title}</p>
      <p className="text-muted-foreground mt-1 max-w-md text-sm">{description}</p>
    </div>
  );
}

function resolvePrimaryActivity(
  activities: ContextGraphNodeData["activities"]
): ActivitySectionKey {
  return activities[0] ?? "impacted";
}

function primaryActivityClass(activities: ContextGraphNodeData["activities"]) {
  if (activities.length === 0) {
    return "";
  }

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
