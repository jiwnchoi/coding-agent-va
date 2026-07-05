import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";

import type { AgentSessionFileActivity, AgentSessionSummary } from "@/lib/session-watch";

const EMPTY_FILE_ACTIVITY: AgentSessionFileActivity = {
  readFiles: [],
  editedFiles: [],
  impactedFiles: [],
  deletedFiles: [],
  impactedRelations: [],
};

export function useSessionFileActivity(
  selectedSession: AgentSessionSummary | null,
  fileActivityRefreshVersion: number
) {
  const [loadedFileActivity, setLoadedFileActivity] =
    useState<AgentSessionFileActivity>(EMPTY_FILE_ACTIVITY);
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
        const result = await invoke<AgentSessionFileActivity>("get_agent_session_file_activity", {
          provider: currentSession.provider,
          transcriptPath: currentSession.transcriptPath,
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
