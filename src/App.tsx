import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import {
  buildShortcuts,
  buildTabNumberShortcutActions,
  handleTitlebarMouseDown,
  rotateSession,
  selectMostRecentlyActiveSession,
  SessionContextGraphView,
  SessionPickerDropdown,
  SessionTabBar,
  useAgentSessionWatchRefresh,
  useAgentSessionWatches,
  useSessionState,
} from "@/features/session-dashboard";
import { useSessionFileActivity } from "@/features/session-dashboard/hooks/useSessionFileActivity";
import {
  type AgentRuntimeSource,
  type AgentSessionList,
  type AgentSessionSummary,
} from "@/features/session-dashboard/lib/session-watch";
import type { KeyboardShortcut } from "@/shared/hooks/useKeyboardShortcuts";
import { useKeyboardShortcuts } from "@/shared/hooks/useKeyboardShortcuts";
import { cn } from "@/shared/lib/utils";

import styles from "./App.module.css";

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
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState("");
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [fileActivityRefreshVersion, setFileActivityRefreshVersion] = useState(0);
  const watchRegistrations = useAgentSessionWatches(runtimeSources);
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

  useAgentSessionWatchRefresh(
    watchRegistrations,
    reconcileSessions,
    sessionsRef,
    selectedSessionIdRef,
    setFileActivityRefreshVersion
  );

  useKeyboardShortcuts(shortcuts);

  return (
    <div
      className={cn(
        styles.appShell,
        "bg-background text-foreground relative h-screen overflow-hidden"
      )}>
      <header
        data-tauri-drag-region
        onMouseDown={handleTitlebarMouseDown}
        className={cn(
          styles.windowTitlebar,
          "bg-background/90 fixed inset-x-0 top-0 z-20 backdrop-blur"
        )}>
        <div className="flex h-full w-full items-center gap-2 pr-3 pl-18 sm:pr-4 sm:pl-20">
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
      <main className={cn(styles.main, "relative h-full min-h-0")}>
        <div
          className={cn(
            styles.contextGraphTitle,
            "border-border text-card-foreground absolute left-4 z-[6] truncate rounded-lg border px-3 py-2.5 text-sm leading-5 font-medium"
          )}>
          {selectedSessionLabel}
        </div>
        <SessionContextGraphView
          fileActivity={fileActivity}
          isFileActivityLoading={isFileActivityLoading || isLoading}
          selectedActivityFile={null}
          selectedSession={selectedSession}
          onSelectFile={() => {}}
        />
      </main>
    </div>
  );
}

export default App;
