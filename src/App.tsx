import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Settings } from "lucide-react";
import { useEffect, useState } from "react";

import {
  buildShortcuts,
  buildTabNumberShortcutActions,
  handleTitlebarMouseDown,
  rotateSession,
  selectMostRecentlyUsedSession,
  SessionContextGraphView,
  SessionFileViewer,
  SessionPickerDropdown,
  SessionTabBar,
  useAgentSessionWatchRefresh,
  useAgentSessionWatches,
  useSessionState,
} from "@/features/session-dashboard";
import { useSessionFileActivity } from "@/features/session-dashboard/hooks/useSessionFileActivity";
import { useSessionFileDiff } from "@/features/session-dashboard/hooks/useSessionFileDiff";
import {
  type AgentRuntimeSource,
  type AgentSessionList,
  type AgentSessionSummary,
  type SelectedActivityFile,
} from "@/features/session-dashboard/lib/session-watch";
import { SettingsView, useAppSettings } from "@/features/settings";
import { useEditorTheme } from "@/shared/hooks/useEditorTheme";
import type { KeyboardShortcut } from "@/shared/hooks/useKeyboardShortcuts";
import { useKeyboardShortcuts } from "@/shared/hooks/useKeyboardShortcuts";
import { cn } from "@/shared/lib/utils";

import styles from "./App.module.css";

