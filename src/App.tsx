import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ChevronDown, Eye, FilePenLine, Search, Trash2, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

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
  DropdownMenuShortcut,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import keyboardShortcutConfig from "@/config/keyboard-shortcuts.json";
import {
  type KeyboardShortcut,
  type KeyboardShortcutConfig,
  useKeyboardShortcuts,
} from "@/lib/keyboard-shortcuts";
import { cn } from "@/lib/utils";

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
const APP_SHORTCUTS = keyboardShortcutConfig as KeyboardShortcutConfig[];

function getActiveSessionIds(sessions: CodexSessionSummary[], nowMs: number) {
  return sessions
    .filter((session) => nowMs - session.updatedAtMs <= ACTIVE_SESSION_WINDOW_MS)
    .map((session) => session.id);
}

function rotateSession(openSessionIds: string[], selectedSessionId: string, direction: 1 | -1) {
  if (openSessionIds.length <= 1) {
    return selectedSessionId;
  }

  const currentIndex = openSessionIds.indexOf(selectedSessionId);
  const safeIndex = currentIndex >= 0 ? currentIndex : 0;
  const nextIndex = (safeIndex + direction + openSessionIds.length) % openSessionIds.length;

  return openSessionIds[nextIndex] ?? selectedSessionId;
}

function closeSessionTab(openSessionIds: string[], sessionId: string, selectedSessionId: string) {
  const closedSessionIndex = openSessionIds.indexOf(sessionId);

  if (closedSessionIndex < 0) {
    return {
      nextOpenSessionIds: openSessionIds,
      nextSelectedSessionId: selectedSessionId,
    };
  }

  const nextOpenSessionIds = openSessionIds.filter((openSessionId) => openSessionId !== sessionId);

  if (sessionId !== selectedSessionId) {
    return {
      nextOpenSessionIds,
      nextSelectedSessionId: selectedSessionId,
    };
  }

  const fallbackIndex = Math.min(closedSessionIndex, nextOpenSessionIds.length - 1);

  return {
    nextOpenSessionIds,
    nextSelectedSessionId: nextOpenSessionIds[fallbackIndex] ?? "",
  };
}

function updateSessionHistory(
  historySessionIds: string[],
  previousSessionId: string,
  nextSessionId: string
) {
  if (!previousSessionId || previousSessionId === nextSessionId) {
    return historySessionIds;
  }

  return [
    previousSessionId,
    ...historySessionIds.filter(
      (sessionId) => sessionId !== previousSessionId && sessionId !== nextSessionId
    ),
  ];
}

function selectMostRecentlyActiveSession(
  historySessionIds: string[],
  openSessionIds: string[],
  selectedSessionId: string
) {
  if (openSessionIds.length <= 1) {
    return selectedSessionId;
  }

  const nextSessionId = historySessionIds.find(
    (sessionId) => sessionId !== selectedSessionId && openSessionIds.includes(sessionId)
  );

  return nextSessionId ?? selectedSessionId;
}

function reconcileTabState({
  currentDismissedSessionIds,
  currentOpenSessionIds,
  currentSelectedSessionId,
  nowMs,
  previousActiveSessionIds,
  sessions,
}: {
  currentDismissedSessionIds: string[];
  currentOpenSessionIds: string[];
  currentSelectedSessionId: string;
  nowMs: number;
  previousActiveSessionIds: string[];
  sessions: CodexSessionSummary[];
}) {
  const activeSessionIds = getActiveSessionIds(sessions, nowMs);
  const previousActiveSessionIdSet = new Set(previousActiveSessionIds);
  const availableSessionIdSet = new Set(sessions.map((session) => session.id));
  const nextDismissedSessionIds = currentDismissedSessionIds.filter((sessionId) =>
    availableSessionIdSet.has(sessionId)
  );
  const dismissedSessionIdSet = new Set(nextDismissedSessionIds);
  const newlyActiveSessionIds = activeSessionIds.filter(
    (sessionId) => !previousActiveSessionIdSet.has(sessionId)
  );
  const baseOpenSessionIds = currentOpenSessionIds.filter((sessionId) =>
    availableSessionIdSet.has(sessionId)
  );
  const nextOpenSessionIds = [...baseOpenSessionIds];

  for (const sessionId of activeSessionIds) {
    if (newlyActiveSessionIds.includes(sessionId) || !dismissedSessionIdSet.has(sessionId)) {
      if (!nextOpenSessionIds.includes(sessionId)) {
        nextOpenSessionIds.push(sessionId);
      }
    }
  }

  const isSelectedSessionAvailable = availableSessionIdSet.has(currentSelectedSessionId);
  const nextSelectedSessionId =
    currentSelectedSessionId && isSelectedSessionAvailable
      ? currentSelectedSessionId
      : (nextOpenSessionIds[0] ?? (isSelectedSessionAvailable ? currentSelectedSessionId : ""));

  return {
    activeSessionIds,
    nextDismissedSessionIds,
    nextOpenSessionIds,
    nextSelectedSessionId,
  };
}

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

