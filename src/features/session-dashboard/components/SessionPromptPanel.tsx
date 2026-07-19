import { Check, Circle, Files, ListTodo, LoaderCircle, MessageSquareText } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { messagePreview } from "@/features/session-dashboard/lib/session-message";
import { buildSessionScopeSelection } from "@/features/session-dashboard/lib/session-scope";
import type {
  SelectedActivityFile,
  SessionScopeSelection,
} from "@/features/session-dashboard/lib/session-watch";
import type { AgentSessionDetails, AgentSessionTaskStatus } from "@/shared/lib/generated/bindings";
import { cn } from "@/shared/lib/utils";

import { FileActivityMetrics } from "./FileActivityMetrics";
import { SessionMarkdownMessage } from "./SessionMarkdownMessage";
import styles from "./SessionPromptPanel.module.css";

type TrackingMode = "prompts" | "tasks";

const STATUS_ICON = {
  completed: Check,
  in_progress: LoaderCircle,
  pending: Circle,
} satisfies Record<AgentSessionTaskStatus, typeof Circle>;
export function SessionPromptPanel({
  details,
  isLoading,
  selectedScope,
  showReadFiles,
  sessionTitle,
  workspacePath,
  onSelectScope,
  onSelectFile,
  onShowReadFilesChange,
}: {
  details: AgentSessionDetails | undefined;
  isLoading: boolean;
  selectedScope: SessionScopeSelection | null;
  showReadFiles: boolean;
  sessionTitle: string;
  workspacePath: string | null;
  onSelectScope: (selection: SessionScopeSelection | null) => void;
  onSelectFile: (selection: SelectedActivityFile) => void;
  onShowReadFilesChange: (showReadFiles: boolean) => void;
}) {
  const [trackingMode, setTrackingMode] = useState<TrackingMode>("prompts");
  const [expandedMessages, setExpandedMessages] = useState<Set<string>>(() => new Set());
  const activityScrollRef = useRef<HTMLDivElement>(null);
  const turns = details?.turns ?? [];
  const promptTurns = turns.filter(
    (turn) => turn.fileActivity.editedFiles.length > 0 || turn.fileActivity.deletedFiles.length > 0
  );
  const taskEntries = turns.flatMap((turn) =>
    turn.tasks.map((task) => ({ prompts: turn.prompts, task, turnId: turn.id }))
  );

  useEffect(() => {
    const scrollElement = activityScrollRef.current;
    if (scrollElement) {
      scrollElement.scrollTop = scrollElement.scrollHeight;
    }
  }, [details?.turns, trackingMode]);

  function toggleMessage(messageId: string) {
    setExpandedMessages((current) => {
      const next = new Set(current);
      if (next.has(messageId)) next.delete(messageId);
      else next.add(messageId);
      return next;
    });
  }

  function selectTrackingMode(mode: TrackingMode) {
    setTrackingMode(mode);
    onSelectScope(null);
  }

  return (
    <aside className="bg-card border-border flex h-full min-h-0 flex-col border-r">
      <div className="border-border border-b px-4 py-3">
        <p className="truncate text-sm font-medium">{sessionTitle}</p>
        <p className="text-muted-foreground mt-0.5 text-xs">Session activity</p>
      </div>
      <div className="border-border flex items-center gap-2 border-b p-2">
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
      <div className="border-border grid grid-cols-2 border-b p-2" role="tablist">
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
      <div
        ref={activityScrollRef}
        className="min-h-0 flex-1 [scrollbar-gutter:stable] overflow-y-auto py-3 pr-0 pl-3">
        {isLoading ? (
          <p className="text-muted-foreground px-2 py-4 text-sm">Loading activity…</p>
        ) : trackingMode === "prompts" ? (
          <PromptTrackingList
            turns={promptTurns}
            expandedMessages={expandedMessages}
            selectedScope={selectedScope}
            workspacePath={workspacePath}
            onSelectScope={onSelectScope}
            onSelectFile={onSelectFile}
            onToggleMessage={toggleMessage}
          />
        ) : (
          <TaskTrackingList
            entries={taskEntries}
            selectedScope={selectedScope}
            workspacePath={workspacePath}
            onSelectScope={onSelectScope}
          />
        )}
      </div>
    </aside>
  );
}

