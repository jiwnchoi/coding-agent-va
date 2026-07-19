import { useQuery } from "@tanstack/react-query";

import type { AgentSessionSummary } from "@/features/session-dashboard/lib/session-watch";
import { getAgentSessionDetails, queryKeys } from "@/shared/lib/agent-api";

export function useSessionDetails(session: AgentSessionSummary) {
  return useQuery({
    queryKey: queryKeys.sessionDetails(session.id),
    queryFn: () =>
      getAgentSessionDetails({
        provider: session.provider,
        providerSessionId: session.providerSessionId,
        transcriptPath: session.transcriptPath,
        runtimeHome: session.runtimeHome,
        cwd: session.cwd,
      }),
    staleTime: Number.POSITIVE_INFINITY,
  });
}
