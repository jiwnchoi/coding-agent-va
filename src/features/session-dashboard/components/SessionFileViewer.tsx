import { useEffect, useState } from "react";
import { Resizable } from "react-resizable";

import { FileDiffViewer } from "@/components/file-diff-viewer";
import {
  DEFAULT_DIFF_PANEL_WIDTH,
  MIN_DIFF_PANEL_WIDTH,
  WIDE_LAYOUT_MEDIA_QUERY,
} from "@/features/session-dashboard/constants";
import { useMediaQuery } from "@/features/session-dashboard/hooks/useMediaQuery";
import { useViewportWidth } from "@/features/session-dashboard/hooks/useViewportWidth";
import { clampNumber, getMaxDiffPanelWidth } from "@/features/session-dashboard/layout";
import type { SelectedActivityFile } from "@/lib/session-watch";
import type { useEditorTheme } from "@/lib/useEditorTheme";
import type { useSessionFileDiff } from "@/lib/useSessionFileDiff";

export function SessionFileViewer({
  errorMessage,
  isLoading,
  onClose,
  selectedActivityFile,
  selectedFileDiff,
  theme,
}: {
  errorMessage: string;
  isLoading: boolean;
  onClose: () => void;
  selectedActivityFile: SelectedActivityFile | null;
  selectedFileDiff: ReturnType<typeof useSessionFileDiff>["selectedFileDiff"];
  theme: ReturnType<typeof useEditorTheme>;
}) {
  const [diffPanelWidth, setDiffPanelWidth] = useState(DEFAULT_DIFF_PANEL_WIDTH);
  const viewportWidth = useViewportWidth();
  const isWideLayout = useMediaQuery(WIDE_LAYOUT_MEDIA_QUERY);
  const isDiffViewerOpen = selectedActivityFile !== null;
  const maxDiffPanelWidth = getMaxDiffPanelWidth(viewportWidth);
  const resolvedDiffPanelWidth = clampNumber(
    diffPanelWidth,
    MIN_DIFF_PANEL_WIDTH,
    maxDiffPanelWidth
  );
  const diffViewer = selectedActivityFile ? (
    <FileDiffViewer
      diff={selectedFileDiff}
      isLoading={isLoading}
      errorMessage={errorMessage}
      onClose={onClose}
      theme={theme}
      viewerMode={
        selectedActivityFile.activityKey === "edited" ||
        selectedActivityFile.activityKey === "deleted"
          ? "diff"
          : "read"
      }
    />
  ) : null;

  useEffect(() => {
    setDiffPanelWidth((currentWidth) =>
      clampNumber(currentWidth, MIN_DIFF_PANEL_WIDTH, maxDiffPanelWidth)
    );
  }, [maxDiffPanelWidth]);

  useEffect(() => {
    if (!isDiffViewerOpen || isWideLayout) {
      return;
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onClose();
      }
    }

    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [isDiffViewerOpen, isWideLayout, onClose]);

  if (!isDiffViewerOpen) {
    return null;
  }

  if (!isWideLayout) {
    return (
      <div
        className="app-diff-modal"
        role="dialog"
        aria-modal="true"
        onMouseDown={(event) => {
          if (event.target === event.currentTarget) {
            onClose();
          }
        }}>
        <div className="app-diff-modal-panel">{diffViewer}</div>
      </div>
    );
  }

  return (
    <Resizable
      axis="x"
      width={resolvedDiffPanelWidth}
      height={0}
      minConstraints={[MIN_DIFF_PANEL_WIDTH, 0]}
      maxConstraints={[maxDiffPanelWidth, 0]}
      resizeHandles={["w"]}
      onResize={(_event, data) => {
        setDiffPanelWidth(clampNumber(data.size.width, MIN_DIFF_PANEL_WIDTH, maxDiffPanelWidth));
      }}>
      <div className="app-diff-panel" style={{ width: resolvedDiffPanelWidth }}>
        {diffViewer}
      </div>
    </Resizable>
  );
}