function SessionPickerDropdown({
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

function SessionTabBar({
  nowMs,
  openSessions,
  selectedSessionId,
  onCloseSession,
  onSelectSession,
}: {
  nowMs: number;
  openSessions: CodexSessionSummary[];
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

function buildShortcuts(
  shortcutActions: Record<string, KeyboardShortcut["handler"]>
): KeyboardShortcut[] {
  return APP_SHORTCUTS.flatMap((shortcut) => {
    const handler = shortcutActions[shortcut.action];

    if (!handler) {
      return [];
    }

    return [
      {
        ...shortcut,
        handler,
      },
    ];
  });
}

function useSessionState() {
  const [sessions, setSessions] = useState<CodexSessionSummary[]>([]);
  const [openSessionIds, setOpenSessionIds] = useState<string[]>([]);
  const [dismissedSessionIds, setDismissedSessionIds] = useState<string[]>([]);
  const [selectedSessionId, setSelectedSessionId] = useState<string>("");
  const [sessionHistoryIds, setSessionHistoryIds] = useState<string[]>([]);
  const activeSessionIdsRef = useRef<string[]>([]);
  const dismissedSessionIdsRef = useRef(dismissedSessionIds);
  const sessionsRef = useRef(sessions);
  const openSessionIdsRef = useRef(openSessionIds);
  const selectedSessionIdRef = useRef(selectedSessionId);
  const sessionHistoryIdsRef = useRef(sessionHistoryIds);

  useEffect(() => {
    dismissedSessionIdsRef.current = dismissedSessionIds;
  }, [dismissedSessionIds]);

  useEffect(() => {
    sessionsRef.current = sessions;
  }, [sessions]);

  useEffect(() => {
    openSessionIdsRef.current = openSessionIds;
  }, [openSessionIds]);

  useEffect(() => {
    selectedSessionIdRef.current = selectedSessionId;
  }, [selectedSessionId]);

  useEffect(() => {
    sessionHistoryIdsRef.current = sessionHistoryIds;
  }, [sessionHistoryIds]);

  function setSelectedSessionWithHistory(nextSessionId: string) {
    const previousSessionId = selectedSessionIdRef.current;

    setSessionHistoryIds((currentHistorySessionIds) =>
      updateSessionHistory(currentHistorySessionIds, previousSessionId, nextSessionId)
    );
    setSelectedSessionId(nextSessionId);
  }

  function selectSession(sessionId: string) {
    setDismissedSessionIds((currentDismissedSessionIds) =>
      currentDismissedSessionIds.filter((dismissedSessionId) => dismissedSessionId !== sessionId)
    );
    setOpenSessionIds((currentOpenSessionIds) => {
      if (currentOpenSessionIds.includes(sessionId)) {
        return currentOpenSessionIds;
      }

      return [...currentOpenSessionIds, sessionId];
    });
    setSelectedSessionWithHistory(sessionId);
  }

  function handleCloseSession(sessionId: string) {
    const { nextOpenSessionIds, nextSelectedSessionId } = closeSessionTab(
      openSessionIdsRef.current,
      sessionId,
      selectedSessionIdRef.current
    );

    setOpenSessionIds(nextOpenSessionIds);
    setSelectedSessionWithHistory(nextSelectedSessionId);
    setSessionHistoryIds((currentHistorySessionIds) =>
      currentHistorySessionIds.filter((historySessionId) => historySessionId !== sessionId)
    );
    setDismissedSessionIds((currentDismissedSessionIds) =>
      currentDismissedSessionIds.includes(sessionId)
        ? currentDismissedSessionIds
        : [...currentDismissedSessionIds, sessionId]
    );
  }

  function reconcileSessions(nextSessions: CodexSessionSummary[], currentNowMs: number) {
    const { activeSessionIds, nextDismissedSessionIds, nextOpenSessionIds, nextSelectedSessionId } =
      reconcileTabState({
        currentDismissedSessionIds: dismissedSessionIdsRef.current,
        currentOpenSessionIds: openSessionIdsRef.current,
        currentSelectedSessionId: selectedSessionIdRef.current,
        nowMs: currentNowMs,
        previousActiveSessionIds: activeSessionIdsRef.current,
        sessions: nextSessions,
      });

    setSessions(nextSessions);
    setDismissedSessionIds(nextDismissedSessionIds);
    setOpenSessionIds(nextOpenSessionIds);
    setSessionHistoryIds((currentHistorySessionIds) =>
      currentHistorySessionIds.filter(
        (sessionId) =>
          nextOpenSessionIds.includes(sessionId) &&
          nextSessions.some((session) => session.id === sessionId)
      )
    );
    setSelectedSessionWithHistory(nextSelectedSessionId);
    activeSessionIdsRef.current = activeSessionIds;
  }

  return {
    sessions,
    openSessionIds,
    selectedSessionId,
    sessionsRef,
    openSessionIdsRef,
    selectedSessionIdRef,
    sessionHistoryIdsRef,
    selectSession,
    handleCloseSession,
    reconcileSessions,
  };
}

function App() {
  const [runtimeHome, setRuntimeHome] = useState<string>("");
  const {
    sessions,
    openSessionIds,
    selectedSessionId,
    sessionsRef,
    openSessionIdsRef,
    selectedSessionIdRef,
    sessionHistoryIdsRef,
    selectSession,
    handleCloseSession,
    reconcileSessions,
  } = useSessionState();
  const [watchId, setWatchId] = useState<string>("");
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState("");
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [fileActivityRefreshVersion, setFileActivityRefreshVersion] = useState(0);
  const [fileActivity, setFileActivity] = useState<CodexSessionFileActivity>({
    readFiles: [],
    editedFiles: [],
    deletedFiles: [],
  });
  const [isFileActivityLoading, setIsFileActivityLoading] = useState(false);
  const openSessions = openSessionIds
    .map((sessionId) => sessions.find((session) => session.id === sessionId) ?? null)
    .filter((session): session is CodexSessionSummary => session !== null);
  const selectedSession = sessions.find((session) => session.id === selectedSessionId) ?? null;
  const selectedSessionLabel =
    selectedSession?.title ?? (isLoading ? "Loading sessions..." : "No sessions");
  const activitySections = buildActivitySections(fileActivity);

  function handleSelectSession(sessionId: string) {
    selectSession(sessionId);
    setSearchQuery("");
  }

  const shortcutActions: Record<string, KeyboardShortcut["handler"]> = {
    selectNextSessionTab: () => {
      handleSelectSession(
        rotateSession(openSessionIdsRef.current, selectedSessionIdRef.current, 1)
      );
    },
    selectPreviousSessionTab: () => {
      handleSelectSession(
        rotateSession(openSessionIdsRef.current, selectedSessionIdRef.current, -1)
      );
    },
    selectMostRecentlyActiveSessionTab: () => {
      handleSelectSession(
        selectMostRecentlyActiveSession(
          sessionHistoryIdsRef.current,
          openSessionIdsRef.current,
          selectedSessionIdRef.current
        )
      );
    },
    closeSelectedSessionTab: () => {
      const selectedSessionId = selectedSessionIdRef.current;

      if (!selectedSessionId) {
        return;
      }

      handleCloseSession(selectedSessionId);
    },
  };

  const shortcuts = buildShortcuts(shortcutActions);

  useEffect(() => {
    let disposed = false;

    async function loadSessions() {
      const result = await invoke<CodexSessionList>("list_codex_sessions");
      if (disposed) {
        return;
      }

      setRuntimeHome(result.runtimeHome);
      reconcileSessions(result.sessions, Date.now());
      setIsLoading(false);
    }

    void loadSessions();

    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    const intervalId = window.setInterval(() => {
      const currentNowMs = Date.now();

      setNowMs(currentNowMs);
      reconcileSessions(sessionsRef.current, currentNowMs);
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

      reconcileSessions(result.sessions, Date.now());
      setFileActivityRefreshVersion((currentVersion) => currentVersion + 1);
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
  }, [fileActivityRefreshVersion, selectedSession]);

  useKeyboardShortcuts(shortcuts);

  return (
    <div className="bg-background text-foreground relative min-h-screen">
      <header
        data-tauri-drag-region
        onMouseDown={handleTitlebarMouseDown}
        className="app-window-titlebar bg-background/90 fixed inset-x-0 top-0 z-20 backdrop-blur">
        <div className="flex h-10 w-full items-center gap-2 pr-3 pl-18 sm:pr-4 sm:pl-20">
          <div className="shrink-0">
            <SessionPickerDropdown
              nowMs={nowMs}
              searchQuery={searchQuery}
              sessions={sessions}
              setSearchQuery={setSearchQuery}
              onSelectSession={handleSelectSession}
            />
          </div>
          <div className="min-w-0 flex-1">
            <SessionTabBar
              nowMs={nowMs}
              openSessions={openSessions}
              selectedSessionId={selectedSessionId}
              onCloseSession={handleCloseSession}
              onSelectSession={handleSelectSession}
            />
          </div>
        </div>
      </header>
      <main className="px-6 pt-12 pb-6">
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
