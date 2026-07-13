import { useQuery, useQueryClient } from "@tanstack/react-query";

import type {
  AgentSessionSummary,
  SelectedActivityFile,
} from "@/features/session-dashboard/lib/session-watch";
import { getAgentSessionFileDiff, queryKeys } from "@/shared/lib/agent-api";

export function useSessionFileDiff(
  selectedSession: AgentSessionSummary | null,
  selectedActivityFile: SelectedActivityFile | null
) {
  const queryClient = useQueryClient();
  const filePath = selectedActivityFile?.filePath ?? "";
  const cwd = selectedSession?.cwd ?? null;
  const query = useQuery({
    queryKey: queryKeys.fileDiff(cwd, filePath),
    queryFn: () => getAgentSessionFileDiff({ filePath, cwd }),
    enabled: selectedSession !== null && selectedActivityFile !== null,
  });

  const clearSelectedFileDiffState = () => {
    if (selectedActivityFile) {
      queryClient.removeQueries({ queryKey: queryKeys.fileDiff(cwd, filePath) });
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
