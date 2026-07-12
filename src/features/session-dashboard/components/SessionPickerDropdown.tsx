import { Claude, OpenAI } from "@lobehub/icons";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ChevronDown, Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import piLogo from "@/assets/agent-logos/pi.svg";
import type { AgentSessionSummary } from "@/features/session-dashboard/lib/session-watch";
import { isSessionChecked } from "@/features/session-dashboard/session-tabs/session-tab-utils";
import { Button } from "@/shared/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/shared/components/ui/dropdown-menu";
import { Input } from "@/shared/components/ui/input";

type SessionProvider = AgentSessionSummary["provider"];

const SESSION_PAGE_SIZE = 30;
const SESSION_LOAD_AHEAD_COUNT = 5;

const providerLabels: Record<SessionProvider, string> = {
  codex: "OpenAI",
  claude: "Claude",
  pi: "Pi",
};

function ProviderLogo({ provider }: { provider: SessionProvider }) {
  const label = providerLabels[provider];

  return (
    <span
      title={label}
      className="bg-muted text-foreground flex size-7 shrink-0 items-center justify-center rounded-md">
      {provider === "codex" ? (
        <OpenAI aria-label={label} className="size-4" />
      ) : provider === "claude" ? (
        <Claude.Color aria-label={label} className="size-4" />
      ) : (
        <img src={piLogo} alt={label} className="size-4 dark:invert" />
      )}
    </span>
  );
}

export function SessionPickerDropdown({
  searchQuery,
  sessions,
  viewedSessionUpdatedAtMs,
  setSearchQuery,
  onSelectSession,
}: {
  searchQuery: string;
  sessions: AgentSessionSummary[];
  viewedSessionUpdatedAtMs: Record<string, number>;
  setSearchQuery: (value: string) => void;
  onSelectSession: (sessionId: string) => void;
}) {
  const normalizedQuery = searchQuery.trim().toLowerCase();
  return (
    <DropdownMenu modal={false}>
      <DropdownMenuTrigger asChild>
        <Button variant="outline" size="icon" className="rounded-md">
          <ChevronDown className="size-4" />
          <span className="sr-only">Search sessions</span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-80">
        <div className="px-1 pb-1" onKeyDown={(event) => event.stopPropagation()}>
          <div className="relative">
            <Search className="text-muted-foreground absolute top-1/2 left-2.5 size-4 -translate-y-1/2" />
            <Input
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder="Search sessions"
              className="pl-8"
            />
          </div>
        </div>
        <DropdownMenuSeparator />
        <SessionPickerVirtualList
          key={normalizedQuery}
          normalizedQuery={normalizedQuery}
          sessions={sessions}
          viewedSessionUpdatedAtMs={viewedSessionUpdatedAtMs}
          onSelectSession={onSelectSession}
        />
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function SessionPickerVirtualList({
  normalizedQuery,
  sessions,
  viewedSessionUpdatedAtMs,
  onSelectSession,
}: {
  normalizedQuery: string;
  sessions: AgentSessionSummary[];
  viewedSessionUpdatedAtMs: Record<string, number>;
  onSelectSession: (sessionId: string) => void;
}) {
  const [loadedSessionCount, setLoadedSessionCount] = useState(SESSION_PAGE_SIZE);
  const scrollElementRef = useRef<HTMLDivElement>(null);
  const filteredSessions = useMemo(
    () =>
      sessions.filter((session) => {
        if (!normalizedQuery) {
          return true;
        }

        return [session.title, session.cwd ?? ""].some((value) =>
          value.toLowerCase().includes(normalizedQuery)
        );
      }),
    [normalizedQuery, sessions]
  );
  const visibleSessions = filteredSessions.slice(0, loadedSessionCount);
  const hasSearchResults = filteredSessions.length > 0;
  const hasMoreSessions = visibleSessions.length < filteredSessions.length;
  const rowVirtualizer = useVirtualizer({
    count: visibleSessions.length,
    getScrollElement: () => scrollElementRef.current,
    estimateSize: () => 36,
    overscan: 5,
  });
  const virtualRows = rowVirtualizer.getVirtualItems();

  useEffect(() => {
    const lastVirtualRow = virtualRows[virtualRows.length - 1];

    if (
      hasMoreSessions &&
      lastVirtualRow !== undefined &&
      lastVirtualRow.index >= visibleSessions.length - SESSION_LOAD_AHEAD_COUNT
    ) {
      setLoadedSessionCount((currentCount) =>
        Math.min(currentCount + SESSION_PAGE_SIZE, filteredSessions.length)
      );
    }
  }, [filteredSessions.length, hasMoreSessions, visibleSessions.length, virtualRows]);

  if (!hasSearchResults) {
    return sessions.length === 0 ? (
      <DropdownMenuItem disabled>No agent sessions found</DropdownMenuItem>
    ) : (
      <DropdownMenuItem disabled>No matching sessions</DropdownMenuItem>
    );
  }

  return (
    <div ref={scrollElementRef} className="max-h-80 overflow-y-auto">
      <div className="relative w-full" style={{ height: rowVirtualizer.getTotalSize() }}>
        {virtualRows.map((virtualRow) => {
          const session = visibleSessions[virtualRow.index];
          const isChecked = isSessionChecked(session, viewedSessionUpdatedAtMs);

          return (
            <DropdownMenuItem
              key={session.id}
              data-index={virtualRow.index}
              ref={rowVirtualizer.measureElement}
              onSelect={() => onSelectSession(session.id)}
              className="border border-transparent"
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                transform: `translateY(${virtualRow.start}px)`,
              }}>
              <ProviderLogo provider={session.provider} />
              <span className="min-w-0 flex-1 truncate">{session.title}</span>
              <span className="text-muted-foreground flex shrink-0 items-center gap-1.5 text-xs">
                <span
                  className={
                    isChecked
                      ? "size-2 rounded-full bg-orange-500"
                      : "size-2 rounded-full bg-green-500"
                  }
                />
              </span>
            </DropdownMenuItem>
          );
        })}
      </div>
    </div>
  );
}
