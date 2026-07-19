import { useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";

import type {
  AgentRuntimeSource,
  AgentSessionSummary,
  SessionWatchEventPayload,
  SessionWatchRegistration,
} from "@/features/session-dashboard/lib/session-watch";
import { queryKeys } from "@/shared/lib/agent-api";
import { logger } from "@/shared/lib/logger";

const WATCH_REFRESH_INTERVAL_MS = 500;
const WATCH_RECOVERY_INTERVAL_MS = 30_000;
const EMPTY_WATCH_REGISTRATIONS: SessionWatchRegistration[] = [];

type CurrentRef<T> = {
  current: T;
};

export function useAgentSessionWatches(runtimeSources: AgentRuntimeSource[]) {
  const [watchRegistrations, setWatchRegistrations] = useState<SessionWatchRegistration[]>([]);
  const [watchRestartVersion, setWatchRestartVersion] = useState(0);
  const activeWatchIdsRef = useRef<string[]>([]);
  const runtimeSourcesRef = useRef(runtimeSources);
  const watchTransitionRef = useRef(Promise.resolve());
  runtimeSourcesRef.current = runtimeSources;
  const runtimeSourceKey = runtimeSources
    .map(
      (source) =>
        `${source.provider}:${source.runtimeHome}:${source.available ? "available" : "unavailable"}`
    )
    .sort()
    .join("\0");
  useEffect(() => {
    let disposed = false;
    const unlistenPromise = listen("agent-session-watch-event", (event) => {
      const payload = event.payload as SessionWatchEventPayload;
      if (
        !disposed &&
        activeWatchIdsRef.current.includes(payload.watchId) &&
        isWatchError(payload)
      ) {
        setWatchRestartVersion((currentVersion) => currentVersion + 1);
      }
    });

    return () => {
      disposed = true;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    const availableSources = runtimeSourcesRef.current.filter((source) => source.available);
    let disposed = false;

    async function replaceWatches() {
      const previousWatchIds = activeWatchIdsRef.current.splice(0);
      await Promise.allSettled(
        previousWatchIds.map((watchId) => invoke("stop_agent_session_watch", { watchId }))
      );
      if (disposed) {
        return;
      }

      const registrationResults = await Promise.allSettled(
        availableSources.map((source) =>
          invoke<SessionWatchRegistration>("start_agent_session_watch", {
            provider: source.provider,
            runtimeHome: source.runtimeHome,
          })
        )
      );
      if (disposed) {
        await Promise.allSettled(
          registrationResults.flatMap((result) =>
            result.status === "fulfilled"
              ? [invoke("stop_agent_session_watch", { watchId: result.value.watchId })]
              : []
          )
        );
        return;
      }

      const registrations: SessionWatchRegistration[] = [];
      const nextWatchIds: string[] = [];

      for (const result of registrationResults) {
        if (result.status === "fulfilled") {
          nextWatchIds.push(result.value.watchId);
          registrations.push(result.value);
        } else {
          void logger.error("Failed to start agent session watcher", {
            error: String(result.reason),
          });
        }
      }

      if (!disposed) {
        activeWatchIdsRef.current = nextWatchIds;
        setWatchRegistrations(registrations);
      }
    }

    watchTransitionRef.current = watchTransitionRef.current.then(replaceWatches, replaceWatches);

    return () => {
      disposed = true;
      const stopWatches = async () => {
        const watchIds = activeWatchIdsRef.current.splice(0);
        await Promise.allSettled(
          watchIds.map((watchId) => invoke("stop_agent_session_watch", { watchId }))
        );
      };
      watchTransitionRef.current = watchTransitionRef.current.then(stopWatches, stopWatches);
    };
  }, [runtimeSourceKey, watchRestartVersion]);

  return runtimeSources.some((source) => source.available)
    ? watchRegistrations
    : EMPTY_WATCH_REGISTRATIONS;
}

export function useAgentSessionWatchRefresh(
  watchRegistrations: SessionWatchRegistration[],
  sessionsRef: CurrentRef<AgentSessionSummary[]>,
  selectedSessionIdRef: CurrentRef<string>
) {
  const queryClient = useQueryClient();
  useEffect(() => {
    if (watchRegistrations.length === 0) {
      return;
    }

    let disposed = false;
    let refreshTimeoutId: number | null = null;
    let settleRefreshTimeoutId: number | null = null;
    let recoveryIntervalId: number | null = null;
    let refreshInFlight = false;
    let refreshQueued = false;
    let pendingSessionsRefresh = false;
    let pendingSessionDetailsRefresh = false;
    let pendingWorkspaceGraphRefresh = false;
    let pendingSettleRefresh = false;
    const activeWatchIds = new Set(watchRegistrations.map((registration) => registration.watchId));

    function scheduleRefresh() {
      // Keep a trailing refresh at a fixed cadence while events continue to arrive.
      if (refreshTimeoutId !== null) {
        return;
      }

      refreshTimeoutId = window.setTimeout(() => {
        refreshTimeoutId = null;
        void refreshSessions();
      }, WATCH_REFRESH_INTERVAL_MS);
    }

    async function refreshSessions() {
      if (refreshInFlight) {
        refreshQueued = true;
        return;
      }

      refreshInFlight = true;
      const shouldRefreshSessions = pendingSessionsRefresh;
      const shouldRefreshSessionDetails = pendingSessionDetailsRefresh;
      const shouldRefreshWorkspaceGraph = pendingWorkspaceGraphRefresh;
      const shouldSettleRefresh = pendingSettleRefresh;
      pendingSessionsRefresh = false;
      pendingSessionDetailsRefresh = false;
      pendingWorkspaceGraphRefresh = false;
      pendingSettleRefresh = false;

      try {
        if (shouldRefreshSessions) {
          await queryClient.invalidateQueries({ queryKey: ["agent-sessions"] });
        }

        if (!disposed && shouldRefreshSessionDetails) {
          await refreshSelectedSessionDetails(shouldRefreshWorkspaceGraph);

          if (shouldSettleRefresh) {
            if (settleRefreshTimeoutId !== null) {
              window.clearTimeout(settleRefreshTimeoutId);
            }
            settleRefreshTimeoutId = window.setTimeout(() => {
              settleRefreshTimeoutId = null;
              if (!disposed) {
                void refreshSelectedSessionDetails(shouldRefreshWorkspaceGraph);
              }
            }, WATCH_REFRESH_INTERVAL_MS);
          }
        }
      } finally {
        refreshInFlight = false;

        if (
          !disposed &&
          (refreshQueued ||
            pendingSessionsRefresh ||
            pendingSessionDetailsRefresh ||
            pendingWorkspaceGraphRefresh ||
            pendingSettleRefresh)
        ) {
          refreshQueued = false;
          scheduleRefresh();
        }
      }
    }

    async function refreshSelectedSessionDetails(refreshWorkspaceGraph: boolean) {
      const selectedSessionId = selectedSessionIdRef.current;
      if (!selectedSessionId) {
        return;
      }

      const selectedSession = sessionsRef.current.find(
        (session) => session.id === selectedSessionId
      );
      await Promise.all([
        queryClient.refetchQueries({
          queryKey: queryKeys.sessionDetails(selectedSessionId),
          type: "active",
        }),
        queryClient.refetchQueries({
          queryKey: queryKeys.sessionFileDiffs(selectedSessionId),
          type: "active",
        }),
        ...(refreshWorkspaceGraph && selectedSession?.cwd
          ? [
              queryClient.refetchQueries({
                queryKey: queryKeys.workspaceGraph(selectedSession.cwd),
                type: "active",
              }),
            ]
          : []),
      ]);
    }

    const unlistenPromise = listen("agent-session-watch-event", (event) => {
      const payload = event.payload as SessionWatchEventPayload;

      if (!payload.watchId || !activeWatchIds.has(payload.watchId)) {
        return;
      }

      if (isWatchError(payload)) {
        void logger.error("Agent session watcher reported an error", {
          error: payload.eventTags.join(", "),
          provider: payload.provider,
        });
        pendingSessionsRefresh = true;
        pendingSessionDetailsRefresh = true;
        pendingWorkspaceGraphRefresh = true;
        scheduleRefresh();
        return;
      }

      if (isControlWatchEvent(payload)) {
        return;
      }

      const changedPaths = new Set(payload.changedPaths.map(normalizeWatchPath));
      if (changedPaths.size === 0) {
        return;
      }

      pendingSessionsRefresh = true;
      const selectedSessionChanged = sessionsRef.current.some(
        (session) =>
          session.id === selectedSessionIdRef.current && session.provider === payload.provider
      );
      pendingSessionDetailsRefresh ||= selectedSessionChanged;
      pendingWorkspaceGraphRefresh ||= selectedSessionChanged;
      pendingSettleRefresh ||= selectedSessionChanged;

      scheduleRefresh();
    });

    const recoverFromMissedEvent = () => {
      if (document.visibilityState !== "visible") {
        return;
      }
      pendingSessionsRefresh = true;
      pendingSessionDetailsRefresh = true;
      pendingWorkspaceGraphRefresh = true;
      scheduleRefresh();
    };

    const recoverSessionData = () => {
      if (document.visibilityState !== "visible") {
        return;
      }
      pendingSessionsRefresh = true;
      pendingSessionDetailsRefresh = true;
      scheduleRefresh();
    };

    window.addEventListener("focus", recoverFromMissedEvent);
    document.addEventListener("visibilitychange", recoverFromMissedEvent);
    recoveryIntervalId = window.setInterval(recoverSessionData, WATCH_RECOVERY_INTERVAL_MS);

    // Re-read once after native watch registration so writes made during startup
    // cannot remain stale until the next filesystem event.
    pendingSessionsRefresh = true;
    pendingSessionDetailsRefresh = true;
    scheduleRefresh();

    return () => {
      disposed = true;
      if (refreshTimeoutId !== null) {
        window.clearTimeout(refreshTimeoutId);
      }
      if (settleRefreshTimeoutId !== null) {
        window.clearTimeout(settleRefreshTimeoutId);
      }
      if (recoveryIntervalId !== null) {
        window.clearInterval(recoveryIntervalId);
      }
      window.removeEventListener("focus", recoverFromMissedEvent);
      document.removeEventListener("visibilitychange", recoverFromMissedEvent);
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [queryClient, selectedSessionIdRef, sessionsRef, watchRegistrations]);
}

function normalizeWatchPath(path: string) {
  return path.replace(/\/+$/, "");
}

function isControlWatchEvent(payload: SessionWatchEventPayload) {
  return payload.eventTags.some(
    (tag) => tag === "watch_started" || tag === "watch_stopped" || tag.startsWith("watch_error:")
  );
}

function isWatchError(payload: SessionWatchEventPayload) {
  return payload.eventTags.some((tag) => tag.startsWith("watch_error:"));
}
