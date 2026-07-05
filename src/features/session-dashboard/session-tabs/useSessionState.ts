import { useCallback, useEffect, useRef, useState } from "react";

import type { AgentSessionSummary } from "@/lib/session-watch";

import { closeSessionTab, reconcileTabState, updateSessionHistory } from "./session-tab-utils";

export function useSessionState() {
  const [sessions, setSessions] = useState<AgentSessionSummary[]>([]);
  const [openSessionIds, setOpenSessionIds] = useState<string[]>([]);
  const [dismissedSessionIds, setDismissedSessionIds] = useState<string[]>([]);
  const [selectedSessionId, setSelectedSessionId] = useState<string>("");
  const [sessionHistoryIds, setSessionHistoryIds] = useState<string[]>([]);
  const activeSessionIdsRef = useRef<string[]>([]);
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
      setSessionHistoryIds((currentHistorySessionIds) =>
        currentHistorySessionIds.filter((historySessionId) => historySessionId !== sessionId)
      );
      setDismissedSessionIds((currentDismissedSessionIds) =>
        currentDismissedSessionIds.includes(sessionId)
          ? currentDismissedSessionIds
          : [...currentDismissedSessionIds, sessionId]
      );
    },
    [setSelectedSessionWithHistory]
  );

  const reconcileSessions = useCallback(
    (nextSessions: AgentSessionSummary[], currentNowMs: number) => {
      const {
        activeSessionIds,
        nextDismissedSessionIds,
        nextOpenSessionIds,
        nextSelectedSessionId,
      } = reconcileTabState({
        currentDismissedSessionIds: dismissedSessionIdsRef.current,
        currentOpenSessionIds: openSessionIdsRef.current,
        currentSelectedSessionId: selectedSessionIdRef.current,
        nowMs: currentNowMs,
        previousActiveSessionIds: activeSessionIdsRef.current,
        sessions: nextSessions,
      });

      setSessions(nextSessions);
      setDismissedSessionIds(nextDismissedSessionIds);
      setOpenSessionIds(nextOpenSessionIds);
      setSessionHistoryIds((currentHistorySessionIds) =>
        currentHistorySessionIds.filter(
          (sessionId) =>
            nextOpenSessionIds.includes(sessionId) &&
            nextSessions.some((session) => session.id === sessionId)
        )
      );
      setSelectedSessionWithHistory(nextSelectedSessionId);
      activeSessionIdsRef.current = activeSessionIds;
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
    selectSession,
    handleCloseSession,
    reconcileSessions,
  };
}
