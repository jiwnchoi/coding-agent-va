import type { AgentSessionSummary } from "@/features/session-dashboard/lib/session-watch";

export function isSessionChecked(
  session: AgentSessionSummary,
  viewedSessionUpdatedAtMs: Record<string, number>
) {
  return (viewedSessionUpdatedAtMs[session.id] ?? -1) >= session.updatedAtMs;
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
    return { nextOpenSessionIds: openSessionIds, nextSelectedSessionId: selectedSessionId };
  }

  const nextOpenSessionIds = openSessionIds.filter((openSessionId) => openSessionId !== sessionId);

  if (sessionId !== selectedSessionId) {
    return { nextOpenSessionIds, nextSelectedSessionId: selectedSessionId };
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

export function selectMostRecentlyUsedSession(
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
  openUntrackedSessions,
  sessions,
}: {
  currentDismissedSessionIds: string[];
  currentOpenSessionIds: string[];
  currentSelectedSessionId: string;
  openUntrackedSessions: boolean;
  sessions: AgentSessionSummary[];
}) {
  const dismissedSessionIdSet = new Set(currentDismissedSessionIds);
  const nextOpenSessionIds = [...currentOpenSessionIds];

  if (openUntrackedSessions) {
    for (const session of sessions) {
      if (!dismissedSessionIdSet.has(session.id) && !nextOpenSessionIds.includes(session.id)) {
        nextOpenSessionIds.push(session.id);
      }
    }
  }

  const isSelectedSessionAvailable = nextOpenSessionIds.includes(currentSelectedSessionId);
  const nextSelectedSessionId =
    currentSelectedSessionId && isSelectedSessionAvailable
      ? currentSelectedSessionId
      : (nextOpenSessionIds[0] ?? (isSelectedSessionAvailable ? currentSelectedSessionId : ""));

  return {
    nextOpenSessionIds,
    nextSelectedSessionId,
  };
}
