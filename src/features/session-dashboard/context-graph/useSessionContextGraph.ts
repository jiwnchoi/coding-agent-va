import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import type {
  AgentSessionFileActivity,
  AgentSessionSummary,
} from "@/features/session-dashboard/lib/session-watch";

import { buildContextGraph } from "./buildContextGraph";
import { layoutContextGraphWithHierarchy } from "./layoutContextGraphWithHierarchy";
import type { ArchitectureGraph, ContextGraphModel } from "./types";

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
  const sessionId = selectedSession?.id ?? null;
  const [indexedArchitectureGraph, setIndexedArchitectureGraph] = useState<{
    graph: ArchitectureGraph;
    workspacePath: string;
  } | null>(null);
  const [contextGraph, setContextGraph] = useState<ContextGraphModel | null>(null);
  const [errorMessage, setErrorMessage] = useState("");
  const [isIndexing, setIsIndexing] = useState(false);
  const [isLayouting, setIsLayouting] = useState(false);
  const architectureGraph =
    indexedArchitectureGraph?.workspacePath === workspacePath
      ? indexedArchitectureGraph.graph
      : null;

  useEffect(() => {
    if (!workspacePath) {
      return;
    }

    const currentWorkspacePath = workspacePath;
    let disposed = false;

    async function indexWorkspace() {
      setIsIndexing(true);
      setErrorMessage("");

      try {
        const graph = await invoke<ArchitectureGraph>("index_workspace_graph", {
          workspacePath: currentWorkspacePath,
        });

        if (!disposed) {
          setIndexedArchitectureGraph({ graph, workspacePath: currentWorkspacePath });
        }
      } catch (error) {
        if (!disposed) {
          setIndexedArchitectureGraph(null);
          setErrorMessage(error instanceof Error ? error.message : String(error));
        }
      } finally {
        if (!disposed) {
          setIsIndexing(false);
        }
      }
    }

    void indexWorkspace();

    return () => {
      disposed = true;
    };
  }, [sessionId, workspacePath]);

  useEffect(() => {
    let disposed = false;

    async function layoutGraph() {
      const model = buildContextGraph({
        architectureGraph,
        fileActivity,
        includeEntireWorkspace,
        pinnedFilePaths,
        selectedFilePath,
        workspacePath,
      });

      setIsLayouting(true);

      try {
        const layoutedModel = layoutContextGraphWithHierarchy(model);

        if (!disposed) {
          setContextGraph(layoutedModel);
        }
      } catch (error) {
        if (!disposed) {
          setContextGraph(model);
          setErrorMessage(error instanceof Error ? error.message : String(error));
        }
      } finally {
        if (!disposed) {
          setIsLayouting(false);
        }
      }
    }

    void layoutGraph();

    return () => {
      disposed = true;
    };
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
    errorMessage,
    isLoading: isIndexing || isLayouting,
  };
}
