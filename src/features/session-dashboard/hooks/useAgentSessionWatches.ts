import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

import type {
  AgentRuntimeSource,
  AgentSessionList,
  AgentSessionSummary,
  SessionWatchEventPayload,
  SessionWatchRegistration,
} from "@/features/session-dashboard/lib/session-watch";

const WATCH_REFRESH_DEBOUNCE_MS = 750;
const EMPTY_WATCH_REGISTRATIONS: SessionWatchRegistration[] = [];

type CurrentRef<T> = {
  current: T;
};

type ReconcileSessions = (nextSessions: AgentSessionSummary[]) => void;

type SetFileActivityRefreshVersion = (update: (currentVersion: number) => number) => void;

export function useAgentSessionWatches(runtimeSources: AgentRuntimeSource[]) {
  const [watchRegistrations, setWatchRegistrations] = useState<SessionWatchRegistration[]>([]);

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
      const registrations: SessionWatchRegistration[] = [];

      for (const result of registrationResults) {
        if (result.status === "fulfilled") {
          if (disposed) {
            void invoke("stop_agent_session_watch", { watchId: result.value.watchId });
            continue;
          }

          activeWatchIds.push(result.value.watchId);
          registrations.push(result.value);
        }
      }

      if (!disposed) {
        setWatchRegistrations(registrations);
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

  return runtimeSources.some((source) => source.available)
    ? watchRegistrations
    : EMPTY_WATCH_REGISTRATIONS;
}

export function useAgentSessionWatchRefresh(
  watchRegistrations: SessionWatchRegistration[],
  reconcileSessions: ReconcileSessions,
  sessionsRef: CurrentRef<AgentSessionSummary[]>,
  selectedSessionIdRef: CurrentRef<string>,
  setFileActivityRefreshVersion: SetFileActivityRefreshVersion,
  runtimeHomes: Record<string, string>
) {
  useEffect(() => {
    if (watchRegistrations.length === 0) {
      return;
    }

    let disposed = false;
    let refreshTimeoutId: number | null = null;
    let refreshInFlight = false;
    let refreshQueued = false;
    let pendingSessionsRefresh = false;
    let pendingFileActivityRefresh = false;
    const activeWatchIds = new Set(watchRegistrations.map((registration) => registration.watchId));
    const gitIndexPaths = new Set(
      watchRegistrations
        .flatMap((registration) => registration.gitIndexPaths)
        .map(normalizeWatchPath)
    );

    function scheduleRefresh() {
      if (refreshTimeoutId !== null) {
        window.clearTimeout(refreshTimeoutId);
      }

      refreshTimeoutId = window.setTimeout(() => {
        refreshTimeoutId = null;
        void refreshSessions();
      }, WATCH_REFRESH_DEBOUNCE_MS);
    }

    async function refreshSessions() {
      if (refreshInFlight) {
        refreshQueued = true;
        return;
      }

      refreshInFlight = true;
      const shouldRefreshSessions = pendingSessionsRefresh;
      const shouldRefreshFileActivity = pendingFileActivityRefresh;
      pendingSessionsRefresh = false;
      pendingFileActivityRefresh = false;

      try {
        if (shouldRefreshSessions) {
          const result = await invoke<AgentSessionList>("list_agent_sessions", { runtimeHomes });
          if (disposed) {
            return;
          }

          reconcileSessions(result.sessions);
        }

        if (!disposed && shouldRefreshFileActivity) {
          setFileActivityRefreshVersion((currentVersion) => currentVersion + 1);
        }
      } finally {
        refreshInFlight = false;

        if (!disposed && (refreshQueued || pendingSessionsRefresh || pendingFileActivityRefresh)) {
          refreshQueued = false;
          scheduleRefresh();
        }
      }
    }

    const unlistenPromise = listen("agent-session-watch-event", (event) => {
      const payload = event.payload as SessionWatchEventPayload;

      if (
        !payload.watchId ||
        !activeWatchIds.has(payload.watchId) ||
        isControlWatchEvent(payload)
      ) {
        return;
      }

      const changedPaths = new Set(payload.changedPaths.map(normalizeWatchPath));
      if (changedPaths.size === 0) {
        return;
      }

      pendingSessionsRefresh ||= !isGitIndexOnlyChange(changedPaths, gitIndexPaths);
      pendingFileActivityRefresh ||= selectedSessionBelongsToProvider(
        sessionsRef.current,
        selectedSessionIdRef.current,
        payload.provider
      );

      scheduleRefresh();
    });

    return () => {
      disposed = true;
      if (refreshTimeoutId !== null) {
        window.clearTimeout(refreshTimeoutId);
      }
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [
    reconcileSessions,
    selectedSessionIdRef,
    sessionsRef,
    setFileActivityRefreshVersion,
    runtimeHomes,
    watchRegistrations,
  ]);
}

function normalizeWatchPath(path: string) {
  return path.replace(/\/+$/, "");
}

function isControlWatchEvent(payload: SessionWatchEventPayload) {
  return payload.eventTags.some(
    (tag) => tag === "watch_started" || tag === "watch_stopped" || tag.startsWith("watch_error:")
  );
}

function isGitIndexOnlyChange(changedPaths: Set<string>, gitIndexPaths: Set<string>) {
  if (changedPaths.size === 0) {
    return false;
  }

  for (const path of changedPaths) {
    if (!gitIndexPaths.has(path)) {
      return false;
    }
  }

  return true;
}

function selectedSessionBelongsToProvider(
  sessions: AgentSessionSummary[],
  selectedSessionId: string,
  provider: AgentRuntimeSource["provider"]
) {
  const selectedSession = sessions.find((session) => session.id === selectedSessionId);

  return selectedSession?.provider === provider;
}
