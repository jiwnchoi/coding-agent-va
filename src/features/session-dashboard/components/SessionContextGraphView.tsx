import {
  BaseEdge,
  type EdgeProps,
  Handle,
  Position,
  ReactFlow,
  type NodeProps,
  type ReactFlowInstance,
} from "@xyflow/react";
import { FileCode2, FolderTree, SearchCode } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import styles from "@/features/session-dashboard/context-graph/ContextGraphView.module.css";
import type {
  ContextGraphEdge,
  ContextGraphNode,
  ContextGraphNodeData,
} from "@/features/session-dashboard/context-graph/types";
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
  contextGraphImpactEdge: ContextGraphImpactEdgeComponent,
};
const FIT_VIEW_OPTIONS = {
  padding: 0.08,
} as const;
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
  const nodes = contextGraph.nodes;
  const edges = useMemo(
    () => [...contextGraph.containsEdges, ...contextGraph.impactEdges],
    [contextGraph]
  );

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
        void currentReactFlowInstance.fitView(FIT_VIEW_OPTIONS);
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
  }, [nodes, reactFlowInstance]);

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
          "relative h-full min-h-0 w-full overflow-hidden bg-transparent"
        )}>
        {selectedSession?.cwd ? (
          <ReactFlow
            className="h-full w-full"
            nodes={nodes}
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
  const Icon =
    data.kind === "repo" ? SearchCode : data.kind === "directory" ? FolderTree : FileCode2;

  return (
    <div
      className={cn(
        styles.node,
        "text-card-foreground border-border h-11 w-[156px] rounded-md border px-2 py-1.5",
        data.kind === "repo" && styles.repoNode,
        data.kind === "directory" && styles.directoryNode,
        data.kind === "file" && "bg-card",
        primaryActivityClass(data.activities),
        data.isSelected && styles.selectedNode
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
      <div className="flex items-start gap-2">
        <Icon className="mt-0.5 size-3.5 shrink-0" />
        <div className="min-w-0 flex-1">
          <p className="truncate text-xs leading-4 font-medium">{data.label}</p>
          <p className="text-muted-foreground truncate text-[10px] leading-3">{data.displayPath}</p>
        </div>
      </div>
    </div>
  );
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

  return <BaseEdge path={edgePath} markerEnd={markerEnd} />;
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
      return styles.impactedNode;
    case "read":
      return "";
  }
}
