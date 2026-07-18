import { useRef } from "react";
import type { KeyboardEvent, PointerEvent } from "react";

import styles from "./HorizontalResizeHandle.module.css";

const KEYBOARD_RESIZE_STEP = 16;

function clampWidth(width: number, minWidth: number, maxWidth: number) {
  return Math.min(Math.max(width, minWidth), maxWidth);
}

export function HorizontalResizeHandle({
  edge,
  maxWidth,
  minWidth,
  onResize,
  width,
}: {
  edge: "start" | "end";
  maxWidth: number;
  minWidth: number;
  onResize: (width: number) => void;
  width: number;
}) {
  const dragRef = useRef<{ pointerId: number; startWidth: number; startX: number } | null>(null);
  const direction = edge === "start" ? -1 : 1;

  function stopResize(event: PointerEvent<HTMLDivElement>) {
    if (dragRef.current?.pointerId !== event.pointerId) {
      return;
    }

    dragRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") {
      return;
    }

    event.preventDefault();
    const pointerDelta = event.key === "ArrowLeft" ? -KEYBOARD_RESIZE_STEP : KEYBOARD_RESIZE_STEP;
    onResize(clampWidth(width + pointerDelta * direction, minWidth, maxWidth));
  }

  return (
    <div
      aria-label="Resize panel"
      aria-orientation="vertical"
      aria-valuemax={maxWidth}
      aria-valuemin={minWidth}
      aria-valuenow={width}
      className={styles.handle}
      data-edge={edge}
      role="separator"
      tabIndex={0}
      onDragStart={(event) => event.preventDefault()}
      onKeyDown={handleKeyDown}
      onPointerCancel={stopResize}
      onPointerDown={(event) => {
        if (event.button !== 0) {
          return;
        }

        event.preventDefault();
        event.stopPropagation();
        dragRef.current = {
          pointerId: event.pointerId,
          startWidth: width,
          startX: event.clientX,
        };
        event.currentTarget.setPointerCapture(event.pointerId);
      }}
      onPointerMove={(event) => {
        const drag = dragRef.current;
        if (!drag || drag.pointerId !== event.pointerId) {
          return;
        }

        const nextWidth = drag.startWidth + (event.clientX - drag.startX) * direction;
        onResize(clampWidth(nextWidth, minWidth, maxWidth));
      }}
      onPointerUp={stopResize}
    />
  );
}
