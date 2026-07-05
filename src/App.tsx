import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";

import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  buildShortcuts,
  buildTabNumberShortcutActions,
  FileActivityPanels,
  handleTitlebarMouseDown,
  rotateSession,
  selectMostRecentlyActiveSession,
  SessionFileViewer,
  SessionPickerDropdown,
  SessionTabBar,
  useSessionState,
} from "@/features/session-dashboard";
import type { KeyboardShortcut } from "@/lib/keyboard-shortcuts";
import { useKeyboardShortcuts } from "@/lib/keyboard-shortcuts";
import {
  buildActivitySections,
  type CodexSessionList,
  type CodexSessionSummary,
  type SelectedActivityFile,
  type SessionWatchRegistration,
} from "@/lib/session-watch";
import { useEditorTheme } from "@/lib/useEditorTheme";
import { useSessionFileActivity } from "@/lib/useSessionFileActivity";
import { useSessionFileDiff } from "@/lib/useSessionFileDiff";

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
  const [selectedActivityFile, setSelectedActivityFile] = useState<SelectedActivityFile | null>(
    null
  );
  const editorTheme = useEditorTheme();
  const openSessions = openSessionIds
    .map((sessionId) => sessions.find((session) => session.id === sessionId) ?? null)
    .filter((session): session is CodexSessionSummary => session !== null);
  const selectedSession = sessions.find((session) => session.id === selectedSessionId) ?? null;
  const { fileActivity, isFileActivityLoading } = useSessionFileActivity(
    selectedSession,
    fileActivityRefreshVersion
  );
  const { selectedFileDiff, isFileDiffLoading, fileDiffErrorMessage, clearSelectedFileDiffState } =
    useSessionFileDiff(selectedSession, selectedActivityFile);
  const selectedSessionLabel =
    selectedSession?.title ?? (isLoading ? "Loading sessions..." : "No sessions");
  const activitySections = buildActivitySections(fileActivity);

  function handleSelectSession(sessionId: string) {
    selectSession(sessionId);
    setSearchQuery("");
    setSelectedActivityFile(null);
    clearSelectedFileDiffState();
  }

  function handleSelectActivityFile(selection: SelectedActivityFile) {
    if (
      selectedActivityFile?.activityKey === selection.activityKey &&
      selectedActivityFile.filePath === selection.filePath
    ) {
      setSelectedActivityFile(null);
      clearSelectedFileDiffState();
      return;
    }

    setSelectedActivityFile(selection);
    clearSelectedFileDiffState();
  }

  const handleCloseDiffViewer = useCallback(() => {
    setSelectedActivityFile(null);
    clearSelectedFileDiffState();
  }, [clearSelectedFileDiffState]);

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
  }, [reconcileSessions, runtimeHome, watchId]);

  useKeyboardShortcuts(shortcuts);

  const isDiffViewerOpen = selectedActivityFile !== null;

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
        <div
          data-diff-open={isDiffViewerOpen}
          className="app-main-panels mx-auto flex w-full max-w-[96rem] gap-6">
          <Card className="app-main-panel w-full shadow-sm">
            <CardHeader>
              <CardTitle>{selectedSessionLabel}</CardTitle>
              <CardDescription>현재 선택된 Codex Session의 파일 활동입니다.</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="text-muted-foreground flex flex-wrap gap-3 text-sm">
                <span>runtime home: {runtimeHome || "~/.codex"}</span>
                <span>workspace: {selectedSession?.cwd ?? "Unknown"}</span>
              </div>
              <FileActivityPanels
                isLoading={isFileActivityLoading}
                sections={activitySections}
                selectedFilePath={selectedActivityFile?.filePath ?? ""}
                onSelectFile={handleSelectActivityFile}
              />
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
          <SessionFileViewer
            errorMessage={fileDiffErrorMessage}
            isLoading={isFileDiffLoading}
            onClose={handleCloseDiffViewer}
            selectedActivityFile={selectedActivityFile}
            selectedFileDiff={selectedFileDiff}
            theme={editorTheme}
          />
        </div>
      </main>
    </div>
  );
}

export default App;
