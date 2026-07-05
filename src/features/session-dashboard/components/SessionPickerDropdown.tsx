import { ChevronDown, Search } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { ACTIVE_SESSION_WINDOW_MS } from "@/features/session-dashboard/constants";
import type { CodexSessionSummary } from "@/lib/session-watch";

export function SessionPickerDropdown({
  nowMs,
  searchQuery,
  sessions,
  setSearchQuery,
  onSelectSession,
}: {
  nowMs: number;
  searchQuery: string;
  sessions: CodexSessionSummary[];
  setSearchQuery: (value: string) => void;
  onSelectSession: (sessionId: string) => void;
}) {
  const normalizedQuery = searchQuery.trim().toLowerCase();
  const filteredSessions = sessions.filter((session) => {
    if (!normalizedQuery) {
      return true;
    }

    return [session.title, session.cwd ?? ""].some((value) =>
      value.toLowerCase().includes(normalizedQuery)
    );
  });
  const visibleSessions = filteredSessions.slice(0, 5);
  const hasSearchResults = visibleSessions.length > 0;

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
        {visibleSessions.map((session) => {
          const isActive = nowMs - session.updatedAtMs <= ACTIVE_SESSION_WINDOW_MS;

          return (
            <DropdownMenuItem
              key={session.id}
              onSelect={() => onSelectSession(session.id)}
              className="border border-transparent">
              <div className="flex min-w-0 flex-1 items-start gap-2">
                <span
                  className={
                    isActive
                      ? "mt-1.5 size-2 shrink-0 rounded-full bg-green-500"
                      : "mt-1.5 size-2 shrink-0 rounded-full bg-orange-500"
                  }
                />
                <div className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate">{session.title}</span>
                  <span className="text-muted-foreground truncate text-xs">
                    {session.cwd ?? session.rolloutPath}
                  </span>
                </div>
              </div>
              <DropdownMenuShortcut>{session.id.slice(0, 4)}</DropdownMenuShortcut>
            </DropdownMenuItem>
          );
        })}
        {sessions.length === 0 ? (
          <DropdownMenuItem disabled>No Codex sessions found</DropdownMenuItem>
        ) : null}
        {sessions.length > 0 && !hasSearchResults ? (
          <DropdownMenuItem disabled>No matching sessions</DropdownMenuItem>
        ) : null}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
