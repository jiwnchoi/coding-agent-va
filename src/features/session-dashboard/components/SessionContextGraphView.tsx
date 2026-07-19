import {
  getViewportForBounds,
  ReactFlow,
  type ReactFlowInstance,
  type Viewport,
} from "@xyflow/react";
import { useCallback, useMemo, useRef, useState } from "react";

import {
  useExternalGraphState,
  useSessionGraphHover,
} from "@/features/session-dashboard/components/context-graph-external-hover";
import {
  createNodePopover,
  expandNodePopover,
  resetGraphViewport,
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
import { contextGraphBounds } from "@/features/session-dashboard/context-graph/layoutGeometry";
import { buildNodeDescriptionRequest } from "@/features/session-dashboard/context-graph/nodeDescriptionContext";
import type {
  ContextGraphEdge,
  ContextGraphNode,
} from "@/features/session-dashboard/context-graph/types";
import { useSessionContextGraph } from "@/features/session-dashboard/context-graph/useSessionContextGraph";
import { useElementSize } from "@/features/session-dashboard/hooks/useElementSize";
import { useNodeDescription } from "@/features/session-dashboard/hooks/useNodeDescription";
import type {
  AgentSessionFileActivity,
  AgentSessionSummary,
  SelectedActivityFile,
} from "@/features/session-dashboard/lib/session-watch";
import type { DescriptionSettings } from "@/shared/lib/generated/bindings";
import { cn } from "@/shared/lib/utils";

const PRO_OPTIONS = { hideAttribution: true } as const;
const MIN_ZOOM = 0.2;
const MAX_ZOOM = 1.8;
const VIEWPORT_PADDING = 0.08;
const VISIBLE_ELEMENTS_THRESHOLD = 200;
const EMPTY_PINNED_FILE_PATHS: string[] = [];
const viewportByGraphKey = new Map<string, Viewport>();

type SessionContextGraphViewProps = {
  descriptionSettings: DescriptionSettings;
  fileActivity: AgentSessionFileActivity;
  graphScopeKey: string;
  isFileActivityLoading: boolean;
  selectedActivityFile: SelectedActivityFile | null;
  hoveredFilePaths: string[] | null;
  onGraphHoverFilePaths: (filePaths: string[] | null) => void;
  selectedSession: AgentSessionSummary | null;
  showReadFiles: boolean;
  onSelectFile: (selection: SelectedActivityFile) => void;
};

export function SessionContextGraphView({
  descriptionSettings,
  fileActivity,
  graphScopeKey,
  isFileActivityLoading,
  selectedActivityFile,
  hoveredFilePaths,
  onGraphHoverFilePaths,
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
  const shellSize = useElementSize(shellRef);
  const isPanningRef = useRef(false);
  const [nodePopover, setNodePopover] = useState<NodePopoverState | null>(null);
  const {
    describe: describeNode,
    description: nodeDescription,
    errorMessage: nodeDescriptionError,
    isLoading: isNodeDescriptionLoading,
    providerLabel: nodeDescriptionProvider,
    reset: resetNodeDescription,
  } = useNodeDescription();
  const nodes = contextGraph.nodes;
  const graphKey = selectedSession
    ? `${selectedSession.id}:${graphScopeKey}:${showReadFiles ? "with-reads" : "without-reads"}`
    : null;
  const layoutKey = useMemo(
    () =>
      graphKey
        ? `${graphKey}:${nodes
            .map(
              (node) =>
                `${node.id}:${node.position.x}:${node.position.y}:${String(node.style?.width)}:${String(node.style?.height)}`
            )
            .join("\0")}`
        : null,
    [graphKey, nodes]
  );
  const initialViewport = useMemo(() => {
    if (!graphKey || nodes.length === 0 || shellSize.width === 0 || shellSize.height === 0) {
      return null;
    }

    return (
      viewportByGraphKey.get(graphKey) ??
      getViewportForBounds(
        contextGraphBounds(nodes),
        shellSize.width,
        shellSize.height,
        MIN_ZOOM,
        MAX_ZOOM,
        VIEWPORT_PADDING
      )
    );
  }, [graphKey, nodes, shellSize.height, shellSize.width]);
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
  const { handleGraphNodeMouseEnter, handleGraphNodeMouseLeave, pinHover, releaseHover } =
    useSessionGraphHover(shellRef, hoverIndex, isPanningRef, onGraphHoverFilePaths, nodes);
  const { nodes: renderedNodes, edges: renderedEdges } = useExternalGraphState(
    nodes,
    edges,
    hoveredFilePaths,
    selectedSession?.cwd ?? "",
    hoverIndex
  );
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
  const isAwaitingViewport = nodes.length > 0 && initialViewport === null;
  const isGraphLoading = isFileActivityLoading || isLoading || isAwaitingViewport;

  return (
    <section className="h-full min-h-0 w-full">
      <div
        ref={shellRef}
        className={cn(
          styles.shell,
          isLargeGraph && styles.largeGraph,
          "relative h-full min-h-0 w-full overflow-hidden bg-transparent"
        )}>
        {selectedSession?.cwd && (!isAwaitingViewport || nodes.length === 0) ? (
          <ReactFlow
            key={layoutKey}
            className="h-full w-full"
            defaultViewport={initialViewport ?? undefined}
            nodes={renderedNodes}
            edges={renderedEdges}
            edgeTypes={edgeTypes}
            nodeTypes={nodeTypes}
            nodesDraggable={false}
            nodesConnectable={false}
            elementsSelectable={false}
            edgesFocusable={false}
            minZoom={MIN_ZOOM}
            maxZoom={MAX_ZOOM}
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
            onNodeMouseEnter={handleGraphNodeMouseEnter}
            onNodeMouseLeave={handleGraphNodeMouseLeave}
          />
        ) : !selectedSession?.cwd ? (
          <GraphEmptyState
            title="No workspace path"
            description="This session does not expose a workspace yet, so the file tree cannot be indexed."
          />
        ) : null}
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
