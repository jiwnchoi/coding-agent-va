import { useInfiniteQuery } from "@tanstack/react-query";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Settings } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import {
  buildShortcuts,
  buildTabNumberShortcutActions,
  handleTitlebarMouseDown,
  rotateSession,
  selectMostRecentlyUsedSession,
  SessionContextGraphTab,
  SessionFileViewer,
  SessionPickerDropdown,
  SessionTabBar,
  useAgentSessionWatchRefresh,
  useAgentSessionWatches,
  useSessionState,
} from "@/features/session-dashboard";
import { useSessionFileDiff } from "@/features/session-dashboard/hooks/useSessionFileDiff";
import {
  type AgentSessionSummary,
  type SelectedActivityFile,
} from "@/features/session-dashboard/lib/session-watch";
import { SettingsView, useAppSettings } from "@/features/settings";
import { useEditorTheme } from "@/shared/hooks/useEditorTheme";
import type { KeyboardShortcut } from "@/shared/hooks/useKeyboardShortcuts";
import { useKeyboardShortcuts } from "@/shared/hooks/useKeyboardShortcuts";
import { listAgentSessions, queryKeys } from "@/shared/lib/agent-api";
import { logger } from "@/shared/lib/logger";
import { cn } from "@/shared/lib/utils";

import styles from "./App.module.css";

const SESSION_FETCH_PAGE_SIZE = 20;

function App() {
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const { settings, settingsError, updateSettings } = useAppSettings();
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
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedActivityFile, setSelectedActivityFile] = useState<SelectedActivityFile | null>(
    null
  );
  const reconciledSessionPageCountRef = useRef(0);
  const sessionsQuery = useInfiniteQuery({
    queryKey: queryKeys.sessions(settings.runtimeHomes),
    queryFn: ({ pageParam }) =>
      listAgentSessions(settings.runtimeHomes, pageParam, SESSION_FETCH_PAGE_SIZE),
    initialPageParam: 0,
    getNextPageParam: (lastPage) => (lastPage.hasMore ? lastPage.nextOffset : undefined),
  });
  const loadedSessions = useMemo(() => {
    const sessionsById = new Map<string, AgentSessionSummary>();

    for (const session of (sessionsQuery.data?.pages ?? []).flatMap((page) => page.sessions)) {
      sessionsById.set(session.id, session);
    }

    return [...sessionsById.values()].sort(
      (left, right) =>
        right.updatedAtMs - left.updatedAtMs ||
        left.transcriptPath.localeCompare(right.transcriptPath)
    );
  }, [sessionsQuery.data?.pages]);
  const runtimeSources = sessionsQuery.data?.pages[0]?.sources ?? [];
  const isLoading = sessionsQuery.isPending;
  const sessionById = useMemo(
    () => new Map(sessions.map((session) => [session.id, session])),
    [sessions]
  );
  const openSessions = openSessionIds
    .map((sessionId) => sessionById.get(sessionId) ?? null)
    .filter((session): session is AgentSessionSummary => session !== null);
  const selectedSession = sessions.find((session) => session.id === selectedSessionId) ?? null;
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
    reconciledSessionPageCountRef.current = 0;
  }, [settings.runtimeHomes]);

  useEffect(() => {
    if (sessionsQuery.data) {
      void logger.info("Loaded agent sessions", {
        count: String(loadedSessions.length),
      });
      const pageCount = sessionsQuery.data.pages.length;
      reconcileSessions(loadedSessions, pageCount > reconciledSessionPageCountRef.current);
      reconciledSessionPageCountRef.current = pageCount;
    }
    if (sessionsQuery.error) {
      void logger.error("Failed to load agent sessions", { error: String(sessionsQuery.error) });
    }
  }, [loadedSessions, reconcileSessions, sessionsQuery.data, sessionsQuery.error]);

  const watchRegistrations = useAgentSessionWatches(runtimeSources, loadedSessions);

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

  useAgentSessionWatchRefresh(watchRegistrations, sessionsRef, selectedSessionIdRef);

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
                  hasMoreSessions={sessionsQuery.hasNextPage}
                  isFetchingMoreSessions={sessionsQuery.isFetchingNextPage}
                  searchQuery={searchQuery}
                  sessions={sessions}
                  viewedSessionUpdatedAtMs={viewedSessionUpdatedAtMs}
                  setSearchQuery={setSearchQuery}
                  onLoadMoreSessions={() => void sessionsQuery.fetchNextPage()}
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
              <div className="relative h-full min-h-0 w-full">
                {selectedSession ? (
                  <SessionContextGraphTab
                    key={selectedSession.id}
                    descriptionSettings={settings.descriptions}
                    hideCommittedFiles={settings.hideCommittedFiles}
                    showReadFiles={settings.showReadFiles}
                    isSessionListLoading={isLoading}
                    selectedActivityFile={selectedActivityFile}
                    selectedSession={selectedSession}
                    onSelectFile={setSelectedActivityFile}
                  />
                ) : null}
              </div>
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
