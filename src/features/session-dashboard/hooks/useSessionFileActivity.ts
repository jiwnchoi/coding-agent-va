import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import type {
  AgentSessionFileActivity,
  AgentSessionSummary,
} from "@/features/session-dashboard/lib/session-watch";

const EMPTY_FILE_ACTIVITY: AgentSessionFileActivity = {
  readFiles: [],
  editedFiles: [],
  impactedFiles: [],
  deletedFiles: [],
  impactedRelations: [],
};

export function useSessionFileActivity(
  selectedSession: AgentSessionSummary | null,
  fileActivityRefreshVersion: number,
  hideCommittedFiles: boolean
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
          hideCommittedFiles,
        });

        if (!disposed) {
          setLoadedFileActivity((current) =>
            areFileActivitiesEqual(current, result) ? current : result
          );
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
  }, [fileActivityRefreshVersion, hideCommittedFiles, selectedSession]);

  const fileActivity = selectedSession ? loadedFileActivity : EMPTY_FILE_ACTIVITY;

  return {
    fileActivity,
    isFileActivityLoading: selectedSession ? isFileActivityLoading : false,
  };
}

function areFileActivitiesEqual(left: AgentSessionFileActivity, right: AgentSessionFileActivity) {
  return (
    areStringArraysEqual(left.readFiles, right.readFiles) &&
    areStringArraysEqual(left.editedFiles, right.editedFiles) &&
    areStringArraysEqual(left.impactedFiles, right.impactedFiles) &&
    areStringArraysEqual(left.deletedFiles, right.deletedFiles) &&
    left.impactedRelations.length === right.impactedRelations.length &&
    left.impactedRelations.every((relation, index) => {
      const other = right.impactedRelations[index];
      return (
        relation.changedFile === other?.changedFile &&
        relation.impactedFile === other.impactedFile &&
        relation.importSpecifier === other.importSpecifier
      );
    })
  );
}

function areStringArraysEqual(left: string[], right: string[]) {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}
