import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";

import type { CodexSessionFileActivity, CodexSessionSummary } from "@/lib/session-watch";

const EMPTY_FILE_ACTIVITY: CodexSessionFileActivity = {
  readFiles: [],
  editedFiles: [],
  impactedFiles: [],
  deletedFiles: [],
};

export function useSessionFileActivity(
  selectedSession: CodexSessionSummary | null,
  fileActivityRefreshVersion: number
) {
  const [loadedFileActivity, setLoadedFileActivity] =
    useState<CodexSessionFileActivity>(EMPTY_FILE_ACTIVITY);
  const [isFileActivityLoading, setIsFileActivityLoading] = useState(false);

  useEffect(() => {
    if (!selectedSession) {
      return;
    }

    const currentSession = selectedSession;
    let disposed = false;

    async function loadFileActivity() {
      setIsFileActivityLoading(true);

      try {
        const result = await invoke<CodexSessionFileActivity>("get_codex_session_file_activity", {
          rolloutPath: currentSession.rolloutPath,
          cwd: currentSession.cwd,
        });

        if (!disposed) {
          setLoadedFileActivity(result);
        }
      } finally {
        if (!disposed) {
          setIsFileActivityLoading(false);
        }
      }
    }

    void loadFileActivity();

    return () => {
      disposed = true;
    };
  }, [fileActivityRefreshVersion, selectedSession]);

  const fileActivity = useMemo(() => {
    if (!selectedSession) {
      return EMPTY_FILE_ACTIVITY;
    }

    return loadedFileActivity;
  }, [loadedFileActivity, selectedSession]);

  return {
    fileActivity,
    isFileActivityLoading: selectedSession ? isFileActivityLoading : false,
  };
}
