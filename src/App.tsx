import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ChevronDown, Eye, FilePenLine, Search, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";

type CodexSessionSummary = {
  id: string;
  title: string;
  rolloutPath: string;
  cwd: string | null;
  updatedAtMs: number;
};

type CodexSessionList = {
  runtimeHome: string;
  sessions: CodexSessionSummary[];
};

type SessionWatchRegistration = {
  watchId: string;
};

type CodexSessionFileActivity = {
  readFiles: string[];
  editedFiles: string[];
  deletedFiles: string[];
};

type ActivitySection = {
  key: string;
  title: string;
  icon: typeof Eye;
  files: string[];
};

const ACTIVE_SESSION_WINDOW_MS = 60 * 1000;

function isWindowControlExcluded(target: EventTarget | null) {
  return target instanceof Element
    ? target.closest(
        [
          "[data-window-control-exclusion]",
          "a",
          "button",
          "input",
          "select",
          "textarea",
          "[role='button']",
          "[role='menu']",
          "[role='menuitem']",
        ].join(",")
      ) !== null
    : false;
}

function handleTitlebarMouseDown(event: React.MouseEvent<HTMLElement>) {
  if (event.button !== 0 || isWindowControlExcluded(event.target)) {
    return;
  }

  try {
    const appWindow = getCurrentWindow();

    if (event.detail === 2) {
      void appWindow.toggleMaximize();
      return;
    }

    if (event.detail === 1) {
      void appWindow.startDragging();
    }
  } catch {
    // Ignore titlebar behavior in plain browser mode.
  }
}

function buildActivitySections(fileActivity: CodexSessionFileActivity): ActivitySection[] {
  return [
    { key: "read", title: "Read", icon: Eye, files: fileActivity.readFiles },
    { key: "edited", title: "Edited", icon: FilePenLine, files: fileActivity.editedFiles },
    { key: "deleted", title: "Deleted", icon: Trash2, files: fileActivity.deletedFiles },
  ];
}

