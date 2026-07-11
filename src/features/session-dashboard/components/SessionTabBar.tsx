import { X } from "lucide-react";

import { ACTIVE_SESSION_WINDOW_MS } from "@/features/session-dashboard/constants";
import type { AgentSessionSummary } from "@/features/session-dashboard/lib/session-watch";
import { cn } from "@/shared/lib/utils";

export function SessionTabBar({
  nowMs,
  openSessions,
  selectedSessionId,
  onCloseSession,
  onSelectSession,
}: {
  nowMs: number;
  openSessions: AgentSessionSummary[];
  selectedSessionId: string;
  onCloseSession: (sessionId: string) => void;
  onSelectSession: (sessionId: string) => void;
}) {
  return (
    <div className="flex min-w-0 flex-1 items-center gap-2 overflow-hidden">
      <div className="flex min-w-0 flex-1 [scrollbar-width:none] items-center gap-2 overflow-x-auto [&::-webkit-scrollbar]:hidden">
        {openSessions.map((session) => {
          const isSelected = session.id === selectedSessionId;
          const isActive = nowMs - session.updatedAtMs <= ACTIVE_SESSION_WINDOW_MS;

          return (
            <button
              key={session.id}
              type="button"
              onClick={() => onSelectSession(session.id)}
              className={cn(
                "border-border/70 bg-background/80 hover:bg-muted/80 flex min-w-0 shrink-0 items-center gap-2.5 rounded-md border px-3 py-1 text-left transition-colors",
                isSelected && "bg-accent text-accent-foreground shadow-sm"
              )}>
              <span
                className={cn(
                  "size-2 shrink-0 rounded-full bg-orange-500",
                  isActive && "bg-green-500"
                )}
              />
              <span className="max-w-52 min-w-0 truncate text-sm font-medium">{session.title}</span>
              <button
                type="button"
                data-window-control-exclusion
                aria-label={`Close ${session.title}`}
                onClick={(event) => {
                  event.stopPropagation();
                  onCloseSession(session.id);
                }}
                className="text-muted-foreground hover:bg-foreground/10 hover:text-foreground inline-flex size-4 shrink-0 items-center justify-center rounded-sm transition-colors">
                <X className="size-3" />
              </button>
            </button>
          );
        })}
      </div>
    </div>
  );
}
