import { Files, ListTodo, MessageSquareText } from "lucide-react";
import { useState } from "react";

import type {
  SelectedActivityFile,
  SessionScopeSelection,
} from "@/features/session-dashboard/lib/session-watch";
import type { AgentSessionDetails } from "@/shared/lib/generated/bindings";
import { cn } from "@/shared/lib/utils";

import { buildEventRows, EventSequenceVisualization } from "./EventSequenceVisualization";

type TrackingMode = "prompts" | "tasks";

export function SessionPromptPanel({
  details,
  isLoading,
  selectedScope,
  hoveredFilePaths,
  showReadFiles,
  workspacePath,
  onSelectScope,
  onSelectFile,
  onHoverFilePaths,
  onShowReadFilesChange,
}: {
  details: AgentSessionDetails | undefined;
  isLoading: boolean;
  selectedScope: SessionScopeSelection | null;
  hoveredFilePaths: string[] | null;
  showReadFiles: boolean;
  workspacePath: string | null;
  onSelectScope: (selection: SessionScopeSelection | null) => void;
  onSelectFile: (selection: SelectedActivityFile) => void;
  onHoverFilePaths: (filePaths: string[] | null) => void;
  onShowReadFilesChange: (showReadFiles: boolean) => void;
}) {
  const [trackingMode, setTrackingMode] = useState<TrackingMode>("prompts");
  const turns = details?.turns ?? [];
  const eventRows = buildEventRows(turns, trackingMode);

  function selectTrackingMode(mode: TrackingMode) {
    setTrackingMode(mode);
    onSelectScope(null);
  }

  return (
    <aside className="bg-card border-border flex h-full min-h-0 flex-col border-l">
      <div className="border-border grid grid-cols-2 border-b p-1" role="tablist">
        <button
          type="button"
          role="tab"
          aria-selected={trackingMode === "prompts"}
          className={cn(
            "hover:bg-accent inline-flex items-center justify-center gap-1.5 rounded-md px-2 py-1.5 text-sm transition-colors",
            trackingMode === "prompts" && "bg-accent font-medium"
          )}
          onClick={() => selectTrackingMode("prompts")}>
          <MessageSquareText className="size-3.5" /> Prompts
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={trackingMode === "tasks"}
          className={cn(
            "hover:bg-accent inline-flex items-center justify-center gap-1.5 rounded-md px-2 py-1.5 text-sm transition-colors",
            trackingMode === "tasks" && "bg-accent font-medium"
          )}
          onClick={() => selectTrackingMode("tasks")}>
          <ListTodo className="size-3.5" /> Tasks
        </button>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto px-3 py-1">
        {isLoading ? (
          <p className="text-muted-foreground px-2 py-4 text-sm">Loading activity…</p>
        ) : (
          <EventSequenceVisualization
            rows={eventRows}
            showReadFiles={showReadFiles}
            selectedScope={selectedScope}
            hoveredFilePaths={hoveredFilePaths}
            workspacePath={workspacePath}
            onSelectScope={onSelectScope}
            onSelectFile={onSelectFile}
            onHoverFilePaths={onHoverFilePaths}
          />
        )}
      </div>
      <div className="border-border flex shrink-0 items-center gap-1 border-t p-1">
        <button
          type="button"
          aria-pressed={selectedScope === null}
          className={cn(
            "hover:bg-accent flex min-w-0 flex-1 items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm transition-colors",
            selectedScope === null && "bg-accent font-medium"
          )}
          onClick={() => onSelectScope(null)}>
          <Files className="size-3.5 shrink-0" />
          <span className="truncate">All changes</span>
        </button>
        <label className="flex shrink-0 cursor-pointer items-center gap-1.5 px-1 text-xs">
          <span className="text-muted-foreground">Read files</span>
          <input
            type="checkbox"
            aria-label="Show read files"
            checked={showReadFiles}
            onChange={(event) => onShowReadFilesChange(event.target.checked)}
            className="peer sr-only"
          />
          <span className="bg-muted peer-checked:bg-primary relative block h-5 w-9 rounded-full transition-colors after:absolute after:top-0.5 after:left-0.5 after:size-4 after:rounded-full after:bg-white after:shadow-sm after:transition-transform peer-checked:after:translate-x-4" />
        </label>
      </div>
    </aside>
  );
}
