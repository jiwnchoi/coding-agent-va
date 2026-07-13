import { useQuery } from "@tanstack/react-query";

import type {
  AgentSessionFileActivity,
  AgentSessionSummary,
} from "@/features/session-dashboard/lib/session-watch";
import { getAgentSessionFileActivity, queryKeys } from "@/shared/lib/agent-api";

const EMPTY_FILE_ACTIVITY: AgentSessionFileActivity = {
  readFiles: [],
  editedFiles: [],
  impactedFiles: [],
  deletedFiles: [],
  impactedRelations: [],
};

export function useSessionFileActivity(
  selectedSession: AgentSessionSummary | null,
  hideCommittedFiles: boolean
) {
  const query = useQuery({
    queryKey: queryKeys.fileActivity(selectedSession?.id ?? "none", hideCommittedFiles),
    queryFn: () =>
      getAgentSessionFileActivity({
        provider: selectedSession?.provider ?? "",
        transcriptPath: selectedSession?.transcriptPath ?? "",
        cwd: selectedSession?.cwd ?? null,
        hideCommittedFiles,
      }),
    enabled: selectedSession !== null,
  });

  return {
    fileActivity: selectedSession ? (query.data ?? EMPTY_FILE_ACTIVITY) : EMPTY_FILE_ACTIVITY,
    isFileActivityLoading: selectedSession ? query.isPending || query.isFetching : false,
    fileActivityErrorMessage: selectedSession && query.error ? String(query.error) : "",
  };
}
