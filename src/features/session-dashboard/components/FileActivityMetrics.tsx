import { Eye, FilePenLine, TriangleAlert } from "lucide-react";

import { normalizeWorkspacePath } from "@/features/session-dashboard/context-graph/contextGraphPaths";
import type { AgentSessionFileActivity } from "@/shared/lib/generated/bindings";
import { cn } from "@/shared/lib/utils";

import styles from "./FileActivityMetrics.module.css";

export function FileActivityMetrics({
  fileActivity,
  workspacePath,
}: {
  fileActivity: AgentSessionFileActivity;
  workspacePath: string | null;
}) {
  const normalizePaths = (paths: string[]) =>
    new Set(paths.map((path) => normalizeWorkspacePath(path, workspacePath ?? "")));
  const editedCount = normalizePaths(fileActivity.editedFiles).size;
  const readFiles = normalizePaths(fileActivity.readFiles);
  const impactedFiles = normalizePaths(fileActivity.impactedFiles);
  const readImpactedCount = [...impactedFiles].filter((file) => readFiles.has(file)).length;
  const unreadImpactedCount = impactedFiles.size - readImpactedCount;

  return (
    <span
      className="flex shrink-0 items-center gap-1"
      aria-label={`${editedCount} edited files, ${readImpactedCount} read impacted files, ${unreadImpactedCount} unread impacted files`}>
      <span
        className={cn(styles.metric, editedCount > 0 ? styles.edited : styles.empty)}
        title={`${editedCount} edited files`}>
        <FilePenLine className="size-2.5" aria-hidden="true" />
        {editedCount}
      </span>
      <span
        className={cn(styles.metric, readImpactedCount > 0 ? styles.readImpacted : styles.empty)}
        title={`${readImpactedCount} impacted files read by the agent`}>
        <Eye className="size-2.5" aria-hidden="true" />
        {readImpactedCount}
      </span>
      <span
        className={cn(
          styles.metric,
          unreadImpactedCount > 0 ? styles.unreadImpacted : styles.empty
        )}
        title={`${unreadImpactedCount} impacted files not read by the agent`}>
        <TriangleAlert className="size-2.5" aria-hidden="true" />
        {unreadImpactedCount}
      </span>
    </span>
  );
}