function PromptTrackingList({
  turns,
  expandedMessages,
  selectedScope,
  workspacePath,
  onSelectScope,
  onSelectFile,
  onToggleMessage,
}: {
  turns: AgentSessionDetails["turns"];
  expandedMessages: Set<string>;
  selectedScope: SessionScopeSelection | null;
  workspacePath: string | null;
  onSelectScope: (selection: SessionScopeSelection | null) => void;
  onSelectFile: (selection: SelectedActivityFile) => void;
  onToggleMessage: (messageId: string) => void;
}) {
  if (turns.length === 0) {
    return <p className="text-muted-foreground px-2 py-4 text-sm">No prompt activity found.</p>;
  }

  return (
    <ol className="space-y-3" role="tabpanel">
      {turns.map((turn) => {
        const isSelected = selectedScope?.turnId === turn.id && selectedScope.taskId === null;
        const summaryMessageId = `${turn.id}:summary`;
        const isSummaryExpanded = expandedMessages.has(summaryMessageId);
        return (
          <li key={turn.id} className={cn(styles.turn, "border-border rounded-lg border")}>
            {turn.prompts.map((prompt, promptIndex) => {
              const promptMessageId = `${turn.id}:prompt:${promptIndex}`;
              const isPromptExpanded = expandedMessages.has(promptMessageId);
              return (
                <SessionMarkdownMessage
                  key={promptMessageId}
                  source={prompt}
                  isExpanded={isPromptExpanded}
                  isPressed={isSelected}
                  className={cn(
                    promptIndex > 0 && "border-border border-t",
                    isSelected && "bg-accent"
                  )}
                  header={
                    promptIndex === 0 ? (
                      <span className="flex items-center justify-between gap-2">
                        <span className="text-muted-foreground flex items-center gap-1.5 text-xs font-medium">
                          <MessageSquareText className="size-3.5" /> User prompt
                        </span>
                        <FileActivityMetrics
                          fileActivity={turn.fileActivity}
                          workspacePath={workspacePath}
                        />
                      </span>
                    ) : null
                  }
                  onPress={() => {
                    onSelectScope(
                      isSelected ? null : buildSessionScopeSelection(turn.id, null, turn)
                    );
                  }}
                  onToggle={() => onToggleMessage(promptMessageId)}
                  onOpenFile={onSelectFile}
                  fileActivity={turn.fileActivity}
                  workspacePath={workspacePath}
                />
              );
            })}
            {turn.summary ? (
              <SessionMarkdownMessage
                source={turn.summary}
                isExpanded={isSummaryExpanded}
                className="border-border border-t"
                header={
                  <span className="text-muted-foreground block text-xs font-medium">
                    End summary
                  </span>
                }
                onToggle={() => onToggleMessage(summaryMessageId)}
                onOpenFile={onSelectFile}
                fileActivity={turn.fileActivity}
                workspacePath={workspacePath}
              />
            ) : null}
          </li>
        );
      })}
    </ol>
  );
}

function TaskTrackingList({
  entries,
  selectedScope,
  workspacePath,
  onSelectScope,
}: {
  entries: Array<{
    prompts: string[];
    task: AgentSessionDetails["turns"][number]["tasks"][number];
    turnId: string;
  }>;
  selectedScope: SessionScopeSelection | null;
  workspacePath: string | null;
  onSelectScope: (selection: SessionScopeSelection | null) => void;
}) {
  if (entries.length === 0) {
    return <p className="text-muted-foreground px-2 py-4 text-sm">No task activity found.</p>;
  }

  return (
    <ol className="space-y-2" role="tabpanel">
      {entries.map(({ prompts, task, turnId }) => {
        const Icon = STATUS_ICON[task.status];
        const isSelected = selectedScope?.taskId === task.id;
        return (
          <li key={task.id} className={cn(styles.turn, "border-border rounded-lg border")}>
            <button
              type="button"
              className={cn(
                "hover:bg-accent w-full rounded-lg px-3 py-3 text-left transition-colors",
                isSelected && "bg-accent"
              )}
              aria-pressed={isSelected}
              onClick={() =>
                onSelectScope(isSelected ? null : buildSessionScopeSelection(turnId, task.id, task))
              }>
              <span className="flex items-start gap-2 text-sm">
                <Icon
                  className={cn(
                    "mt-0.5 size-3.5 shrink-0",
                    task.status === "in_progress" && "animate-spin"
                  )}
                />
                <span className="min-w-0 flex-1">
                  <span className="flex items-start justify-between gap-2">
                    <span className="min-w-0 font-medium">{task.subject}</span>
                    <FileActivityMetrics
                      fileActivity={task.fileActivity}
                      workspacePath={workspacePath}
                    />
                  </span>
                  <span className="text-muted-foreground mt-1 block truncate text-xs">
                    {messagePreview(prompts[0] ?? "")}
                  </span>
                </span>
              </span>
            </button>
          </li>
        );
      })}
    </ol>
  );
}
