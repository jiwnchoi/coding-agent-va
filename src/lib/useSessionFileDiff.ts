import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";

import type {
  CodexSessionFileDiff,
  CodexSessionSummary,
  SelectedActivityFile,
} from "@/lib/session-watch";

export function useSessionFileDiff(
  selectedSession: CodexSessionSummary | null,
  selectedActivityFile: SelectedActivityFile | null
) {
  const [selectedFileDiff, setSelectedFileDiff] = useState<CodexSessionFileDiff | null>(null);
  const [isFileDiffLoading, setIsFileDiffLoading] = useState(false);
  const [fileDiffErrorMessage, setFileDiffErrorMessage] = useState("");
  const clearSelectedFileDiffState = useCallback(() => {
    setSelectedFileDiff(null);
    setFileDiffErrorMessage("");
  }, []);

  useEffect(() => {
    if (!selectedSession || !selectedActivityFile) {
      setSelectedFileDiff(null);
      setIsFileDiffLoading(false);
      setFileDiffErrorMessage("");
      return;
    }

    const currentFilePath = selectedActivityFile.filePath;
    const currentCwd = selectedSession.cwd;
    let disposed = false;

    async function loadFileDiff() {
      setIsFileDiffLoading(true);
      setFileDiffErrorMessage("");

      try {
        const result = await invoke<CodexSessionFileDiff>("get_codex_session_file_diff", {
          filePath: currentFilePath,
          cwd: currentCwd,
        });

        if (!disposed) {
          setSelectedFileDiff(result);
        }
      } catch (error) {
        if (!disposed) {
          setSelectedFileDiff(null);
          setFileDiffErrorMessage(
            error instanceof Error ? error.message : "Failed to load file diff."
          );
        }
      } finally {
        if (!disposed) {
          setIsFileDiffLoading(false);
        }
      }
    }

    void loadFileDiff();

    return () => {
      disposed = true;
    };
  }, [selectedActivityFile, selectedSession]);

  return {
    selectedFileDiff,
    isFileDiffLoading,
    fileDiffErrorMessage,
    clearSelectedFileDiffState,
  };
}
