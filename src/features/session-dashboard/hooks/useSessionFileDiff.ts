import { useQuery, useQueryClient } from "@tanstack/react-query";

import type {
  AgentSessionSummary,
  SelectedActivityFile,
  SessionScopeSelection,
} from "@/features/session-dashboard/lib/session-watch";
import { getAgentSessionFileDiff, queryKeys } from "@/shared/lib/agent-api";

export function useSessionFileDiff(
  selectedSession: AgentSessionSummary | null,
  selectedActivityFile: SelectedActivityFile | null,
  sessionScope: SessionScopeSelection | null
) {
  const queryClient = useQueryClient();
  const filePath = selectedActivityFile?.filePath ?? "";
  const cwd = selectedSession?.cwd ?? null;
  const replaySession =
    selectedActivityFile?.activityKey === "edited" ||
    selectedActivityFile?.activityKey === "deleted";
  const queryKey = queryKeys.fileDiff(
    selectedSession?.id ?? "",
    selectedSession?.updatedAtMs ?? 0,
    filePath,
    replaySession,
    sessionScope?.startEntryIndex ?? null,
    sessionScope?.endEntryIndex ?? null
  );
  const query = useQuery({
    queryKey,
    queryFn: () =>
      getAgentSessionFileDiff({
        provider: selectedSession?.provider ?? "codex",
        transcriptPath: selectedSession?.transcriptPath ?? "",
        filePath,
        cwd,
        replaySession,
        startEntryIndex: sessionScope?.startEntryIndex ?? null,
        endEntryIndex: sessionScope?.endEntryIndex ?? null,
      }),
    enabled: selectedSession !== null && selectedActivityFile !== null,
  });

  const clearSelectedFileDiffState = () => {
    if (selectedActivityFile) {
      queryClient.removeQueries({ queryKey });
    }
  };

  return {
    selectedFileDiff: selectedSession && selectedActivityFile ? (query.data ?? null) : null,
    isFileDiffLoading:
      Boolean(selectedSession && selectedActivityFile) && (query.isPending || query.isFetching),
    fileDiffErrorMessage:
      selectedSession && selectedActivityFile && query.error
        ? query.error instanceof Error
          ? query.error.message
          : String(query.error)
        : "",
    clearSelectedFileDiffState,
  };
}
