import { Code2, LoaderCircle, RotateCw, Sparkles, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { MdxDescription } from "@/features/session-dashboard/components/MdxDescription";
import styles from "@/features/session-dashboard/context-graph/ContextGraphView.module.css";
import { Button } from "@/shared/components/ui/button";
import { cn } from "@/shared/lib/utils";

export function NodeActionPopover({
  description,
  errorMessage,
  isLoading,
  label,
  position,
  providerLabel,
  onClose,
  onDescribe,
  onOpenCode,
}: {
  description: string;
  errorMessage: string;
  isLoading: boolean;
  label: string;
  position: { x: number; y: number };
  providerLabel: string;
  onClose: () => void;
  onDescribe: () => void;
  onOpenCode: () => void;
}) {
  const popoverRef = useRef<HTMLDivElement>(null);
  const dragStateRef = useRef<{
    offsetX: number;
    offsetY: number;
    pointerId: number;
    startX: number;
    startY: number;
  } | null>(null);
  const [dragOffset, setDragOffset] = useState({ x: 0, y: 0 });
  const hasResult = Boolean(description || errorMessage || isLoading);

  function handleDragStart(event: React.PointerEvent<HTMLDivElement>) {
    if ((event.target as HTMLElement).closest("button")) return;
    dragStateRef.current = {
      offsetX: dragOffset.x,
      offsetY: dragOffset.y,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function handleDragMove(event: React.PointerEvent<HTMLDivElement>) {
    const dragState = dragStateRef.current;
    const popover = popoverRef.current;
    const container = popover?.offsetParent as HTMLElement | null;
    if (!dragState || dragState.pointerId !== event.pointerId || !popover || !container) return;

    const nextX = position.x + dragState.offsetX + event.clientX - dragState.startX;
    const nextY = position.y + dragState.offsetY + event.clientY - dragState.startY;
    setDragOffset({
      x: Math.max(8, Math.min(nextX, container.clientWidth - popover.offsetWidth - 8)) - position.x,
      y:
        Math.max(8, Math.min(nextY, container.clientHeight - popover.offsetHeight - 8)) -
        position.y,
    });
  }

  function handleDragEnd(event: React.PointerEvent<HTMLDivElement>) {
    if (dragStateRef.current?.pointerId !== event.pointerId) return;
    dragStateRef.current = null;
    event.currentTarget.releasePointerCapture(event.pointerId);
  }

  useEffect(() => {
    const popover = popoverRef.current;
    const container = popover?.offsetParent as HTMLElement | null;
    if (!popover || !container) return;
    const containerElement = container;

    function keepPopoverInsideContainer() {
      const currentPopover = popoverRef.current;
      if (!currentPopover) return;
      const nextLeft = Math.max(
        8,
        Math.min(
          currentPopover.offsetLeft,
          containerElement.clientWidth - currentPopover.offsetWidth - 8
        )
      );
      const nextTop = Math.max(
        8,
        Math.min(
          currentPopover.offsetTop,
          containerElement.clientHeight - currentPopover.offsetHeight - 8
        )
      );
      setDragOffset((currentOffset) => {
        const nextOffset = { x: nextLeft - position.x, y: nextTop - position.y };
        return currentOffset.x === nextOffset.x && currentOffset.y === nextOffset.y
          ? currentOffset
          : nextOffset;
      });
    }

    const resizeObserver = new ResizeObserver(keepPopoverInsideContainer);
    resizeObserver.observe(popover);
    resizeObserver.observe(containerElement);
    keepPopoverInsideContainer();
    return () => resizeObserver.disconnect();
  }, [position.x, position.y]);

  useEffect(() => {
    function handlePointerDown(event: PointerEvent) {
      if (!popoverRef.current?.contains(event.target as globalThis.Node)) {
        onClose();
      }
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onClose();
      }
    }

    if (!hasResult) {
      window.addEventListener("pointerdown", handlePointerDown);
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("pointerdown", handlePointerDown);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [hasResult, onClose]);

  return (
    <div
      ref={popoverRef}
      role="dialog"
      aria-label={`Actions for ${label}`}
      className={cn(
        styles.nodePopover,
        "bg-popover text-popover-foreground border-border absolute z-[1000] overflow-hidden rounded-lg border shadow-xl",
        hasResult ? styles.nodePopoverResult : styles.nodePopoverActions
      )}
      style={{ left: position.x + dragOffset.x, top: position.y + dragOffset.y }}
      onPointerDown={(event) => event.stopPropagation()}>
      {hasResult ? (
        <header
          className="border-border flex cursor-grab touch-none items-center gap-2 border-b px-3 py-2.5 active:cursor-grabbing"
          onPointerDown={handleDragStart}
          onPointerMove={handleDragMove}
          onPointerUp={handleDragEnd}
          onPointerCancel={handleDragEnd}>
          <p className="min-w-0 flex-1 truncate text-sm font-medium">{label}</p>
          {providerLabel ? (
            <span className="text-muted-foreground text-[0.6875rem]">via {providerLabel}</span>
          ) : null}
          <Button variant="ghost" size="icon-xs" aria-label="Close" onClick={onClose}>
            <X />
          </Button>
        </header>
      ) : null}

      {!hasResult ? (
        <div className="flex gap-1 p-1">
          <Button variant="ghost" size="sm" onClick={onDescribe}>
            <Sparkles />
            Describe
          </Button>
          <Button variant="ghost" size="sm" onClick={onOpenCode}>
            <Code2 />
            Open
          </Button>
        </div>
      ) : (
        <div className="max-h-[min(32rem,70vh)] overflow-y-auto p-4">
          {isLoading && !description ? (
            <div className="text-muted-foreground flex min-h-28 items-center justify-center gap-2 text-sm">
              <LoaderCircle className="size-4 animate-spin" />
              Describing graph changes...
            </div>
          ) : errorMessage ? (
            <div className="space-y-3">
              <p className="text-destructive text-sm whitespace-pre-wrap">{errorMessage}</p>
              <Button variant="outline" size="sm" onClick={onDescribe}>
                <RotateCw /> Retry
              </Button>
            </div>
          ) : description ? (
            <div>
              <div className={cn(styles.nodeDescription, "text-sm leading-6")}>
                <MdxDescription source={description} />
              </div>
              {isLoading ? (
                <div className="text-muted-foreground mt-3 flex items-center gap-2 text-xs">
                  <LoaderCircle className="size-3 animate-spin" />
                  Generating...
                </div>
              ) : null}
            </div>
          ) : null}
        </div>
      )}
    </div>
  );
}
