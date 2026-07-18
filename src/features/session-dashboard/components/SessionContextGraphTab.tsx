import { useState } from "react";
import { Resizable } from "react-resizable";

import { useSessionDetails } from "@/features/session-dashboard/hooks/useSessionDetails";
import { useViewportWidth } from "@/features/session-dashboard/hooks/useViewportWidth";
import type {
  AgentSessionSummary,
  SelectedActivityFile,
} from "@/features/session-dashboard/lib/session-watch";
import type { DescriptionSettings } from "@/shared/lib/generated/bindings";
import { cn } from "@/shared/lib/utils";

import styles from "./SessionContextGraphTab.module.css";
import { SessionContextGraphView } from "./SessionContextGraphView";
import { SessionPromptPanel, type SessionScopeSelection } from "./SessionPromptPanel";

const DEFAULT_PROMPT_PANEL_WIDTH = 360;
const MIN_PROMPT_PANEL_WIDTH = 280;
const MAX_PROMPT_PANEL_WIDTH = 620;
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
}: {
  descriptionSettings: DescriptionSettings;
  showReadFiles: boolean;
  isSessionListLoading: boolean;
  selectedActivityFile: SelectedActivityFile | null;
  selectedSession: AgentSessionSummary;
  onScopeChange: () => void;
  onSelectFile: (selection: SelectedActivityFile) => void;
}) {
  const [promptPanelWidth, setPromptPanelWidth] = useState(DEFAULT_PROMPT_PANEL_WIDTH);
  const [scopeSelection, setScopeSelection] = useState<SessionScopeSelection | null>(null);
  const viewportWidth = useViewportWidth();
  const detailsQuery = useSessionDetails(selectedSession);
  const turns = detailsQuery.data?.turns ?? [];
  const latestTurn = turns[turns.length - 1];
  const resolvedSelection =
    scopeSelection && detailsQuery.data?.turns.some((turn) => turn.id === scopeSelection.turnId)
      ? scopeSelection
      : latestTurn
        ? { turnId: latestTurn.id, taskId: null }
        : null;
  const selectedTurn = detailsQuery.data?.turns.find(
    (turn) => turn.id === resolvedSelection?.turnId
  );
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
  const scopeLabel =
    selectedTask?.subject ?? (selectedTurn ? "Prompt activity" : "Session activity");

  return (
    <div className="absolute inset-0 flex min-h-0">
      <Resizable
        axis="x"
        width={resolvedPanelWidth}
        height={0}
        minConstraints={[MIN_PROMPT_PANEL_WIDTH, 0]}
        maxConstraints={[maxPanelWidth, 0]}
        resizeHandles={["e"]}
        onResize={(_event, data) => setPromptPanelWidth(data.size.width)}>
        <div
          className={cn(styles.promptPanel, "relative h-full min-h-0 flex-none")}
          style={{ width: resolvedPanelWidth }}>
          <SessionPromptPanel
            details={detailsQuery.data}
            isLoading={detailsQuery.isPending}
            selectedScope={resolvedSelection}
            sessionTitle={selectedSession.title}
            onSelectScope={(selection) => {
              setScopeSelection(selection);
              onScopeChange();
            }}
          />
        </div>
      </Resizable>
      <div className="relative min-w-0 flex-1">
        <div
          className={cn(
            styles.graphTitle,
            "border-border text-card-foreground absolute top-4 left-4 z-[6] max-w-[calc(100%-2rem)] truncate rounded-lg border px-3 py-2.5 text-sm leading-5 font-medium"
          )}>
          {scopeLabel}
        </div>
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
