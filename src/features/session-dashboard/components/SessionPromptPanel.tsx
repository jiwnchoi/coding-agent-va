import { Check, Circle, ListTodo, LoaderCircle, MessageSquareText } from "lucide-react";
import { useState } from "react";

import type { AgentSessionDetails, AgentSessionTaskStatus } from "@/shared/lib/generated/bindings";
import { cn } from "@/shared/lib/utils";

import styles from "./SessionPromptPanel.module.css";

export type SessionScopeSelection = { turnId: string; taskId: string | null };

type TrackingMode = "prompts" | "tasks";

const STATUS_ICON = {
  completed: Check,
  in_progress: LoaderCircle,
  pending: Circle,
} satisfies Record<AgentSessionTaskStatus, typeof Circle>;
const MESSAGE_PREVIEW_LENGTH = 120;

function messagePreview(text: string) {
  const firstLine = text.split(/\r?\n/, 1)[0]?.trim() ?? "";
  const preview = firstLine.slice(0, MESSAGE_PREVIEW_LENGTH).trimEnd();
  return preview.length < firstLine.length || text.trim() !== firstLine ? `${preview}…` : preview;
}

export function SessionPromptPanel({
  details,
  isLoading,
  selectedScope,
  sessionTitle,
  onSelectScope,
}: {
  details: AgentSessionDetails | undefined;
  isLoading: boolean;
  selectedScope: SessionScopeSelection | null;
  sessionTitle: string;
  onSelectScope: (selection: SessionScopeSelection) => void;
}) {
  const [trackingMode, setTrackingMode] = useState<TrackingMode>("prompts");
  const [expandedMessages, setExpandedMessages] = useState<Set<string>>(() => new Set());
  const turns = details?.turns ?? [];
  const taskEntries = turns.flatMap((turn) =>
    turn.tasks.map((task) => ({ prompts: turn.prompts, task, turnId: turn.id }))
  );

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
    if (mode === "prompts") {
      const latestTurn = turns[turns.length - 1];
      if (latestTurn) onSelectScope({ turnId: latestTurn.id, taskId: null });
      return;
    }
    const latestTask = taskEntries[taskEntries.length - 1];
    if (latestTask) {
      onSelectScope({ turnId: latestTask.turnId, taskId: latestTask.task.id });
    }
  }

  return (
    <aside className="bg-card border-border flex h-full min-h-0 flex-col border-r">
      <div className="border-border border-b px-4 py-3">
        <p className="truncate text-sm font-medium">{sessionTitle}</p>
        <p className="text-muted-foreground mt-0.5 text-xs">Session activity</p>
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
      <div className="min-h-0 flex-1 overflow-y-auto px-3 py-3">
        {isLoading ? (
          <p className="text-muted-foreground px-2 py-4 text-sm">Loading activity…</p>
        ) : trackingMode === "prompts" ? (
          <PromptTrackingList
            turns={turns}
            expandedMessages={expandedMessages}
            selectedScope={selectedScope}
            onSelectScope={onSelectScope}
            onToggleMessage={toggleMessage}
          />
        ) : (
          <TaskTrackingList
            entries={taskEntries}
            expandedMessages={expandedMessages}
            selectedScope={selectedScope}
            onSelectScope={onSelectScope}
            onToggleMessage={toggleMessage}
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
  onSelectScope,
  onToggleMessage,
}: {
  turns: AgentSessionDetails["turns"];
  expandedMessages: Set<string>;
  selectedScope: SessionScopeSelection | null;
  onSelectScope: (selection: SessionScopeSelection) => void;
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
                <button
                  key={promptMessageId}
                  type="button"
                  className={cn(
                    "hover:bg-accent w-full px-3 py-3 text-left transition-colors",
                    promptIndex > 0 && "border-border border-t",
                    isSelected && "bg-accent"
                  )}
                  aria-pressed={isSelected}
                  aria-expanded={isPromptExpanded}
                  onClick={() => {
                    onSelectScope({ turnId: turn.id, taskId: null });
                    onToggleMessage(promptMessageId);
                  }}>
                  <span className="text-muted-foreground mb-1 flex items-center gap-1.5 text-xs font-medium">
                    <MessageSquareText className="size-3.5" /> User prompt
                  </span>
                  <span className="block text-sm whitespace-pre-wrap">
                    {isPromptExpanded ? prompt : messagePreview(prompt)}
                  </span>
                </button>
              );
            })}
            {turn.summary ? (
              <button
                type="button"
                className="border-border hover:bg-accent w-full border-t px-3 py-3 text-left transition-colors"
                aria-expanded={isSummaryExpanded}
                onClick={() => onToggleMessage(summaryMessageId)}>
                <span className="text-muted-foreground mb-1 block text-xs font-medium">
                  End summary
                </span>
                <span className="block text-sm whitespace-pre-wrap">
                  {isSummaryExpanded ? turn.summary : messagePreview(turn.summary)}
                </span>
              </button>
            ) : null}
          </li>
        );
      })}
    </ol>
  );
}

function TaskTrackingList({
  entries,
  expandedMessages,
  selectedScope,
  onSelectScope,
  onToggleMessage,
}: {
  entries: Array<{
    prompts: string[];
    task: AgentSessionDetails["turns"][number]["tasks"][number];
    turnId: string;
  }>;
  expandedMessages: Set<string>;
  selectedScope: SessionScopeSelection | null;
  onSelectScope: (selection: SessionScopeSelection) => void;
  onToggleMessage: (messageId: string) => void;
}) {
  if (entries.length === 0) {
    return <p className="text-muted-foreground px-2 py-4 text-sm">No task activity found.</p>;
  }

  return (
    <ol className="space-y-2" role="tabpanel">
      {entries.map(({ prompts, task, turnId }) => {
        const Icon = STATUS_ICON[task.status];
        const isSelected = selectedScope?.taskId === task.id;
        const summaryMessageId = `${turnId}:${task.id}:summary`;
        const isSummaryExpanded = expandedMessages.has(summaryMessageId);
        return (
          <li key={task.id} className={cn(styles.turn, "border-border rounded-lg border")}>
            <button
              type="button"
              className={cn(
                "hover:bg-accent w-full rounded-lg px-3 py-3 text-left transition-colors",
                isSelected && "bg-accent"
              )}
              aria-pressed={isSelected}
              aria-expanded={task.summary ? isSummaryExpanded : undefined}
              onClick={() => {
                onSelectScope({ turnId, taskId: task.id });
                if (task.summary) onToggleMessage(summaryMessageId);
              }}>
              <span className="flex items-start gap-2 text-sm">
                <Icon
                  className={cn(
                    "mt-0.5 size-3.5 shrink-0",
                    task.status === "in_progress" && "animate-spin"
                  )}
                />
                <span className="min-w-0">
                  <span className="block font-medium">{task.subject}</span>
                  <span className="text-muted-foreground mt-1 block truncate text-xs">
                    {messagePreview(prompts[0] ?? "")}
                  </span>
                  {task.summary ? (
                    <span className="text-muted-foreground mt-2 block text-xs whitespace-pre-wrap">
                      {isSummaryExpanded ? task.summary : messagePreview(task.summary)}
                    </span>
                  ) : null}
                </span>
              </span>
            </button>
          </li>
        );
      })}
    </ol>
  );
}
