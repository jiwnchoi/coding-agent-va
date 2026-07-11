import { useEffect, useRef, useState } from "react";

import type { ContextGraphNode } from "./types";

const LAYOUT_ANIMATION_DURATION = 420;

export function useAnimatedContextGraphNodes(targetNodes: ContextGraphNode[], graphKey: string) {
  const [renderedNodes, setRenderedNodes] = useState(targetNodes);
  const renderedNodesRef = useRef(renderedNodes);
  const renderedGraphKeyRef = useRef(graphKey);
  const graphChanged = renderedGraphKeyRef.current !== graphKey;

  useEffect(() => {
    if (renderedGraphKeyRef.current !== graphKey) {
      renderedGraphKeyRef.current = graphKey;
      renderedNodesRef.current = targetNodes;
      setRenderedNodes(targetNodes);
      return;
    }

    const previousNodeById = new Map(renderedNodesRef.current.map((node) => [node.id, node]));
    const hasMovedNode = targetNodes.some((node) => {
      const previousNode = previousNodeById.get(node.id);
      return (
        previousNode &&
        (previousNode.position.x !== node.position.x || previousNode.position.y !== node.position.y)
      );
    });

    if (!hasMovedNode || window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      renderedNodesRef.current = targetNodes;
      setRenderedNodes(targetNodes);
      return;
    }

    const startedAt = performance.now();
    let animationFrameId = requestAnimationFrame(animateLayout);

    function animateLayout(timestamp: number) {
      const progress = Math.min(1, (timestamp - startedAt) / LAYOUT_ANIMATION_DURATION);
      const easedProgress = 1 - Math.pow(1 - progress, 3);
      const nextNodes = targetNodes.map((node) => {
        const previousNode = previousNodeById.get(node.id);

        if (!previousNode) {
          return node;
        }

        return {
          ...node,
          position: {
            x: interpolate(previousNode.position.x, node.position.x, easedProgress),
            y: interpolate(previousNode.position.y, node.position.y, easedProgress),
          },
        };
      });

      renderedNodesRef.current = nextNodes;
      setRenderedNodes(nextNodes);

      if (progress < 1) {
        animationFrameId = requestAnimationFrame(animateLayout);
      }
    }

    return () => cancelAnimationFrame(animationFrameId);
  }, [graphKey, targetNodes]);

  return {
    isGraphSwitch: graphChanged,
    nodes: graphChanged ? targetNodes : renderedNodes,
  };
}

function interpolate(start: number, end: number, progress: number) {
  return start + (end - start) * progress;
}
