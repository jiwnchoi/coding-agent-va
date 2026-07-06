import type { ActivitySectionKey } from "@/features/session-dashboard/lib/session-watch";

import { displayPathForNode, normalizeWorkspacePath } from "./contextGraphPaths";
import type { ArchitectureGraph, ArchitectureNode, ContextGraphBuildOptions } from "./types";

const ACTIVITY_ORDER: ActivitySectionKey[] = ["impacted", "edited", "deleted", "read"];

export function buildActivityByPath(
  fileActivity: ContextGraphBuildOptions["fileActivity"],
  workspacePath: string
) {
  const activityByPath = new Map<string, ActivitySectionKey[]>();

  addActivity(activityByPath, fileActivity.readFiles, "read", workspacePath);
  addActivity(activityByPath, fileActivity.editedFiles, "edited", workspacePath);
  addActivity(activityByPath, fileActivity.impactedFiles, "impacted", workspacePath);
  addActivity(activityByPath, fileActivity.deletedFiles, "deleted", workspacePath);

  for (const [filePath, activities] of activityByPath) {
    activities.sort((left, right) => ACTIVITY_ORDER.indexOf(left) - ACTIVITY_ORDER.indexOf(right));
    activityByPath.set(filePath, [...new Set(activities)]);
  }

  return activityByPath;
}

export function buildChildActivityCounts(
  architectureGraph: ArchitectureGraph,
  visibleNodeIds: Set<string>,
  nodeById: Map<string, ArchitectureNode>,
  workspacePath: string,
  activityByPath: Map<string, ActivitySectionKey[]>
) {
  const childActivityCountByNodeId = new Map<string, number>();
  const parentByChildId = new Map<string, string>();

  for (const edge of architectureGraph.edges) {
    if (edge.kind === "contains") {
      parentByChildId.set(edge.target, edge.source);
    }
  }

  for (const [filePath, activities] of activityByPath) {
    if (activities.length === 0) {
      continue;
    }

    const fileNode = architectureGraph.nodes.find(
      (node) => node.kind === "file" && displayPathForNode(node, workspacePath) === filePath
    );
    let currentNodeId = fileNode?.id;

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
  workspacePath: string
) {
  for (const filePath of filePaths) {
    const pathKey = normalizeWorkspacePath(filePath, workspacePath);
    const activities = activityByPath.get(pathKey) ?? [];
    activities.push(activity);
    activityByPath.set(pathKey, activities);
  }
}
