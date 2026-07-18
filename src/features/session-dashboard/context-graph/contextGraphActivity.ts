import type { ActivitySectionKey } from "@/features/session-dashboard/lib/session-watch";

import { normalizeWorkspacePath } from "./contextGraphPaths";
import type { ArchitectureNode, ContextGraphBuildOptions } from "./types";

const ACTIVITY_ORDER: ActivitySectionKey[] = ["impacted", "edited", "deleted", "read"];

export function buildActivityByPath(
  fileActivity: ContextGraphBuildOptions["fileActivity"],
  workspacePath: string,
  showReadFiles: boolean
) {
  const activityByPath = new Map<string, ActivitySectionKey[]>();

  addActivity(activityByPath, fileActivity.editedFiles, "edited", workspacePath);
  addActivity(activityByPath, fileActivity.impactedFiles, "impacted", workspacePath);
  addActivity(activityByPath, fileActivity.deletedFiles, "deleted", workspacePath);

  const impactedFiles = new Set(
    fileActivity.impactedFiles.map((filePath) => normalizeWorkspacePath(filePath, workspacePath))
  );
  addActivity(
    activityByPath,
    fileActivity.readFiles,
    "read",
    workspacePath,
    (filePath) =>
      showReadFiles || impactedFiles.has(normalizeWorkspacePath(filePath, workspacePath))
  );

  for (const [filePath, activities] of activityByPath) {
    activities.sort((left, right) => ACTIVITY_ORDER.indexOf(left) - ACTIVITY_ORDER.indexOf(right));
    activityByPath.set(filePath, [...new Set(activities)]);
  }

  return activityByPath;
}

export function buildChildActivityCounts(
  visibleNodeIds: Set<string>,
  nodeById: Map<string, ArchitectureNode>,
  activityByPath: Map<string, ActivitySectionKey[]>,
  fileNodeIdByPathKey: Map<string, string>,
  parentByChildId: Map<string, string>
) {
  const childActivityCountByNodeId = new Map<string, number>();

  for (const [filePath, activities] of activityByPath) {
    if (activities.length === 0) {
      continue;
    }

    let currentNodeId = fileNodeIdByPathKey.get(filePath);

    while (currentNodeId) {
      if (visibleNodeIds.has(currentNodeId)) {
        childActivityCountByNodeId.set(
          currentNodeId,
          (childActivityCountByNodeId.get(currentNodeId) ?? 0) + 1
        );
      }
      currentNodeId = parentByChildId.get(currentNodeId);
    }
  }

  for (const nodeId of childActivityCountByNodeId.keys()) {
    if (!nodeById.has(nodeId)) {
      childActivityCountByNodeId.delete(nodeId);
    }
  }

  return childActivityCountByNodeId;
}

function addActivity(
  activityByPath: Map<string, ActivitySectionKey[]>,
  filePaths: string[],
  activity: ActivitySectionKey,
  workspacePath: string,
  shouldInclude: (filePath: string) => boolean = () => true
) {
  for (const filePath of filePaths) {
    if (!shouldInclude(filePath)) {
      continue;
    }
    const pathKey = normalizeWorkspacePath(filePath, workspacePath);
    const activities = activityByPath.get(pathKey) ?? [];
    activities.push(activity);
    activityByPath.set(pathKey, activities);
  }
}
