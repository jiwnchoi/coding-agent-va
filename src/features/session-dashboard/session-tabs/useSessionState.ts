import { useCallback, useEffect, useRef, useState } from "react";

import type { AgentSessionSummary } from "@/features/session-dashboard/lib/session-watch";

import { closeSessionTab, reconcileTabState, updateSessionHistory } from "./session-tab-utils";

export function useSessionState() {
  const [sessions, setSessions] = useState<AgentSessionSummary[]>([]);
  const [openSessionIds, setOpenSessionIds] = useState<string[]>([]);
  const [dismissedSessionIds, setDismissedSessionIds] = useState<string[]>([]);
  const [selectedSessionId, setSelectedSessionId] = useState<string>("");
  const [sessionHistoryIds, setSessionHistoryIds] = useState<string[]>([]);
  const [viewedSessionUpdatedAtMs, setViewedSessionUpdatedAtMs] = useState<Record<string, number>>(
    {}
  );
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

  const setSelectedSessionWithHistory = useCallback((nextSessionId: string) => {
    const previousSessionId = selectedSessionIdRef.current;

    setSessionHistoryIds((currentHistorySessionIds) =>
      updateSessionHistory(currentHistorySessionIds, previousSessionId, nextSessionId)
    );
    setSelectedSessionId(nextSessionId);
  }, []);

  const selectSession = useCallback(
    (sessionId: string) => {
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
      setDismissedSessionIds((currentDismissedSessionIds) =>
        currentDismissedSessionIds.includes(sessionId)
          ? currentDismissedSessionIds
          : [...currentDismissedSessionIds, sessionId]
      );
    },
    [setSelectedSessionWithHistory]
  );

  const markSelectedSessionAsViewed = useCallback(() => {
    const selectedSession = sessionsRef.current.find(
      (session) => session.id === selectedSessionIdRef.current
    );

    if (!selectedSession) {
      return;
    }

    setViewedSessionUpdatedAtMs((currentViewed) => ({
      ...currentViewed,
      [selectedSession.id]: selectedSession.updatedAtMs,
    }));
  }, []);

  const reconcileSessions = useCallback(
    (nextSessions: AgentSessionSummary[]) => {
      const { nextOpenSessionIds, nextSelectedSessionId } = reconcileTabState({
        currentDismissedSessionIds: dismissedSessionIdsRef.current,
        currentOpenSessionIds: openSessionIdsRef.current,
        currentSelectedSessionId: selectedSessionIdRef.current,
        sessions: nextSessions,
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
        const initialSession = nextSessions.find((session) => session.id === nextSelectedSessionId);

        if (!selectedSessionIdRef.current && initialSession) {
          nextViewed[initialSession.id] = initialSession.updatedAtMs;
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
    markSelectedSessionAsViewed,
    reconcileSessions,
  };
}
