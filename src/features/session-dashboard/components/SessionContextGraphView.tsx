import {
  BaseEdge,
  type EdgeProps,
  Handle,
  Position,
  ReactFlow,
  type NodeProps,
  type ReactFlowInstance,
  type Viewport,
} from "@xyflow/react";
import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";

import { NodeActionPopover } from "@/features/session-dashboard/components/NodeActionPopover";
import {
  buildContextGraphHoverIndex,
  collectHoverRelatedEdgeIds,
  collectHoverRelatedNodeIds,
} from "@/features/session-dashboard/context-graph/contextGraphHover";
import styles from "@/features/session-dashboard/context-graph/ContextGraphView.module.css";
import { fileIconForLanguage } from "@/features/session-dashboard/context-graph/fileIcon";
import { buildNodeDescriptionRequest } from "@/features/session-dashboard/context-graph/nodeDescriptionContext";
import type {
  ContextGraphEdge,
  ContextGraphNode,
  ContextGraphNodeData,
} from "@/features/session-dashboard/context-graph/types";
import { useSessionContextGraph } from "@/features/session-dashboard/context-graph/useSessionContextGraph";
import { useNodeDescription } from "@/features/session-dashboard/hooks/useNodeDescription";
import {
  type ActivitySectionKey,
  type AgentSessionFileActivity,
  type AgentSessionSummary,
  type SelectedActivityFile,
} from "@/features/session-dashboard/lib/session-watch";
import type { DescriptionSettings } from "@/shared/lib/generated/bindings";
import { cn } from "@/shared/lib/utils";

const nodeTypes = {
  contextGraphNode: memo(ContextGraphNodeComponent),
};
const viewportByGraphKey = new Map<string, Viewport>();
const edgeTypes = {
  contextGraphContainsEdge: memo(ContextGraphContainsEdgeComponent),
  contextGraphImpactEdge: memo(ContextGraphImpactEdgeComponent),
};
const PRO_OPTIONS = { hideAttribution: true } as const;
const FIT_VIEW_OPTIONS = {
  padding: 0.08,
} as const;
const EDGE_ENDPOINT_GAP = 1;
const IMPACT_EDGE_HEAD_LENGTH = 8;
const IMPACT_EDGE_HEAD_HALF_WIDTH = 4;
const VISIBLE_ELEMENTS_THRESHOLD = 200;
const POPOVER_OFFSET = 12;
const POPOVER_ACTIONS_WIDTH = 192;
const POPOVER_ACTIONS_HEIGHT = 40;
const POPOVER_RESULT_WIDTH = 416;
const POPOVER_RESULT_MAX_HEIGHT = 556;
const EMPTY_PINNED_FILE_PATHS: string[] = [];
type PopoverAnchor = { bottom: number; left: number; right: number; top: number };
type NodePopoverState = {
  anchor: PopoverAnchor;
  node: ContextGraphNode;
  position: { x: number; y: number };
  sessionId: string;
};
const handlePositions = [
  ["top", Position.Top],
  ["right", Position.Right],
  ["bottom", Position.Bottom],
  ["left", Position.Left],
] as const;

type SessionContextGraphViewProps = {
  descriptionSettings: DescriptionSettings;
  fileActivity: AgentSessionFileActivity;
  isFileActivityLoading: boolean;
  selectedActivityFile: SelectedActivityFile | null;
  selectedSession: AgentSessionSummary | null;
  onSelectFile: (selection: SelectedActivityFile) => void;
};

