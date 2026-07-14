import { useCallback, useEffect, useRef, useState } from "react";

import type { AgentSessionSummary } from "@/features/session-dashboard/lib/session-watch";

import {
  closeSessionTab,
  isSessionChecked,
  reconcileTabState,
  updateSessionHistory,
} from "./session-tab-utils";

export function useSessionState() {
  const [sessions, setSessions] = useState<AgentSessionSummary[]>([]);
  const [openSessionIds, setOpenSessionIds] = useState<string[]>([]);
  const [selectedSessionId, setSelectedSessionId] = useState<string>("");
  const [sessionHistoryIds, setSessionHistoryIds] = useState<string[]>([]);
  const [viewedSessionUpdatedAtMs, setViewedSessionUpdatedAtMs] = useState<Record<string, number>>(
    {}
  );
  const sessionsRef = useRef(sessions);
  const openSessionIdsRef = useRef(openSessionIds);
  const selectedSessionIdRef = useRef(selectedSessionId);
  const sessionHistoryIdsRef = useRef(sessionHistoryIds);
  const viewedSessionUpdatedAtMsRef = useRef(viewedSessionUpdatedAtMs);
  const hasInitializedSessionTabsRef = useRef(false);
  const hasInitializedViewedSessionsRef = useRef(false);

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

  useEffect(() => {
    viewedSessionUpdatedAtMsRef.current = viewedSessionUpdatedAtMs;
  }, [viewedSessionUpdatedAtMs]);

  const setSelectedSessionWithHistory = useCallback((nextSessionId: string) => {
    const previousSessionId = selectedSessionIdRef.current;

    setSessionHistoryIds((currentHistorySessionIds) =>
      updateSessionHistory(currentHistorySessionIds, previousSessionId, nextSessionId)
    );
    setSelectedSessionId(nextSessionId);
  }, []);

  const selectSession = useCallback(
    (sessionId: string) => {
      setOpenSessionIds((currentOpenSessionIds) => {
        if (currentOpenSessionIds.includes(sessionId)) {
          return currentOpenSessionIds;
        }

        return [...currentOpenSessionIds, sessionId];
      });
      setSelectedSessionWithHistory(sessionId);
    },
    [setSelectedSessionWithHistory]
  );

  const handleCloseSession = useCallback(
    (sessionId: string) => {
      const { nextOpenSessionIds, nextSelectedSessionId } = closeSessionTab(
        openSessionIdsRef.current,
        sessionId,
        selectedSessionIdRef.current
      );

      setOpenSessionIds(nextOpenSessionIds);
      setSelectedSessionWithHistory(nextSelectedSessionId);
      setSessionHistoryIds((currentSessionHistoryIds) =>
        currentSessionHistoryIds.filter((historySessionId) => historySessionId !== sessionId)
      );
    },
    [setSelectedSessionWithHistory]
  );

  const markSessionAsViewed = useCallback((sessionId: string) => {
    const session = sessionsRef.current.find((candidate) => candidate.id === sessionId);

    if (!session) {
      return;
    }

    setViewedSessionUpdatedAtMs((currentViewed) => ({
      ...currentViewed,
      [session.id]: session.updatedAtMs,
    }));
  }, []);

  const markSelectedSessionAsViewed = useCallback(() => {
    markSessionAsViewed(selectedSessionIdRef.current);
  }, [markSessionAsViewed]);

  const reconcileSessions = useCallback(
    (nextSessions: AgentSessionSummary[], markNewSessionsAsViewed = false) => {
      const shouldInitializeSessionTabs = !hasInitializedSessionTabsRef.current;
      hasInitializedSessionTabsRef.current = true;
      const shouldInitializeViewedSessions = !hasInitializedViewedSessionsRef.current;
      hasInitializedViewedSessionsRef.current = true;
      const currentOpenSessionIds = shouldInitializeSessionTabs ? [] : openSessionIdsRef.current;
      const currentSelectedSessionId = shouldInitializeSessionTabs
        ? ""
        : selectedSessionIdRef.current;
      const currentSessionIds = new Set(sessionsRef.current.map((session) => session.id));
      const sessionIdsToOpen = shouldInitializeSessionTabs
        ? []
        : nextSessions
            .filter(
              (session) =>
                (!markNewSessionsAsViewed || currentSessionIds.has(session.id)) &&
                !isSessionChecked(session, viewedSessionUpdatedAtMsRef.current)
            )
            .map((session) => session.id);
      const { nextOpenSessionIds, nextSelectedSessionId } = reconcileTabState({
        currentOpenSessionIds,
        currentSelectedSessionId,
        sessionIdsToOpen,
      });

      const nextSessionIds = new Set(nextSessions.map((session) => session.id));
      const retainedOpenSessions = sessionsRef.current.filter(
        (session) => !nextSessionIds.has(session.id) && nextOpenSessionIds.includes(session.id)
      );

      setSessions([...nextSessions, ...retainedOpenSessions]);
      setViewedSessionUpdatedAtMs((currentViewed) => {
        const nextViewed = Object.fromEntries(
          Object.entries(currentViewed).filter(
            ([sessionId]) =>
              nextSessions.some((session) => session.id === sessionId) ||
              nextOpenSessionIds.includes(sessionId)
          )
        );
        if (shouldInitializeViewedSessions || markNewSessionsAsViewed) {
          for (const session of nextSessions) {
            if (shouldInitializeViewedSessions || !currentSessionIds.has(session.id)) {
              nextViewed[session.id] = session.updatedAtMs;
            }
          }
        }

        return nextViewed;
      });
      setOpenSessionIds(nextOpenSessionIds);
      setSessionHistoryIds((currentHistorySessionIds) =>
        currentHistorySessionIds.filter(
          (sessionId) =>
            (nextOpenSessionIds.includes(sessionId) &&
              nextSessions.some((session) => session.id === sessionId)) ||
            retainedOpenSessions.some((session) => session.id === sessionId)
        )
      );
      setSelectedSessionWithHistory(nextSelectedSessionId);
    },
    [setSelectedSessionWithHistory]
  );

  return {
    sessions,
    openSessionIds,
    selectedSessionId,
    sessionsRef,
    openSessionIdsRef,
    selectedSessionIdRef,
    sessionHistoryIdsRef,
    viewedSessionUpdatedAtMs,
    selectSession,
    handleCloseSession,
    markSessionAsViewed,
    markSelectedSessionAsViewed,
    reconcileSessions,
  };
}