function App() {
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const { settings, settingsError, updateSettings } = useAppSettings();
  const [runtimeSources, setRuntimeSources] = useState<AgentRuntimeSource[]>([]);
  const {
    sessions,
    viewedSessionUpdatedAtMs,
    openSessionIds,
    selectedSessionId,
    sessionsRef,
    openSessionIdsRef,
    selectedSessionIdRef,
    sessionHistoryIdsRef,
    selectSession,
    handleCloseSession,
    markSessionAsViewed,
    markSelectedSessionAsViewed,
    reconcileSessions,
  } = useSessionState();
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState("");
  const [fileActivityRefreshVersion, setFileActivityRefreshVersion] = useState(0);
  const [selectedActivityFile, setSelectedActivityFile] = useState<SelectedActivityFile | null>(
    null
  );
  const watchRegistrations = useAgentSessionWatches(runtimeSources);
  const openSessions = openSessionIds
    .map((sessionId) => sessions.find((session) => session.id === sessionId) ?? null)
    .filter((session): session is AgentSessionSummary => session !== null);
  const selectedSession = sessions.find((session) => session.id === selectedSessionId) ?? null;
  const { fileActivity, isFileActivityLoading } = useSessionFileActivity(
    selectedSession,
    fileActivityRefreshVersion,
    settings.hideCommittedFiles
  );
  const { clearSelectedFileDiffState, fileDiffErrorMessage, isFileDiffLoading, selectedFileDiff } =
    useSessionFileDiff(selectedSession, selectedActivityFile);
  const editorTheme = useEditorTheme(settings.monacoTheme);
  const selectedSessionLabel =
    selectedSession?.title ?? (isLoading ? "Loading sessions..." : "No sessions");

  function handleSelectSession(sessionId: string) {
    selectSession(sessionId);
    if (document.hasFocus()) {
      markSessionAsViewed(sessionId);
    }
    setSearchQuery("");
    setSelectedActivityFile(null);
    clearSelectedFileDiffState();
  }

  function handleCloseFileViewer() {
    setSelectedActivityFile(null);
    clearSelectedFileDiffState();
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
    selectMostRecentlyUsedSessionTab: () => {
      handleSelectSession(
        selectMostRecentlyUsedSession(
          sessionHistoryIdsRef.current,
          openSessionIdsRef.current,
          selectedSessionIdRef.current
        )
      );
    },
    closeSelectedSessionTab: () => {
      const selectedSessionId = selectedSessionIdRef.current;

      if (selectedSessionId) {
        handleCloseSession(selectedSessionId);
      }
    },
    toggleSettings: () => setIsSettingsOpen((isOpen) => !isOpen),
  };

  const shortcuts = buildShortcuts(shortcutActions, settings.keyboardShortcuts);

  useEffect(() => {
    let disposed = false;

    async function loadSessions() {
      const result = await invoke<AgentSessionList>("list_agent_sessions", {
        runtimeHomes: settings.runtimeHomes,
      });
      if (disposed) {
        return;
      }

      setRuntimeSources(result.sources);
      reconcileSessions(result.sessions);
      setIsLoading(false);
    }

    void loadSessions();

    return () => {
      disposed = true;
    };
  }, [reconcileSessions, settings.runtimeHomes]);

  useEffect(() => {
    let disposed = false;
    const handleWindowFocus = () => {
      if (!disposed) {
        markSelectedSessionAsViewed();
      }
    };

    window.addEventListener("focus", handleWindowFocus);
    const currentWindow = getCurrentWindow();
    let unlistenFocusChanged: (() => void) | undefined;
    const unlistenPromise = currentWindow.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        handleWindowFocus();
      }
    });

    void unlistenPromise.then((unlisten) => {
      if (disposed) {
        unlisten();
        return;
      }

      unlistenFocusChanged = unlisten;
    });
    void currentWindow.isFocused().then((focused) => {
      if (focused) {
        handleWindowFocus();
      }
    });

    return () => {
      disposed = true;
      window.removeEventListener("focus", handleWindowFocus);
      unlistenFocusChanged?.();
    };
  }, [markSelectedSessionAsViewed]);

  useAgentSessionWatchRefresh(
    watchRegistrations,
    reconcileSessions,
    sessionsRef,
    selectedSessionIdRef,
    setFileActivityRefreshVersion,
    settings.runtimeHomes
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
          {isSettingsOpen ? (
            <p className="min-w-0 flex-1 truncate text-sm font-medium">Settings</p>
          ) : (
            <>
              <div className="shrink-0">
                <SessionPickerDropdown
                  searchQuery={searchQuery}
                  sessions={sessions}
                  viewedSessionUpdatedAtMs={viewedSessionUpdatedAtMs}
                  setSearchQuery={setSearchQuery}
                  onSelectSession={handleSelectSession}
                />
              </div>
              <div className="min-w-0 flex-1">
                <SessionTabBar
                  openSessions={openSessions}
                  selectedSessionId={selectedSessionId}
                  viewedSessionUpdatedAtMs={viewedSessionUpdatedAtMs}
                  onCloseSession={handleCloseSession}
                  onSelectSession={handleSelectSession}
                />
              </div>
            </>
          )}
          <button
            type="button"
            data-window-control-exclusion
            aria-label={isSettingsOpen ? "Close settings" : "Open settings"}
            title="Settings (⌘,)"
            onClick={() => setIsSettingsOpen((isOpen) => !isOpen)}
            className="text-muted-foreground hover:bg-accent hover:text-foreground inline-flex size-7 shrink-0 items-center justify-center rounded-md transition-colors">
            <Settings className="size-4" />
          </button>
        </div>
      </header>
      <main className={cn(styles.main, "relative flex h-full min-h-0")}>
        {isSettingsOpen ? (
          <SettingsView
            runtimeSources={runtimeSources}
            settings={settings}
            settingsError={settingsError}
            onClose={() => setIsSettingsOpen(false)}
            onSettingsChange={updateSettings}
          />
        ) : (
          <>
            <div
              className={cn(
                styles.contextGraphTitle,
                "border-border text-card-foreground absolute left-4 z-[6] truncate rounded-lg border px-3 py-2.5 text-sm leading-5 font-medium"
              )}>
              {selectedSessionLabel}
            </div>
            <div className="min-w-0 flex-1">
              <SessionContextGraphView
                fileActivity={fileActivity}
                isFileActivityLoading={isFileActivityLoading || isLoading}
                selectedActivityFile={selectedActivityFile}
                selectedSession={selectedSession}
                onSelectFile={setSelectedActivityFile}
              />
            </div>
            <SessionFileViewer
              errorMessage={fileDiffErrorMessage}
              isLoading={Boolean(isFileDiffLoading)}
              onClose={handleCloseFileViewer}
              selectedActivityFile={selectedActivityFile}
              selectedFileDiff={selectedFileDiff}
              theme={editorTheme}
            />
          </>
        )}
      </main>
    </div>
  );
}

export default App;
