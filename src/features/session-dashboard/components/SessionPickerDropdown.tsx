import { ChevronDown, Search } from "lucide-react";

import { ACTIVE_SESSION_WINDOW_MS } from "@/features/session-dashboard/constants";
import type { AgentSessionSummary } from "@/features/session-dashboard/lib/session-watch";
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

function ProviderLogo({ provider }: { provider: SessionProvider }) {
  const label = provider === "codex" ? "Codex" : provider === "claude" ? "Claude" : "Pi";

  return (
    <span
      aria-label={label}
      title={label}
      className="bg-muted text-foreground flex size-7 shrink-0 items-center justify-center rounded-md">
      {provider === "codex" ? (
        <svg viewBox="0 0 24 24" aria-hidden="true" className="size-4" fill="none">
          <path
            d="M12 3.3a4.35 4.35 0 0 1 7.75 2.6 4.35 4.35 0 0 1 1.55 7.95 4.35 4.35 0 0 1-6.2 5.35 4.35 4.35 0 0 1-7.75-2.6A4.35 4.35 0 0 1 5.8 8.65 4.35 4.35 0 0 1 12 3.3Z"
            stroke="currentColor"
            strokeWidth="1.8"
          />
          <path
            d="m8.1 9.7 3.9-2.25 3.9 2.25v4.6L12 16.55 8.1 14.3V9.7Z"
            stroke="currentColor"
            strokeWidth="1.5"
          />
        </svg>
      ) : provider === "claude" ? (
        <svg viewBox="0 0 24 24" aria-hidden="true" className="size-4" fill="currentColor">
          <path d="M10.9 2h2.2l.45 6.25 3.5-5.2 1.8 1.25-2.75 5.65 5.9-2.1.7 2.1-5.65 2.95 5.65 2.95-.7 2.1-5.9-2.1 2.75 5.65-1.8 1.25-3.5-5.2L13.1 22h-2.2l-.45-6.25-3.5 5.2-1.8-1.25 2.75-5.65L2 16.15l-.7-2.1 5.65-2.95L1.3 8.15 2 6.05l5.9 2.1L5.15 2.5l1.8-1.25 3.5 5.2L10.9 2Z" />
        </svg>
      ) : (
        <span aria-hidden="true" className="font-serif text-base leading-none font-semibold">
          π
        </span>
      )}
    </span>
  );
}

export function SessionPickerDropdown({
  nowMs,
  searchQuery,
  sessions,
  setSearchQuery,
  onSelectSession,
}: {
  nowMs: number;
  searchQuery: string;
  sessions: AgentSessionSummary[];
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
              <ProviderLogo provider={session.provider} />
              <span className="min-w-0 flex-1 truncate">{session.title}</span>
              <span className="text-muted-foreground flex shrink-0 items-center gap-1.5 text-xs">
                <span
                  className={
                    isActive
                      ? "size-2 rounded-full bg-green-500"
                      : "size-2 rounded-full bg-orange-500"
                  }
                />
                {isActive ? "Active" : "Idle"}
              </span>
            </DropdownMenuItem>
          );
        })}
        {sessions.length === 0 ? (
          <DropdownMenuItem disabled>No agent sessions found</DropdownMenuItem>
        ) : null}
        {sessions.length > 0 && !hasSearchResults ? (
          <DropdownMenuItem disabled>No matching sessions</DropdownMenuItem>
        ) : null}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
