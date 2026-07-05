import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  buildShortcuts,
  buildTabNumberShortcutActions,
  handleTitlebarMouseDown,
  rotateSession,
  selectMostRecentlyActiveSession,
  SessionContextGraphView,
  SessionPickerDropdown,
  SessionTabBar,
  useSessionState,
} from "@/features/session-dashboard";
import type { KeyboardShortcut } from "@/lib/keyboard-shortcuts";
import { useKeyboardShortcuts } from "@/lib/keyboard-shortcuts";
import {
  type AgentRuntimeSource,
  type AgentSessionList,
  type AgentSessionSummary,
  type SessionWatchRegistration,
} from "@/lib/session-watch";
import { useSessionFileActivity } from "@/lib/useSessionFileActivity";

function App() {
  const [runtimeSources, setRuntimeSources] = useState<AgentRuntimeSource[]>([]);
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
  const openSessions = openSessionIds
    .map((sessionId) => sessions.find((session) => session.id === sessionId) ?? null)
    .filter((session): session is AgentSessionSummary => session !== null);
  const selectedSession = sessions.find((session) => session.id === selectedSessionId) ?? null;
  const { fileActivity, isFileActivityLoading } = useSessionFileActivity(
    selectedSession,
    fileActivityRefreshVersion
  );
  const selectedSessionLabel =
    selectedSession?.title ?? (isLoading ? "Loading sessions..." : "No sessions");

  function handleSelectSession(sessionId: string) {
    selectSession(sessionId);
    setSearchQuery("");
  }

  const shortcutActions: Record<string, KeyboardShortcut["handler"]> = {
    ...buildTabNumberShortcutActions(handleSelectSession, openSessionIdsRef),
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
      const result = await invoke<AgentSessionList>("list_agent_sessions");
      if (disposed) {
        return;
      }

      setRuntimeSources(result.sources);
      reconcileSessions(result.sessions, Date.now());
      setIsLoading(false);
    }

    void loadSessions();

    return () => {
      disposed = true;
    };
  }, [reconcileSessions]);

  useEffect(() => {
    const intervalId = window.setInterval(() => {
      const currentNowMs = Date.now();

      setNowMs(currentNowMs);
      reconcileSessions(sessionsRef.current, currentNowMs);
    }, 15_000);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [reconcileSessions, sessionsRef]);

  useEffect(() => {
    const availableSources = runtimeSources.filter((source) => source.available);

    if (availableSources.length === 0) {
      return;
    }

    let disposed = false;
    const activeWatchIds: string[] = [];

    async function startWatches() {
      const registrationResults = await Promise.allSettled(
        availableSources.map((source) =>
          invoke<SessionWatchRegistration>("start_agent_session_watch", {
            provider: source.provider,
            runtimeHome: source.runtimeHome,
          })
        )
      );

      for (const result of registrationResults) {
        if (result.status === "fulfilled") {
          activeWatchIds.push(result.value.watchId);
        }
      }

      if (!disposed) {
        setWatchId(activeWatchIds.join(","));
      }
    }

    void startWatches();

    return () => {
      disposed = true;

      for (const activeWatchId of activeWatchIds) {
        void invoke("stop_agent_session_watch", { watchId: activeWatchId });
      }
    };
  }, [runtimeSources]);

  useEffect(() => {
    if (!watchId) {
      return;
    }

    let disposed = false;
    const activeWatchIds = new Set(watchId.split(",").filter(Boolean));

    async function refreshSessions() {
      const result = await invoke<AgentSessionList>("list_agent_sessions");
      if (disposed) {
        return;
      }

      reconcileSessions(result.sessions, Date.now());
      setFileActivityRefreshVersion((currentVersion) => currentVersion + 1);
    }

    const unlistenPromise = listen("agent-session-watch-event", async (event) => {
      const payload = event.payload as { watchId?: string };

      if (!payload.watchId || !activeWatchIds.has(payload.watchId)) {
        return;
      }

      await refreshSessions();
    });

    return () => {
      disposed = true;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [reconcileSessions, watchId]);

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
        <div className="app-main-panels mx-auto flex w-full max-w-[72rem] gap-6">
          <Card className="app-main-panel w-full shadow-sm">
            <CardHeader>
              <CardTitle>{selectedSessionLabel}</CardTitle>
            </CardHeader>
            <CardContent>
              <SessionContextGraphView
                fileActivity={fileActivity}
                isFileActivityLoading={isFileActivityLoading}
                selectedActivityFile={null}
                selectedSession={selectedSession}
                onSelectFile={() => {}}
              />
            </CardContent>
          </Card>
        </div>
      </main>
    </div>
  );
}

export default App;
