import { ReactFlow, type ReactFlowInstance, type Viewport } from "@xyflow/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  createNodePopover,
  expandNodePopover,
  resetGraphViewport,
  useContextGraphHover,
  type NodePopoverState,
} from "@/features/session-dashboard/components/context-graph-interaction";
import {
  edgeTypes,
  GraphEmptyState,
  nodeTypes,
  resolvePrimaryActivity,
} from "@/features/session-dashboard/components/context-graph-renderers";
import { NodeActionPopover } from "@/features/session-dashboard/components/NodeActionPopover";
import { buildContextGraphHoverIndex } from "@/features/session-dashboard/context-graph/contextGraphHover";
import styles from "@/features/session-dashboard/context-graph/ContextGraphView.module.css";
import { buildNodeDescriptionRequest } from "@/features/session-dashboard/context-graph/nodeDescriptionContext";
import type {
  ContextGraphEdge,
  ContextGraphNode,
} from "@/features/session-dashboard/context-graph/types";
import { useSessionContextGraph } from "@/features/session-dashboard/context-graph/useSessionContextGraph";
import { useNodeDescription } from "@/features/session-dashboard/hooks/useNodeDescription";
import type {
  AgentSessionFileActivity,
  AgentSessionSummary,
  SelectedActivityFile,
} from "@/features/session-dashboard/lib/session-watch";
import type { DescriptionSettings } from "@/shared/lib/generated/bindings";
import { cn } from "@/shared/lib/utils";

const PRO_OPTIONS = { hideAttribution: true } as const;
const FIT_VIEW_OPTIONS = { padding: 0.08 } as const;
const VISIBLE_ELEMENTS_THRESHOLD = 200;
const EMPTY_PINNED_FILE_PATHS: string[] = [];
const viewportByGraphKey = new Map<string, Viewport>();

type SessionContextGraphViewProps = {
  descriptionSettings: DescriptionSettings;
  fileActivity: AgentSessionFileActivity;
  isFileActivityLoading: boolean;
  selectedActivityFile: SelectedActivityFile | null;
  selectedSession: AgentSessionSummary | null;
  showReadFiles: boolean;
  onSelectFile: (selection: SelectedActivityFile) => void;
};

