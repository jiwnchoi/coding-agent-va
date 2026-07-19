import type { SessionScopeSelection } from "./session-watch";

type EntryRange = {
  startEntryIndex: number;
  endEntryIndex: number;
};

export function buildSessionScopeSelection(
  turnId: string,
  taskId: string | null,
  range: EntryRange
): SessionScopeSelection {
  return {
    turnId,
    taskId,
    startEntryIndex: range.startEntryIndex,
    endEntryIndex: range.endEntryIndex,
  };
}
