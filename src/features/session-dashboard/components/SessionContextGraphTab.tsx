import { useState } from "react";

import {
  MAX_PROMPT_PANEL_WIDTH,
  MIN_PROMPT_PANEL_WIDTH,
} from "@/features/session-dashboard/constants";
import { useDashboardLayout } from "@/features/session-dashboard/hooks/useDashboardLayout";
import { useSessionDetails } from "@/features/session-dashboard/hooks/useSessionDetails";
import { useViewportWidth } from "@/features/session-dashboard/hooks/useViewportWidth";
import type {
  AgentSessionSummary,
  SelectedActivityFile,
} from "@/features/session-dashboard/lib/session-watch";
import { HorizontalResizeHandle } from "@/shared/components/HorizontalResizeHandle";
import type { DescriptionSettings } from "@/shared/lib/generated/bindings";
import { cn } from "@/shared/lib/utils";

import styles from "./SessionContextGraphTab.module.css";
import { SessionContextGraphView } from "./SessionContextGraphView";
import { SessionPromptPanel, type SessionScopeSelection } from "./SessionPromptPanel";

const MIN_GRAPH_WIDTH = 420;
const EMPTY_FILE_ACTIVITY = {
  readFiles: [],
  editedFiles: [],
  impactedFiles: [],
  deletedFiles: [],
  impactedRelations: [],
};

export function SessionContextGraphTab({
  descriptionSettings,
  showReadFiles,
  isSessionListLoading,
  selectedActivityFile,
  selectedSession,
  onScopeChange,
  onSelectFile,
  onShowReadFilesChange,
}: {
  descriptionSettings: DescriptionSettings;
  showReadFiles: boolean;
  isSessionListLoading: boolean;
  selectedActivityFile: SelectedActivityFile | null;
  selectedSession: AgentSessionSummary;
  onScopeChange: () => void;
  onSelectFile: (selection: SelectedActivityFile) => void;
  onShowReadFilesChange: (showReadFiles: boolean) => void;
}) {
  const promptPanelWidth = useDashboardLayout((state) => state.promptPanelWidth);
  const setPromptPanelWidth = useDashboardLayout((state) => state.setPromptPanelWidth);
  const [scopeSelection, setScopeSelection] = useState<SessionScopeSelection | null>(null);
  const viewportWidth = useViewportWidth();
  const detailsQuery = useSessionDetails(selectedSession);
  const turns = detailsQuery.data?.turns ?? [];
  const resolvedSelection =
    scopeSelection &&
    turns.some(
      (turn) =>
        turn.id === scopeSelection.turnId &&
        (scopeSelection.taskId === null ||
          turn.tasks.some((task) => task.id === scopeSelection.taskId))
    )
      ? scopeSelection
      : null;
  const selectedTurn = turns.find((turn) => turn.id === resolvedSelection?.turnId);
  const selectedTask = selectedTurn?.tasks.find((task) => task.id === resolvedSelection?.taskId);
  const scopedActivity =
    selectedTask?.fileActivity ??
    selectedTurn?.fileActivity ??
    detailsQuery.data?.fileActivity ??
    EMPTY_FILE_ACTIVITY;
  const maxPanelWidth = Math.max(
    MIN_PROMPT_PANEL_WIDTH,
    Math.min(MAX_PROMPT_PANEL_WIDTH, viewportWidth - MIN_GRAPH_WIDTH)
  );
  const resolvedPanelWidth = Math.min(promptPanelWidth, maxPanelWidth);
  return (
    <div className="absolute inset-0 flex min-h-0">
      <div
        className={cn(styles.promptPanel, "relative h-full min-h-0 flex-none")}
        style={{ width: resolvedPanelWidth }}>
        <SessionPromptPanel
          details={detailsQuery.data}
          isLoading={detailsQuery.isPending}
          selectedScope={resolvedSelection}
          showReadFiles={showReadFiles}
          sessionTitle={selectedSession.title}
          workspacePath={selectedSession.cwd}
          onSelectScope={(selection) => {
            setScopeSelection(selection);
            onScopeChange();
          }}
          onSelectFile={onSelectFile}
          onShowReadFilesChange={onShowReadFilesChange}
        />
        <HorizontalResizeHandle
          edge="end"
          maxWidth={maxPanelWidth}
          minWidth={MIN_PROMPT_PANEL_WIDTH}
          width={resolvedPanelWidth}
          onResize={setPromptPanelWidth}
        />
      </div>
      <div className="relative min-w-0 flex-1">
        <SessionContextGraphView
          descriptionSettings={descriptionSettings}
          fileActivity={scopedActivity}
          graphScopeKey={`${resolvedSelection?.turnId ?? "session"}:${resolvedSelection?.taskId ?? "prompt"}`}
          isFileActivityLoading={isSessionListLoading || detailsQuery.isPending}
          selectedActivityFile={selectedActivityFile}
          selectedSession={selectedSession}
          showReadFiles={showReadFiles}
          onSelectFile={onSelectFile}
        />
      </div>
    </div>
  );
}