export function SessionContextGraphView({
  descriptionSettings,
  fileActivity,
  isFileActivityLoading,
  selectedActivityFile,
  selectedSession,
  onSelectFile,
}: SessionContextGraphViewProps) {
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
    const nodeById = new Map(contextGraph.nodes.map((node) => [node.id, node]));
    const folderEdges = contextGraph.containsEdges.filter(
      (edge) =>
        nodeById.get(edge.source)?.data.kind === "directory" &&
        nodeById.get(edge.target)?.data.kind === "directory"
    );

    return [...folderEdges, ...contextGraph.impactEdges];
  }, [contextGraph]);
  const hoverIndex = useMemo(
    () => buildContextGraphHoverIndex(contextGraph.containsEdges, contextGraph.impactEdges),
    [contextGraph.containsEdges, contextGraph.impactEdges]
  );
  const isLargeGraph = nodes.length + edges.length >= VISIBLE_ELEMENTS_THRESHOLD;

  const { handleNodeMouseEnter, handleNodeMouseLeave, pinHover, releaseHover } =
    useContextGraphHover(shellRef, hoverIndex, isPanningRef);

  const handleMoveStart = useCallback(() => {
    isPanningRef.current = true;
    const shell = shellRef.current;
    shell?.classList.add(styles.panning);
    releaseHover();
  }, [releaseHover]);
  const handleMoveEnd = useCallback(
    (_event: MouseEvent | TouchEvent | null, viewport: Viewport) => {
      isPanningRef.current = false;
      shellRef.current?.classList.remove(styles.panning);
      if (graphKey) {
        viewportByGraphKey.set(graphKey, viewport);
      }
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
    ) {
      return;
    }

    const currentGraphKey = graphKey;
    const currentReactFlowInstance = reactFlowInstance;
    const savedViewport = viewportByGraphKey.get(currentGraphKey);
    let animationFrameId: number | null = null;

    function fitGraph() {
      if (animationFrameId !== null) {
        cancelAnimationFrame(animationFrameId);
      }

      animationFrameId = requestAnimationFrame(() => {
        if (savedViewport) {
          void currentReactFlowInstance.setViewport(savedViewport, { duration: 0 });
        } else {
          void currentReactFlowInstance.fitView({ ...FIT_VIEW_OPTIONS, duration: 0 });
        }
        lastFittedGraph.current = { key: currentGraphKey, instance: currentReactFlowInstance };
        animationFrameId = null;
      });
    }

    fitGraph();

    return () => {
      if (animationFrameId !== null) {
        cancelAnimationFrame(animationFrameId);
      }
    };
  }, [graphKey, nodes.length, reactFlowInstance]);

  const handleNodeClick = useCallback(
    (event: React.MouseEvent, node: ContextGraphNode) => {
      if (node.data.kind !== "file" || !selectedSession || !shellRef.current) {
        return;
      }

      pinHover(node);
      resetNodeDescription();
      setNodePopover(createNodePopover(event, node, selectedSession.id, shellRef.current));
    },
    [pinHover, resetNodeDescription, selectedSession]
  );

  const handlePaneDoubleClick = useCallback(
    (event: React.MouseEvent) => resetGraphViewport(event, reactFlowInstance, graphKey),
    [graphKey, reactFlowInstance]
  );
  const handleOpenCode = useCallback(
    (node: ContextGraphNode) => {
      const activityKey = resolvePrimaryActivity(node.data.activities);
      onSelectFile({ activityKey, filePath: node.data.displayPath });
      releaseHover();
      setNodePopover(null);
      resetNodeDescription();
    },
    [onSelectFile, releaseHover, resetNodeDescription]
  );

  const handleDescribeNode = useCallback(
    (node: ContextGraphNode) => {
      if (!selectedSession) {
        return;
      }
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
            onDoubleClick={handlePaneDoubleClick}
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

function useContextGraphHover(
  shellRef: React.RefObject<HTMLDivElement | null>,
  hoverIndex: ReturnType<typeof buildContextGraphHoverIndex>,
  isPanningRef: React.MutableRefObject<boolean>
) {
  const hoverElementsRef = useRef<Set<Element>>(new Set());
  const pinnedHoverNodeIdRef = useRef<string | null>(null);

  const clearHover = useCallback(() => {
    clearGraphHover(shellRef.current, hoverElementsRef.current);
  }, [shellRef]);
  const releaseHover = useCallback(() => {
    pinnedHoverNodeIdRef.current = null;
    clearHover();
  }, [clearHover]);
  const handleNodeMouseEnter = useCallback(
    (_event: React.MouseEvent, node: ContextGraphNode) => {
      if (
        (pinnedHoverNodeIdRef.current && pinnedHoverNodeIdRef.current !== node.id) ||
        isPanningRef.current ||
        !isHoverableNode(node.data.activities)
      ) {
        return;
      }
      const relatedNodeIds = collectHoverRelatedNodeIds(node.id, hoverIndex);
      applyGraphHover(
        shellRef.current,
        hoverElementsRef.current,
        relatedNodeIds,
        collectHoverRelatedEdgeIds(relatedNodeIds, hoverIndex)
      );
    },
    [hoverIndex, isPanningRef, shellRef]
  );
  const handleNodeMouseLeave = useCallback(() => {
    if (!pinnedHoverNodeIdRef.current) {
      clearHover();
    }
  }, [clearHover]);
  const pinHover = useCallback(
    (node: ContextGraphNode) => {
      if (!isHoverableNode(node.data.activities)) {
        return;
      }
      pinnedHoverNodeIdRef.current = node.id;
      const relatedNodeIds = collectHoverRelatedNodeIds(node.id, hoverIndex);
      applyGraphHover(
        shellRef.current,
        hoverElementsRef.current,
        relatedNodeIds,
        collectHoverRelatedEdgeIds(relatedNodeIds, hoverIndex)
      );
    },
    [hoverIndex, shellRef]
  );

  useEffect(
    () => () => {
      pinnedHoverNodeIdRef.current = null;
      clearHover();
    },
    [clearHover, hoverIndex]
  );

  return { handleNodeMouseEnter, handleNodeMouseLeave, pinHover, releaseHover };
}

function resetGraphViewport(
  event: React.MouseEvent,
  reactFlowInstance: ReactFlowInstance<ContextGraphNode, ContextGraphEdge> | null,
  graphKey: string | null
) {
  if (
    (event.target instanceof Element &&
      event.target.closest(".react-flow__node, .react-flow__edge")) ||
    !reactFlowInstance
  ) {
    return;
  }
  if (graphKey) {
    viewportByGraphKey.delete(graphKey);
  }
  void reactFlowInstance.fitView({ ...FIT_VIEW_OPTIONS, duration: 0 });
}

function applyGraphHover(
  shell: HTMLDivElement | null,
  hoverElements: Set<Element>,
  relatedNodeIds: Set<string>,
  relatedEdgeIds: Set<string>
) {
  if (!shell) {
    return;
  }

  shell.classList.add(styles.hovering);

  const nextHoverElements = new Set<Element>();
  for (const nodeId of relatedNodeIds) {
    addHoverElement(shell, nextHoverElements, ".react-flow__node", nodeId);
  }
  for (const edgeId of relatedEdgeIds) {
    addHoverElement(shell, nextHoverElements, ".react-flow__edge", edgeId);
  }

  for (const element of hoverElements) {
    if (!nextHoverElements.has(element)) {
      element.classList.remove(styles.relatedElement);
    }
  }
  for (const element of nextHoverElements) {
    if (!hoverElements.has(element)) {
      element.classList.add(styles.relatedElement);
    }
  }

  hoverElements.clear();
  for (const element of nextHoverElements) {
    hoverElements.add(element);
  }
}

function addHoverElement(
  shell: HTMLDivElement,
  hoverElements: Set<Element>,
  selector: string,
  id: string
) {
  const element = shell.querySelector(`${selector}[data-id="${CSS.escape(id)}"]`);
  if (element) {
    element.classList.add(styles.relatedElement);
    hoverElements.add(element);
  }
}

function clearGraphHover(shell: HTMLDivElement | null, hoverElements: Set<Element>) {
  shell?.classList.remove(styles.hovering);
  for (const element of hoverElements) {
    element.classList.remove(styles.relatedElement);
  }
  hoverElements.clear();
}

function expandNodePopover(popover: NodePopoverState | null, shell: HTMLDivElement | null) {
  if (!popover || !shell) return popover;
  return {
    ...popover,
    position: popoverPosition(
      popover.anchor,
      shell,
      Math.min(POPOVER_RESULT_WIDTH, shell.clientWidth - POPOVER_OFFSET * 2),
      Math.min(POPOVER_RESULT_MAX_HEIGHT, shell.clientHeight - POPOVER_OFFSET * 2)
    ),
  };
}

function createNodePopover(
  event: React.MouseEvent,
  node: ContextGraphNode,
  sessionId: string,
  shell: HTMLDivElement
): NodePopoverState {
  const anchor = nodePopoverAnchor(event, shell);
  return {
    anchor,
    node,
    position: popoverPosition(anchor, shell, POPOVER_ACTIONS_WIDTH, POPOVER_ACTIONS_HEIGHT),
    sessionId,
  };
}

function nodePopoverAnchor(event: React.MouseEvent, shell: HTMLDivElement): PopoverAnchor {
  const shellBounds = shell.getBoundingClientRect();
  const nodeElement = [event.currentTarget, event.target]
    .filter((target): target is Element => target instanceof Element)
    .map((target) =>
      target.matches(".react-flow__node") ? target : target.closest(".react-flow__node")
    )
    .find((target): target is Element => target !== null);
  const nodeBounds = nodeElement?.getBoundingClientRect();
  if (!nodeBounds) {
    const x = event.clientX - shellBounds.left;
    const y = event.clientY - shellBounds.top;
    return { bottom: y, left: x, right: x, top: y };
  }
  return {
    bottom: nodeBounds.bottom - shellBounds.top,
    left: nodeBounds.left - shellBounds.left,
    right: nodeBounds.right - shellBounds.left,
    top: nodeBounds.top - shellBounds.top,
  };
}

function popoverPosition(
  anchor: PopoverAnchor,
  shell: HTMLDivElement,
  width: number,
  height: number
) {
  const centeredX = (anchor.left + anchor.right - width) / 2;
  const centeredY = (anchor.top + anchor.bottom - height) / 2;
  const candidates = [
    {
      available: shell.clientWidth - anchor.right - POPOVER_OFFSET,
      position: { x: anchor.right + POPOVER_OFFSET, y: clampY(centeredY, shell, height) },
      required: width,
    },
    {
      available: anchor.left - POPOVER_OFFSET,
      position: { x: anchor.left - width - POPOVER_OFFSET, y: clampY(centeredY, shell, height) },
      required: width,
    },
    {
      available: shell.clientHeight - anchor.bottom - POPOVER_OFFSET,
      position: { x: clampX(centeredX, shell, width), y: anchor.bottom + POPOVER_OFFSET },
      required: height,
    },
    {
      available: anchor.top - POPOVER_OFFSET,
      position: { x: clampX(centeredX, shell, width), y: anchor.top - height - POPOVER_OFFSET },
      required: height,
    },
  ];
  return (
    candidates.find((candidate) => candidate.available >= candidate.required) ??
    [...candidates].sort((left, right) => right.available - left.available)[0]
  ).position;
}

function clampX(value: number, shell: HTMLDivElement, width: number) {
  return Math.max(POPOVER_OFFSET, Math.min(value, shell.clientWidth - width - POPOVER_OFFSET));
}

function clampY(value: number, shell: HTMLDivElement, height: number) {
  return Math.max(POPOVER_OFFSET, Math.min(value, shell.clientHeight - height - POPOVER_OFFSET));
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
          {handlePositions.map(([side, position]) => (
            <GraphHandle key={`target-${side}`} side={side} position={position} type="target" />
          ))}
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
    {
      x: sourceX + (data?.sourceOffset?.x ?? 0),
      y: sourceY + (data?.sourceOffset?.y ?? 0),
    },
    sourcePosition
  );
  const targetPoint = moveOutsideNode(
    {
      x: targetX + (data?.targetOffset?.x ?? 0),
      y: targetY + (data?.targetOffset?.y ?? 0),
    },
    targetPosition
  );
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
  const deltaX = tip.x - precedingPoint.x;
  const deltaY = tip.y - precedingPoint.y;
  const length = Math.hypot(deltaX, deltaY) || 1;
  const directionX = deltaX / length;
  const directionY = deltaY / length;
  const baseX = tip.x - directionX * IMPACT_EDGE_HEAD_LENGTH;
  const baseY = tip.y - directionY * IMPACT_EDGE_HEAD_LENGTH;
  const normalX = -directionY * IMPACT_EDGE_HEAD_HALF_WIDTH;
  const normalY = directionX * IMPACT_EDGE_HEAD_HALF_WIDTH;

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

function isHoverableNode(activities: ContextGraphNodeData["activities"]) {
  return activities.includes("edited") || activities.includes("impacted");
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
