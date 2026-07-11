import { ChevronDown, Search } from "lucide-react";

import claudeLogo from "@/assets/agent-logos/claude.svg";
import codexLogo from "@/assets/agent-logos/codex.svg";
import piLogo from "@/assets/agent-logos/pi.svg";
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

const providerLogos: Record<SessionProvider, { label: string; src: string }> = {
  codex: { label: "Codex", src: codexLogo },
  claude: { label: "Claude", src: claudeLogo },
  pi: { label: "Pi", src: piLogo },
};

function ProviderLogo({ provider }: { provider: SessionProvider }) {
  const logo = providerLogos[provider];

  return (
    <span
      title={logo.label}
      className="bg-muted text-foreground flex size-7 shrink-0 items-center justify-center rounded-md">
      <img
        src={logo.src}
        alt={logo.label}
        className={provider === "claude" ? "size-4" : "size-4 dark:invert"}
      />
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
