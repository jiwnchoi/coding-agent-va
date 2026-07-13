import type { ReactFlowInstance, Viewport } from "@xyflow/react";
import { useCallback, useEffect, useRef } from "react";

import type { ContextGraphHoverIndex } from "@/features/session-dashboard/context-graph/contextGraphHover";
import {
  collectHoverRelatedEdgeIds,
  collectHoverRelatedNodeIds,
} from "@/features/session-dashboard/context-graph/contextGraphHover";
import styles from "@/features/session-dashboard/context-graph/ContextGraphView.module.css";
import type {
  ContextGraphEdge,
  ContextGraphNode,
  ContextGraphNodeData,
} from "@/features/session-dashboard/context-graph/types";

const FIT_VIEW_OPTIONS = { padding: 0.08 } as const;
const POPOVER_OFFSET = 12;
const POPOVER_ACTIONS_WIDTH = 192;
const POPOVER_ACTIONS_HEIGHT = 40;
const POPOVER_RESULT_WIDTH = 416;
const POPOVER_RESULT_MAX_HEIGHT = 556;

export type PopoverAnchor = { bottom: number; left: number; right: number; top: number };
export type NodePopoverState = {
  anchor: PopoverAnchor;
  node: ContextGraphNode;
  position: { x: number; y: number };
  sessionId: string;
};

export function useContextGraphHover(
  shellRef: React.RefObject<HTMLDivElement | null>,
  hoverIndex: ContextGraphHoverIndex,
  isPanningRef: React.MutableRefObject<boolean>
) {
  const hoverElementsRef = useRef<Set<Element>>(new Set());
  const pinnedHoverNodeIdRef = useRef<string | null>(null);
  const pendingClearTimeoutRef = useRef<number | null>(null);
  const cancelPendingClear = useCallback(() => {
    if (pendingClearTimeoutRef.current !== null) {
      window.clearTimeout(pendingClearTimeoutRef.current);
      pendingClearTimeoutRef.current = null;
    }
  }, []);
  const clearHover = useCallback(() => {
    clearGraphHover(shellRef.current, hoverElementsRef.current);
  }, [shellRef]);
  const releaseHover = useCallback(() => {
    cancelPendingClear();
    pinnedHoverNodeIdRef.current = null;
    clearHover();
  }, [cancelPendingClear, clearHover]);
  const applyHover = useCallback(
    (node: ContextGraphNode) => {
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
  const handleNodeMouseEnter = useCallback(
    (_event: React.MouseEvent, node: ContextGraphNode) => {
      if (
        (pinnedHoverNodeIdRef.current && pinnedHoverNodeIdRef.current !== node.id) ||
        isPanningRef.current ||
        !isHoverableNode(node.data.activities)
      ) {
        return;
      }
      cancelPendingClear();
      applyHover(node);
    },
    [applyHover, cancelPendingClear, isPanningRef]
  );
  const handleNodeMouseLeave = useCallback(() => {
    if (!pinnedHoverNodeIdRef.current) {
      cancelPendingClear();
      pendingClearTimeoutRef.current = window.setTimeout(() => {
        pendingClearTimeoutRef.current = null;
        if (!pinnedHoverNodeIdRef.current) clearHover();
      }, 50);
    }
  }, [cancelPendingClear, clearHover]);
  const pinHover = useCallback(
    (node: ContextGraphNode) => {
      if (!isHoverableNode(node.data.activities)) return;
      cancelPendingClear();
      pinnedHoverNodeIdRef.current = node.id;
      applyHover(node);
    },
    [applyHover, cancelPendingClear]
  );
  useEffect(
    () => () => {
      cancelPendingClear();
      pinnedHoverNodeIdRef.current = null;
      clearHover();
    },
    [cancelPendingClear, clearHover, hoverIndex]
  );
  return { handleNodeMouseEnter, handleNodeMouseLeave, pinHover, releaseHover };
}

export function resetGraphViewport(
  event: React.MouseEvent,
  reactFlowInstance: ReactFlowInstance<ContextGraphNode, ContextGraphEdge> | null,
  graphKey: string | null,
  viewportByGraphKey: Map<string, Viewport>
) {
  if (
    (event.target instanceof Element &&
      event.target.closest(".react-flow__node, .react-flow__edge")) ||
    !reactFlowInstance
  )
    return;
  if (graphKey) viewportByGraphKey.delete(graphKey);
  void reactFlowInstance.fitView({ ...FIT_VIEW_OPTIONS, duration: 0 });
}

export function createNodePopover(
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

export function expandNodePopover(popover: NodePopoverState | null, shell: HTMLDivElement | null) {
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

function applyGraphHover(
  shell: HTMLDivElement | null,
  hoverElements: Set<Element>,
  relatedNodeIds: Set<string>,
  relatedEdgeIds: Set<string>
) {
  if (!shell) return;
  shell.classList.add(styles.hovering);
  const nextHoverElements = new Set<Element>();
  for (const nodeId of relatedNodeIds) {
    addHoverElement(shell, nextHoverElements, ".react-flow__node", nodeId);
  }
  for (const edgeId of relatedEdgeIds) {
    addHoverElement(shell, nextHoverElements, ".react-flow__edge", edgeId);
  }
  for (const element of hoverElements) {
    if (!nextHoverElements.has(element)) element.classList.remove(styles.relatedElement);
  }
  for (const element of nextHoverElements) element.classList.add(styles.relatedElement);
  hoverElements.clear();
  for (const element of nextHoverElements) hoverElements.add(element);
}

function addHoverElement(
  shell: HTMLDivElement,
  elements: Set<Element>,
  selector: string,
  id: string
) {
  const element = shell.querySelector(`${selector}[data-id="${CSS.escape(id)}"]`);
  if (element) elements.add(element);
}

function clearGraphHover(shell: HTMLDivElement | null, elements: Set<Element>) {
  shell?.classList.remove(styles.hovering);
  for (const element of elements) element.classList.remove(styles.relatedElement);
  elements.clear();
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

function isHoverableNode(activities: ContextGraphNodeData["activities"]) {
  return activities.length > 0;
}
