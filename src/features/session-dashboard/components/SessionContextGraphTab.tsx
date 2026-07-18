import { useSessionFileActivity } from "@/features/session-dashboard/hooks/useSessionFileActivity";
import type {
  AgentSessionSummary,
  SelectedActivityFile,
} from "@/features/session-dashboard/lib/session-watch";
import type { DescriptionSettings } from "@/shared/lib/generated/bindings";

import { SessionContextGraphView } from "./SessionContextGraphView";

export function SessionContextGraphTab({
  descriptionSettings,
  hideCommittedFiles,
  showReadFiles,
  isSessionListLoading,
  selectedActivityFile,
  selectedSession,
  onSelectFile,
}: {
  descriptionSettings: DescriptionSettings;
  hideCommittedFiles: boolean;
  showReadFiles: boolean;
  isSessionListLoading: boolean;
  selectedActivityFile: SelectedActivityFile | null;
  selectedSession: AgentSessionSummary;
  onSelectFile: (selection: SelectedActivityFile) => void;
}) {
  const { fileActivity, isFileActivityLoading } = useSessionFileActivity(
    selectedSession,
    hideCommittedFiles
  );
  return (
    <div className="absolute inset-0">
      <SessionContextGraphView
        descriptionSettings={descriptionSettings}
        fileActivity={fileActivity}
        isFileActivityLoading={isSessionListLoading || isFileActivityLoading}
        selectedActivityFile={selectedActivityFile}
        selectedSession={selectedSession}
        showReadFiles={showReadFiles}
        onSelectFile={onSelectFile}
      />
    </div>
  );
}
