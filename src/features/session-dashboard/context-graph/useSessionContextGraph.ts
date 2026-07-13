import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";

import type {
  AgentSessionFileActivity,
  AgentSessionSummary,
} from "@/features/session-dashboard/lib/session-watch";

import { buildContextGraph } from "./buildContextGraph";
import { layoutContextGraph } from "./layoutContextGraph";
import type { ArchitectureGraph } from "./types";

const MAX_CACHED_WORKSPACES = 8;
type WorkspaceGraphCacheEntry = {
  graph?: ArchitectureGraph;
  promise?: Promise<ArchitectureGraph>;
};
const workspaceGraphCache = new Map<string, WorkspaceGraphCacheEntry>();

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
  const [loadedArchitectureGraph, setLoadedArchitectureGraph] = useState<{
    graph: ArchitectureGraph;
    workspacePath: string;
  } | null>(null);
  const [indexError, setIndexError] = useState<{
    message: string;
    workspacePath: string;
  } | null>(null);
  const architectureGraph = workspacePath
    ? loadedArchitectureGraph?.workspacePath === workspacePath
      ? loadedArchitectureGraph.graph
      : (workspaceGraphCache.get(workspacePath)?.graph ?? null)
    : null;

  useEffect(() => {
    if (!workspacePath) {
      return;
    }
    if (architectureGraph) {
      const cached = workspaceGraphCache.get(workspacePath);
      if (cached) {
        touchWorkspaceGraphCache(workspacePath, cached);
      }
      return;
    }

    const currentWorkspacePath = workspacePath;
    let disposed = false;

    async function indexWorkspace() {
      setIndexError(null);

      try {
        const graph = await loadWorkspaceGraph(currentWorkspacePath);

        if (!disposed) {
          setLoadedArchitectureGraph({ graph, workspacePath: currentWorkspacePath });
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
  ]);

  return {
    contextGraph,
    errorMessage:
      indexError?.workspacePath === workspacePath ? indexError.message : layoutErrorMessage,
    isLoading:
      Boolean(workspacePath) && !architectureGraph && indexError?.workspacePath !== workspacePath,
  };
}

function loadWorkspaceGraph(workspacePath: string) {
  const cached = workspaceGraphCache.get(workspacePath);
  if (cached?.graph) {
    touchWorkspaceGraphCache(workspacePath, cached);
    return Promise.resolve(cached.graph);
  }
  if (cached?.promise) {
    return cached.promise;
  }

  const entry: WorkspaceGraphCacheEntry = {};
  entry.promise = invoke<ArchitectureGraph>("index_workspace_graph", { workspacePath })
    .then((graph) => {
      entry.graph = graph;
      entry.promise = undefined;
      touchWorkspaceGraphCache(workspacePath, entry);
      return graph;
    })
    .catch((error: unknown) => {
      workspaceGraphCache.delete(workspacePath);
      throw error;
    });
  workspaceGraphCache.set(workspacePath, entry);
  return entry.promise;
}

function touchWorkspaceGraphCache(workspacePath: string, entry: WorkspaceGraphCacheEntry) {
  workspaceGraphCache.delete(workspacePath);
  workspaceGraphCache.set(workspacePath, entry);

  const oldestWorkspacePath = workspaceGraphCache.keys().next().value;
  if (workspaceGraphCache.size > MAX_CACHED_WORKSPACES && oldestWorkspacePath) {
    workspaceGraphCache.delete(oldestWorkspacePath);
  }
}
