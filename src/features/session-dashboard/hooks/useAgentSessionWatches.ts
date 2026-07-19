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

const WATCH_REFRESH_DEBOUNCE_MS = 750;
const WATCH_SETTLE_REFRESH_DELAY_MS = 1000;
const EMPTY_WATCH_REGISTRATIONS: SessionWatchRegistration[] = [];

type CurrentRef<T> = {
  current: T;
};

export function useAgentSessionWatches(runtimeSources: AgentRuntimeSource[]) {
  const [watchRegistrations, setWatchRegistrations] = useState<SessionWatchRegistration[]>([]);
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
  }, [runtimeSourceKey]);

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
    let refreshInFlight = false;
    let refreshQueued = false;
    let pendingSessionsRefresh = false;
    let pendingSessionDetailsRefresh = false;
    const activeWatchIds = new Set(watchRegistrations.map((registration) => registration.watchId));

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
      const shouldRefreshSessionDetails = pendingSessionDetailsRefresh;
      pendingSessionsRefresh = false;
      pendingSessionDetailsRefresh = false;

      try {
        if (shouldRefreshSessions) {
          await queryClient.invalidateQueries({ queryKey: ["agent-sessions"] });
        }

        if (!disposed && shouldRefreshSessionDetails) {
          await refreshSelectedSessionDetails();

          if (settleRefreshTimeoutId !== null) {
            window.clearTimeout(settleRefreshTimeoutId);
          }
          settleRefreshTimeoutId = window.setTimeout(() => {
            settleRefreshTimeoutId = null;
            if (!disposed) {
              void refreshSelectedSessionDetails();
            }
          }, WATCH_SETTLE_REFRESH_DELAY_MS);
        }
      } finally {
        refreshInFlight = false;

        if (
          !disposed &&
          (refreshQueued || pendingSessionsRefresh || pendingSessionDetailsRefresh)
        ) {
          refreshQueued = false;
          scheduleRefresh();
        }
      }
    }

    async function refreshSelectedSessionDetails() {
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
        ...(selectedSession?.cwd
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

      pendingSessionsRefresh = true;
      pendingSessionDetailsRefresh ||= selectedSessionInputChanged(
        sessionsRef.current,
        selectedSessionIdRef.current,
        payload.provider,
        changedPaths
      );

      scheduleRefresh();
    });

    // A transcript can change while the native watcher is starting, before the
    // event listener is attached. Re-read the selected session once on startup
    // so that those writes are not left stale until the app is reopened.
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

function selectedSessionInputChanged(
  sessions: AgentSessionSummary[],
  selectedSessionId: string,
  provider: AgentRuntimeSource["provider"],
  changedPaths: Set<string>
) {
  const selectedSession = sessions.find((session) => session.id === selectedSessionId);
  if (!selectedSession || selectedSession.provider !== provider) {
    return false;
  }
  if (changedPaths.has(normalizeWatchPath(selectedSession.transcriptPath))) {
    return true;
  }

  const runtimeHome = normalizeWatchPath(selectedSession.runtimeHome);
  if (provider === "claude") {
    const taskDirectory = normalizeWatchPath(
      `${runtimeHome}/tasks/${selectedSession.providerSessionId}`
    );
    return [...changedPaths].some(
      (path) => path.startsWith(`${taskDirectory}/`) || path.startsWith(`${runtimeHome}/projects/`)
    );
  }

  return [...changedPaths].some((path) => path.startsWith(`${runtimeHome}/sessions/`));
}
