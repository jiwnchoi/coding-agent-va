import { ACTIVE_SESSION_WINDOW_MS } from "@/features/session-dashboard/constants";
import type { AgentSessionSummary } from "@/lib/session-watch";

export function getActiveSessionIds(sessions: AgentSessionSummary[], nowMs: number) {
  return sessions
    .filter((session) => nowMs - session.updatedAtMs <= ACTIVE_SESSION_WINDOW_MS)
    .map((session) => session.id);
}

export function rotateSession(
  openSessionIds: string[],
  selectedSessionId: string,
  direction: 1 | -1
) {
  if (openSessionIds.length <= 1) {
    return selectedSessionId;
  }

  const currentIndex = openSessionIds.indexOf(selectedSessionId);
  const safeIndex = currentIndex >= 0 ? currentIndex : 0;
  const nextIndex = (safeIndex + direction + openSessionIds.length) % openSessionIds.length;

  return openSessionIds[nextIndex] ?? selectedSessionId;
}

export function closeSessionTab(
  openSessionIds: string[],
  sessionId: string,
  selectedSessionId: string
) {
  const closedSessionIndex = openSessionIds.indexOf(sessionId);

  if (closedSessionIndex < 0) {
    return {
      nextOpenSessionIds: openSessionIds,
      nextSelectedSessionId: selectedSessionId,
    };
  }

  const nextOpenSessionIds = openSessionIds.filter((openSessionId) => openSessionId !== sessionId);

  if (sessionId !== selectedSessionId) {
    return {
      nextOpenSessionIds,
      nextSelectedSessionId: selectedSessionId,
    };
  }

  const fallbackIndex = Math.min(closedSessionIndex, nextOpenSessionIds.length - 1);

  return {
    nextOpenSessionIds,
    nextSelectedSessionId: nextOpenSessionIds[fallbackIndex] ?? "",
  };
}

export function updateSessionHistory(
  historySessionIds: string[],
  previousSessionId: string,
  nextSessionId: string
) {
  if (!previousSessionId || previousSessionId === nextSessionId) {
    return historySessionIds;
  }

  return [
    previousSessionId,
    ...historySessionIds.filter(
      (sessionId) => sessionId !== previousSessionId && sessionId !== nextSessionId
    ),
  ];
}

export function selectMostRecentlyActiveSession(
  historySessionIds: string[],
  openSessionIds: string[],
  selectedSessionId: string
) {
  if (openSessionIds.length <= 1) {
    return selectedSessionId;
  }

  const nextSessionId = historySessionIds.find(
    (sessionId) => sessionId !== selectedSessionId && openSessionIds.includes(sessionId)
  );

  return nextSessionId ?? selectedSessionId;
}

export function selectSessionByTabNumber(openSessionIds: string[], tabNumber: number) {
  const sessionIndex = tabNumber === 0 ? 9 : tabNumber - 1;

  return openSessionIds[sessionIndex] ?? "";
}

export function reconcileTabState({
  currentDismissedSessionIds,
  currentOpenSessionIds,
  currentSelectedSessionId,
  nowMs,
  previousActiveSessionIds,
  sessions,
}: {
  currentDismissedSessionIds: string[];
  currentOpenSessionIds: string[];
  currentSelectedSessionId: string;
  nowMs: number;
  previousActiveSessionIds: string[];
  sessions: AgentSessionSummary[];
}) {
  const activeSessionIds = getActiveSessionIds(sessions, nowMs);
  const previousActiveSessionIdSet = new Set(previousActiveSessionIds);
  const availableSessionIdSet = new Set(sessions.map((session) => session.id));
  const nextDismissedSessionIds = currentDismissedSessionIds.filter((sessionId) =>
    availableSessionIdSet.has(sessionId)
  );
  const dismissedSessionIdSet = new Set(nextDismissedSessionIds);
  const newlyActiveSessionIds = activeSessionIds.filter(
    (sessionId) => !previousActiveSessionIdSet.has(sessionId)
  );
  const baseOpenSessionIds = currentOpenSessionIds.filter((sessionId) =>
    availableSessionIdSet.has(sessionId)
  );
  const nextOpenSessionIds = [...baseOpenSessionIds];

  for (const sessionId of activeSessionIds) {
    if (newlyActiveSessionIds.includes(sessionId) || !dismissedSessionIdSet.has(sessionId)) {
      if (!nextOpenSessionIds.includes(sessionId)) {
        nextOpenSessionIds.push(sessionId);
      }
    }
  }

  const isSelectedSessionAvailable = availableSessionIdSet.has(currentSelectedSessionId);
  const nextSelectedSessionId =
    currentSelectedSessionId && isSelectedSessionAvailable
      ? currentSelectedSessionId
      : (nextOpenSessionIds[0] ?? (isSelectedSessionAvailable ? currentSelectedSessionId : ""));

  return {
    activeSessionIds,
    nextDismissedSessionIds,
    nextOpenSessionIds,
    nextSelectedSessionId,
  };
}
