import { useEffect, useState } from "react";
import { Resizable } from "react-resizable";

import { FileDiffViewer } from "@/features/session-dashboard/components/FileDiffViewer";
import {
  DEFAULT_DIFF_PANEL_WIDTH,
  MIN_DIFF_PANEL_WIDTH,
  WIDE_LAYOUT_MEDIA_QUERY,
} from "@/features/session-dashboard/constants";
import { useMediaQuery } from "@/features/session-dashboard/hooks/useMediaQuery";
import type { useSessionFileDiff } from "@/features/session-dashboard/hooks/useSessionFileDiff";
import { useViewportWidth } from "@/features/session-dashboard/hooks/useViewportWidth";
import { clampNumber, getMaxDiffPanelWidth } from "@/features/session-dashboard/layout";
import type { SelectedActivityFile } from "@/features/session-dashboard/lib/session-watch";
import type { useEditorTheme } from "@/shared/hooks/useEditorTheme";
import { cn } from "@/shared/lib/utils";

import styles from "./SessionFileViewer.module.css";

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
    if (!isDiffViewerOpen) {
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
  }, [isDiffViewerOpen, onClose]);

  if (!isDiffViewerOpen) {
    return null;
  }

  if (!isWideLayout) {
    return (
      <div
        className={cn(
          styles.modal,
          "fixed inset-0 z-30 flex items-stretch px-3 pt-13 pb-3 backdrop-blur-[10px] sm:px-6 sm:pt-16 sm:pb-6"
        )}
        role="dialog"
        aria-modal="true"
        onMouseDown={(event) => {
          if (event.target === event.currentTarget) {
            onClose();
          }
        }}>
        <div className={cn(styles.modalPanel, "flex min-h-0 w-full")}>{diffViewer}</div>
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
      <div
        className={cn(styles.diffPanel, "flex min-w-0 flex-none opacity-0")}
        style={{ width: resolvedDiffPanelWidth }}>
        {diffViewer}
      </div>
    </Resizable>
  );
}
