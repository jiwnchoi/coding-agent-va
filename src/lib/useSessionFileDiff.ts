import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useState } from "react";

import type {
  CodexSessionFileDiff,
  CodexSessionSummary,
  SelectedActivityFile,
} from "@/lib/session-watch";

export function useSessionFileDiff(
  selectedSession: CodexSessionSummary | null,
  selectedActivityFile: SelectedActivityFile | null
) {
  const [loadedSelectedFileDiff, setLoadedSelectedFileDiff] = useState<CodexSessionFileDiff | null>(
    null
  );
  const [isFileDiffLoading, setIsFileDiffLoading] = useState(false);
  const [loadedFileDiffErrorMessage, setLoadedFileDiffErrorMessage] = useState("");
  const clearSelectedFileDiffState = useCallback(() => {
    setLoadedSelectedFileDiff(null);
    setLoadedFileDiffErrorMessage("");
  }, []);

  useEffect(() => {
    if (!selectedSession || !selectedActivityFile) {
      return;
    }

    const currentFilePath = selectedActivityFile.filePath;
    const currentCwd = selectedSession.cwd;
    let disposed = false;

    async function loadFileDiff() {
      setIsFileDiffLoading(true);
      setLoadedFileDiffErrorMessage("");

      try {
        const result = await invoke<CodexSessionFileDiff>("get_codex_session_file_diff", {
          filePath: currentFilePath,
          cwd: currentCwd,
        });

        if (!disposed) {
          setLoadedSelectedFileDiff(result);
        }
      } catch (error) {
        if (!disposed) {
          setLoadedSelectedFileDiff(null);
          setLoadedFileDiffErrorMessage(
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

  const selectedFileDiff = useMemo(() => {
    if (!selectedSession || !selectedActivityFile) {
      return null;
    }

    return loadedSelectedFileDiff;
  }, [loadedSelectedFileDiff, selectedActivityFile, selectedSession]);

  const fileDiffErrorMessage = useMemo(() => {
    if (!selectedSession || !selectedActivityFile) {
      return "";
    }

    return loadedFileDiffErrorMessage;
  }, [loadedFileDiffErrorMessage, selectedActivityFile, selectedSession]);

  return {
    selectedFileDiff,
    isFileDiffLoading: selectedSession && selectedActivityFile ? isFileDiffLoading : false,
    fileDiffErrorMessage,
    clearSelectedFileDiffState,
  };
}