function SessionDropdown({
  nowMs,
  searchQuery,
  selectedSessionId,
  selectedSessionLabel,
  sessions,
  setSearchQuery,
  setSelectedSessionId,
}: {
  nowMs: number;
  searchQuery: string;
  selectedSessionId: string;
  selectedSessionLabel: string;
  sessions: CodexSessionSummary[];
  setSearchQuery: (value: string) => void;
  setSelectedSessionId: (value: string) => void;
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
        <Button variant="outline" className="w-[min(32rem,60vw)] justify-between gap-2">
          <span className="min-w-0 flex-1 truncate text-left">{selectedSessionLabel}</span>
          <ChevronDown className="size-4 opacity-60" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-80">
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
          const isSelected = session.id === selectedSessionId;

          return (
            <DropdownMenuItem
              key={session.id}
              onSelect={() => setSelectedSessionId(session.id)}
              className={
                isSelected ? "bg-accent/70 border-border/70 border" : "border border-transparent"
              }>
              <div className="flex min-w-0 flex-1 items-start gap-2">
                <span
                  className={
                    isActive
                      ? "mt-1.5 size-2 shrink-0 rounded-full bg-green-500"
                      : "bg-muted mt-1.5 size-2 shrink-0 rounded-full"
                  }
                />
                <div className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate">{session.title}</span>
                  <span className="text-muted-foreground truncate text-xs">
                    {session.cwd ?? session.rolloutPath}
                  </span>
                </div>
              </div>
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

function FileActivityPanels({
  isLoading,
  sections,
}: {
  isLoading: boolean;
  sections: ActivitySection[];
}) {
  return (
    <div className="grid gap-4 lg:grid-cols-3">
      {sections.map((section) => {
        const Icon = section.icon;

        return (
          <section key={section.key} className="border-border bg-muted/20 rounded-lg border p-4">
            <div className="mb-3 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Icon className="text-muted-foreground size-4" />
                <h2 className="text-sm font-medium">{section.title}</h2>
              </div>
              <span className="text-muted-foreground text-xs">{section.files.length}</span>
            </div>
            <div className="space-y-2">
              {section.files.slice(0, 12).map((filePath) => (
                <div key={filePath} className="bg-background rounded-md border px-3 py-2 text-sm">
                  <p className="truncate font-medium">{filePath.split("/").pop() ?? filePath}</p>
                  <p className="text-muted-foreground truncate text-xs">{filePath}</p>
                </div>
              ))}
              {section.files.length === 0 && !isLoading ? (
                <p className="text-muted-foreground text-sm">No files</p>
              ) : null}
              {isLoading ? (
                <p className="text-muted-foreground text-sm">Loading file activity...</p>
              ) : null}
            </div>
          </section>
        );
      })}
    </div>
  );
}

function App() {
  const [runtimeHome, setRuntimeHome] = useState<string>("");
  const [sessions, setSessions] = useState<CodexSessionSummary[]>([]);
  const [selectedSessionId, setSelectedSessionId] = useState<string>("");
  const [watchId, setWatchId] = useState<string>("");
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState("");
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [fileActivity, setFileActivity] = useState<CodexSessionFileActivity>({
    readFiles: [],
    editedFiles: [],
    deletedFiles: [],
  });
  const [isFileActivityLoading, setIsFileActivityLoading] = useState(false);
  const selectedSession = sessions.find((session) => session.id === selectedSessionId) ?? null;
  const selectedSessionLabel =
    selectedSession?.title ?? (isLoading ? "Loading sessions..." : "No sessions");
  const activitySections = buildActivitySections(fileActivity);

  useEffect(() => {
    let disposed = false;

    async function loadSessions() {
      const result = await invoke<CodexSessionList>("list_codex_sessions");
      if (disposed) {
        return;
      }

      setRuntimeHome(result.runtimeHome);
      setSessions(result.sessions);
      setSelectedSessionId((currentId) =>
        result.sessions.some((session) => session.id === currentId)
          ? currentId
          : (result.sessions[0]?.id ?? "")
      );
      setIsLoading(false);
    }

    void loadSessions();

    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    const intervalId = window.setInterval(() => {
      setNowMs(Date.now());
    }, 15_000);

    return () => {
      window.clearInterval(intervalId);
    };
  }, []);

  useEffect(() => {
    if (!runtimeHome) {
      return;
    }

    let activeWatchId = "";

    async function startWatch() {
      const registration = await invoke<SessionWatchRegistration>("start_codex_session_watch", {
        runtimeHome,
      });
      activeWatchId = registration.watchId;
      setWatchId(registration.watchId);
    }

    void startWatch();

    return () => {
      if (!activeWatchId) {
        return;
      }

      void invoke("stop_codex_session_watch", { watchId: activeWatchId });
    };
  }, [runtimeHome]);

  useEffect(() => {
    if (!runtimeHome) {
      return;
    }

    let disposed = false;

    async function refreshSessions() {
      const result = await invoke<CodexSessionList>("list_codex_sessions", { runtimeHome });
      if (disposed) {
        return;
      }

      setSessions(result.sessions);
      setSelectedSessionId((currentId) =>
        result.sessions.some((session) => session.id === currentId)
          ? currentId
          : (result.sessions[0]?.id ?? "")
      );
    }

    const unlistenPromise = listen("codex-session-watch-event", async (event) => {
      const payload = event.payload as { watchId?: string };

      if (payload.watchId !== watchId) {
        return;
      }

      await refreshSessions();
    });

    return () => {
      disposed = true;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [runtimeHome, watchId]);

  useEffect(() => {
    if (!selectedSession) {
      setFileActivity({ readFiles: [], editedFiles: [], deletedFiles: [] });
      return;
    }

    const currentSession = selectedSession;
    let disposed = false;

    async function loadFileActivity() {
      setIsFileActivityLoading(true);

      try {
        const result = await invoke<CodexSessionFileActivity>("get_codex_session_file_activity", {
          rolloutPath: currentSession.rolloutPath,
          cwd: currentSession.cwd,
        });

        if (!disposed) {
          setFileActivity(result);
        }
      } finally {
        if (!disposed) {
          setIsFileActivityLoading(false);
        }
      }
    }

    void loadFileActivity();

    return () => {
      disposed = true;
    };
  }, [selectedSession]);

  return (
    <div className="bg-background text-foreground relative min-h-screen">
      <header
        data-tauri-drag-region
        onMouseDown={handleTitlebarMouseDown}
        className="app-window-titlebar border-border/80 bg-background/90 fixed inset-x-0 top-0 z-20 border-b backdrop-blur">
        <div className="flex h-14 w-full items-center justify-between px-4 sm:px-6">
          <div className="min-w-0">
            <p className="text-foreground truncate text-sm font-semibold tracking-tight">
              Codex Visualization
            </p>
          </div>
          <div data-window-control-exclusion>
            <SessionDropdown
              nowMs={nowMs}
              searchQuery={searchQuery}
              selectedSessionId={selectedSessionId}
              selectedSessionLabel={selectedSessionLabel}
              sessions={sessions}
              setSearchQuery={setSearchQuery}
              setSelectedSessionId={setSelectedSessionId}
            />
          </div>
        </div>
      </header>
      <main className="px-6 pt-20 pb-6">
        <Card className="mx-auto w-full max-w-6xl shadow-sm">
          <CardHeader>
            <CardTitle>{selectedSessionLabel}</CardTitle>
            <CardDescription>현재 선택된 Codex Session의 파일 활동입니다.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="text-muted-foreground flex flex-wrap gap-3 text-sm">
              <span>runtime home: {runtimeHome || "~/.codex"}</span>
              <span>workspace: {selectedSession?.cwd ?? "Unknown"}</span>
            </div>
            <FileActivityPanels isLoading={isFileActivityLoading} sections={activitySections} />
          </CardContent>
          <CardFooter className="text-muted-foreground justify-between text-sm">
            <span>
              Read, edited, and deleted files are extracted from the selected session rollout.
            </span>
            <span>
              {selectedSession ? new Date(selectedSession.updatedAtMs).toLocaleTimeString() : ""}
            </span>
          </CardFooter>
        </Card>
      </main>
    </div>
  );
}

export default App;
