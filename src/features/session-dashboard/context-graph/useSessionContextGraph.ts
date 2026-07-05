import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import type { AgentSessionFileActivity, AgentSessionSummary } from "@/lib/session-watch";

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
  const [architectureGraph, setArchitectureGraph] = useState<ArchitectureGraph | null>(null);
  const [contextGraph, setContextGraph] = useState<ContextGraphModel | null>(null);
  const [errorMessage, setErrorMessage] = useState("");
  const [isIndexing, setIsIndexing] = useState(false);
  const [isLayouting, setIsLayouting] = useState(false);

  useEffect(() => {
    if (!selectedSession?.cwd) {
      setArchitectureGraph(null);
      setErrorMessage("");
      return;
    }

    const currentSession = selectedSession;
    let disposed = false;

    async function indexWorkspace() {
      setIsIndexing(true);
      setErrorMessage("");

      try {
        const graph = await invoke<ArchitectureGraph>("index_workspace_graph", {
          workspacePath: currentSession.cwd,
        });

        if (!disposed) {
          setArchitectureGraph(graph);
        }
      } catch (error) {
        if (!disposed) {
          setArchitectureGraph(null);
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
  }, [selectedSession?.cwd, selectedSession?.id]);

  useEffect(() => {
    let disposed = false;

    async function layoutGraph() {
      const model = buildContextGraph({
        architectureGraph,
        fileActivity,
        includeEntireWorkspace,
        pinnedFilePaths,
        selectedFilePath,
        workspacePath: selectedSession?.cwd ?? null,
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
    selectedSession?.cwd,
  ]);

  return {
    contextGraph,
    errorMessage,
    isLoading: isIndexing || isLayouting,
  };
}