export function SessionContextGraphView({
  descriptionSettings,
  fileActivity,
  isFileActivityLoading,
  selectedActivityFile,
  selectedSession,
  showReadFiles,
  onSelectFile,
}: SessionContextGraphViewProps) {
  const selectedFilePath = selectedActivityFile?.filePath ?? "";
  const { contextGraph, errorMessage, isLoading } = useSessionContextGraph({
    fileActivity,
    includeEntireWorkspace: false,
    pinnedFilePaths: EMPTY_PINNED_FILE_PATHS,
    selectedFilePath,
    selectedSession,
    showReadFiles,
  });
  const [reactFlowInstance, setReactFlowInstance] = useState<ReactFlowInstance<
    ContextGraphNode,
    ContextGraphEdge
  > | null>(null);
  const shellRef = useRef<HTMLDivElement>(null);
  const isPanningRef = useRef(false);
  const [nodePopover, setNodePopover] = useState<NodePopoverState | null>(null);
  const lastFittedGraph = useRef<{
    key: string;
    instance: ReactFlowInstance<ContextGraphNode, ContextGraphEdge>;
  } | null>(null);
  const {
    describe: describeNode,
    description: nodeDescription,
    errorMessage: nodeDescriptionError,
    isLoading: isNodeDescriptionLoading,
    providerLabel: nodeDescriptionProvider,
    reset: resetNodeDescription,
  } = useNodeDescription();
  const nodes = contextGraph.nodes;
  const graphKey = selectedSession ? `${selectedSession.id}:${selectedFilePath}` : null;
  const edges = useMemo(() => {
    const nodeById = new Map(nodes.map((node) => [node.id, node]));
    return [
      ...contextGraph.containsEdges.filter(
        (edge) =>
          nodeById.get(edge.source)?.data.kind === "directory" &&
          nodeById.get(edge.target)?.data.kind === "directory"
      ),
      ...contextGraph.impactEdges,
    ];
  }, [contextGraph, nodes]);
  const hoverIndex = useMemo(
    () => buildContextGraphHoverIndex(contextGraph.containsEdges, contextGraph.impactEdges),
    [contextGraph.containsEdges, contextGraph.impactEdges]
  );
  const isLargeGraph = nodes.length + edges.length >= VISIBLE_ELEMENTS_THRESHOLD;
  const { handleNodeMouseEnter, handleNodeMouseLeave, pinHover, releaseHover } =
    useContextGraphHover(shellRef, hoverIndex, isPanningRef);

  const handleMoveStart = useCallback(() => {
    isPanningRef.current = true;
    shellRef.current?.classList.add(styles.panning);
    releaseHover();
  }, [releaseHover]);
  const handleMoveEnd = useCallback(
    (_event: MouseEvent | TouchEvent | null, viewport: Viewport) => {
      isPanningRef.current = false;
      shellRef.current?.classList.remove(styles.panning);
      if (graphKey) viewportByGraphKey.set(graphKey, viewport);
    },
    [graphKey]
  );
  useEffect(() => {
    if (
      !reactFlowInstance ||
      nodes.length === 0 ||
      !graphKey ||
      (lastFittedGraph.current?.key === graphKey &&
        lastFittedGraph.current.instance === reactFlowInstance)
    )
      return;
    const savedViewport = viewportByGraphKey.get(graphKey);
    const frame = requestAnimationFrame(() => {
      if (savedViewport) void reactFlowInstance.setViewport(savedViewport, { duration: 0 });
      else void reactFlowInstance.fitView({ ...FIT_VIEW_OPTIONS, duration: 0 });
      lastFittedGraph.current = { key: graphKey, instance: reactFlowInstance };
    });
    return () => cancelAnimationFrame(frame);
  }, [graphKey, nodes.length, reactFlowInstance]);

  const handleNodeClick = useCallback(
    (event: React.MouseEvent, node: ContextGraphNode) => {
      if (node.data.kind !== "file" || !selectedSession || !shellRef.current) return;
      pinHover(node);
      resetNodeDescription();
      setNodePopover(createNodePopover(event, node, selectedSession.id, shellRef.current));
    },
    [pinHover, resetNodeDescription, selectedSession]
  );
  const handleOpenCode = useCallback(
    (node: ContextGraphNode) => {
      onSelectFile({
        activityKey: resolvePrimaryActivity(node.data.activities),
        filePath: node.data.displayPath,
      });
      releaseHover();
      setNodePopover(null);
      resetNodeDescription();
    },
    [onSelectFile, releaseHover, resetNodeDescription]
  );
  const handleDescribeNode = useCallback(
    (node: ContextGraphNode) => {
      if (!selectedSession) return;
      const request = buildNodeDescriptionRequest({
        contextGraph,
        descriptionSettings,
        node,
        session: selectedSession,
      });
      if (request) {
        setNodePopover((popover) => expandNodePopover(popover, shellRef.current));
        void describeNode(request);
      }
    },
    [contextGraph, describeNode, descriptionSettings, selectedSession]
  );
  const handleCloseNodePopover = useCallback(() => {
    releaseHover();
    setNodePopover(null);
    resetNodeDescription();
  }, [releaseHover, resetNodeDescription]);
  const isGraphLoading = isFileActivityLoading || isLoading;

  return (
    <section className="h-full min-h-0 w-full">
      <div
        ref={shellRef}
        className={cn(
          styles.shell,
          isLargeGraph && styles.largeGraph,
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
            edgesFocusable={false}
            minZoom={0.2}
            maxZoom={1.8}
            zoomOnDoubleClick={false}
            onlyRenderVisibleElements={isLargeGraph}
            proOptions={PRO_OPTIONS}
            onInit={setReactFlowInstance}
            onMoveStart={handleMoveStart}
            onMoveEnd={handleMoveEnd}
            onNodeClick={handleNodeClick}
            onDoubleClick={(event) =>
              resetGraphViewport(event, reactFlowInstance, graphKey, viewportByGraphKey)
            }
            onNodeMouseEnter={handleNodeMouseEnter}
            onNodeMouseLeave={handleNodeMouseLeave}
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
        {nodePopover && nodePopover.sessionId === selectedSession?.id ? (
          <NodeActionPopover
            key={nodePopover.node.id}
            description={nodeDescription}
            errorMessage={nodeDescriptionError}
            isLoading={isNodeDescriptionLoading}
            label={nodePopover.node.data.label}
            position={nodePopover.position}
            providerLabel={nodeDescriptionProvider}
            onClose={handleCloseNodePopover}
            onDescribe={() => handleDescribeNode(nodePopover.node)}
            onOpenCode={() => handleOpenCode(nodePopover.node)}
          />
        ) : null}
      </div>
    </section>
  );
}
