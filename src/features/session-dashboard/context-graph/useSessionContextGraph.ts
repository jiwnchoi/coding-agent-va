import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";

import type {
  AgentSessionFileActivity,
  AgentSessionSummary,
} from "@/features/session-dashboard/lib/session-watch";

import { buildContextGraph } from "./buildContextGraph";
import { layoutContextGraphWithHierarchy } from "./layoutContextGraphWithHierarchy";
import type { ArchitectureGraph } from "./types";

export function useSessionContextGraph({
  fileActivity,
  includeEntireWorkspace,
  pinnedFilePaths,
  selectedFilePath,
  selectedSession,
}: {
  fileActivity: AgentSessionFileActivity;
  includeEntireWorkspace: boolean;
  pinnedFilePaths: string[];
  selectedFilePath: string;
  selectedSession: AgentSessionSummary | null;
}) {
  const workspacePath = selectedSession?.cwd ?? null;
  const [indexedGraphsByWorkspacePath, setIndexedGraphsByWorkspacePath] = useState(
    () => new Map<string, ArchitectureGraph>()
  );
  const [indexError, setIndexError] = useState<{
    message: string;
    workspacePath: string;
  } | null>(null);
  const architectureGraph = workspacePath
    ? (indexedGraphsByWorkspacePath.get(workspacePath) ?? null)
    : null;

  useEffect(() => {
    if (!workspacePath || architectureGraph) {
      return;
    }

    const currentWorkspacePath = workspacePath;
    let disposed = false;

    async function indexWorkspace() {
      setIndexError(null);

      try {
        const graph = await invoke<ArchitectureGraph>("index_workspace_graph", {
          workspacePath: currentWorkspacePath,
        });

        if (!disposed) {
          setIndexedGraphsByWorkspacePath((graphsByWorkspacePath) => {
            const nextGraphsByWorkspacePath = new Map(graphsByWorkspacePath);
            nextGraphsByWorkspacePath.set(currentWorkspacePath, graph);
            return nextGraphsByWorkspacePath;
          });
        }
      } catch (error) {
        if (!disposed) {
          setIndexError({
            message: error instanceof Error ? error.message : String(error),
            workspacePath: currentWorkspacePath,
          });
        }
      }
    }

    void indexWorkspace();

    return () => {
      disposed = true;
    };
  }, [architectureGraph, workspacePath]);

  const { contextGraph, layoutErrorMessage } = useMemo(() => {
    const model = buildContextGraph({
      architectureGraph,
      fileActivity,
      includeEntireWorkspace,
      pinnedFilePaths,
      selectedFilePath,
      workspacePath,
    });

    try {
      return {
        contextGraph: layoutContextGraphWithHierarchy(model),
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
  ]);

  return {
    contextGraph,
    errorMessage:
      indexError?.workspacePath === workspacePath ? indexError.message : layoutErrorMessage,
    isLoading:
      Boolean(workspacePath) && !architectureGraph && indexError?.workspacePath !== workspacePath,
  };
}
