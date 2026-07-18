import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import type {
  AgentSessionFileActivity,
  AgentSessionSummary,
} from "@/features/session-dashboard/lib/session-watch";
import { indexWorkspaceGraph, queryKeys } from "@/shared/lib/agent-api";

import { buildContextGraph } from "./buildContextGraph";
import { layoutContextGraph } from "./layoutContextGraph";

export function useSessionContextGraph({
  fileActivity,
  includeEntireWorkspace,
  pinnedFilePaths,
  selectedFilePath,
  selectedSession,
  showReadFiles,
}: {
  fileActivity: AgentSessionFileActivity;
  includeEntireWorkspace: boolean;
  pinnedFilePaths: string[];
  selectedFilePath: string;
  selectedSession: AgentSessionSummary | null;
  showReadFiles: boolean;
}) {
  const workspacePath = selectedSession?.cwd ?? null;
  const architectureQuery = useQuery({
    queryKey: queryKeys.workspaceGraph(workspacePath ?? "none"),
    queryFn: () => indexWorkspaceGraph(workspacePath ?? ""),
    enabled: workspacePath !== null,
    staleTime: 5 * 60_000,
    gcTime: 5 * 60_000,
  });
  const architectureGraph = workspacePath ? (architectureQuery.data ?? null) : null;

  const { contextGraph, layoutErrorMessage } = useMemo(() => {
    const model = buildContextGraph({
      architectureGraph,
      fileActivity,
      includeEntireWorkspace,
      pinnedFilePaths,
      selectedFilePath,
      workspacePath,
      showReadFiles,
    });

    try {
      return {
        contextGraph: layoutContextGraph(model),
        layoutErrorMessage: "",
      };
    } catch (error) {
      return {
        contextGraph: model,
        layoutErrorMessage: error instanceof Error ? error.message : String(error),
      };
    }
  }, [
    architectureGraph,
    fileActivity,
    includeEntireWorkspace,
    pinnedFilePaths,
    selectedFilePath,
    workspacePath,
    showReadFiles,
  ]);

  return {
    contextGraph,
    errorMessage: architectureQuery.error
      ? architectureQuery.error instanceof Error
        ? architectureQuery.error.message
        : String(architectureQuery.error)
      : layoutErrorMessage,
    isLoading: Boolean(workspacePath) && architectureQuery.isPending,
  };
}
