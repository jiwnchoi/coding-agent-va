import type { ActivitySectionKey } from "@/lib/session-watch";

import type {
  ArchitectureEdge,
  ArchitectureGraph,
  ArchitectureNode,
  ContextGraphBuildOptions,
  ContextGraphEdge,
  ContextGraphModel,
  ContextGraphNode,
  ContextGraphNodeData,
} from "./types";

const CONTEXT_NODE_TYPE = "contextGraphNode" as const;
const ACTIVITY_ORDER: ActivitySectionKey[] = ["impacted", "edited", "deleted", "read"];

export function buildContextGraph(options: ContextGraphBuildOptions): ContextGraphModel {
  const architectureGraph = options.architectureGraph;

  if (!architectureGraph || !options.workspacePath) {
    return {
      nodes: [],
      containsEdges: [],
      impactEdges: [],
      activity: options.fileActivity,
      impactedRelations: options.fileActivity.impactedRelations,
    };
  }

  const workspacePath = options.workspacePath;
  const nodeById = new Map(architectureGraph.nodes.map((node) => [node.id, node]));
  const fileNodeIdByPathKey = new Map<string, string>();
  const activityByPath = buildActivityByPath(options.fileActivity, workspacePath);
  const selectedFilePath = normalizeWorkspacePath(options.selectedFilePath, workspacePath);
  const pinnedFilePaths = options.pinnedFilePaths.map((filePath) =>
    normalizeWorkspacePath(filePath, workspacePath)
  );
  const activeFilePaths = new Set([
    ...activityByPath.keys(),
    ...pinnedFilePaths,
    ...options.fileActivity.impactedRelations.flatMap((relation) => [
      normalizeWorkspacePath(relation.changedFile, workspacePath),
      normalizeWorkspacePath(relation.impactedFile, workspacePath),
    ]),
  ]);

  for (const node of architectureGraph.nodes) {
    if (node.kind !== "file" || !node.path) {
      continue;
    }

    fileNodeIdByPathKey.set(displayPathForNode(node, workspacePath), node.id);
    fileNodeIdByPathKey.set(normalizeWorkspacePath(node.path, workspacePath), node.id);
  }

  const visibleNodeIds = collectVisibleNodeIds({
    activeFilePaths,
    architectureGraph,
    fileNodeIdByPathKey,
    includeEntireWorkspace: options.includeEntireWorkspace,
  });

  const childActivityCountByNodeId = buildChildActivityCounts(
    architectureGraph,
    visibleNodeIds,
    nodeById,
    workspacePath,
    activityByPath
  );

  const nodes = architectureGraph.nodes
    .filter((node) => isRenderableNode(node))
    .filter((node) => visibleNodeIds.has(node.id))
    .map((node) =>
      toContextGraphNode({
        activities: activityByPath.get(displayPathForNode(node, workspacePath)) ?? [],
        childActivityCount: childActivityCountByNodeId.get(node.id) ?? 0,
        isPinned: pinnedFilePaths.includes(displayPathForNode(node, workspacePath)),
        isSelected: selectedFilePath === displayPathForNode(node, workspacePath),
        node,
        workspacePath,
      })
    );

  const containsEdges = architectureGraph.edges
    .filter((edge) => edge.kind === "contains")
    .filter((edge) => visibleNodeIds.has(edge.source) && visibleNodeIds.has(edge.target))
    .map(toContainsEdge);

  const impactEdges = options.fileActivity.impactedRelations
    .map((relation): ContextGraphEdge | null => {
      const changedFile = normalizeWorkspacePath(relation.changedFile, workspacePath);
      const impactedFile = normalizeWorkspacePath(relation.impactedFile, workspacePath);
      const source = fileNodeIdByPathKey.get(changedFile);
      const target = fileNodeIdByPathKey.get(impactedFile);

      if (!source || !target || !visibleNodeIds.has(source) || !visibleNodeIds.has(target)) {
        return null;
      }

      const isHighlighted = selectedFilePath === changedFile || selectedFilePath === impactedFile;

      return {
        id: `impact:${source}->${target}:${relation.importSpecifier}`,
        source,
        target,
        animated: isHighlighted,
        className: isHighlighted
          ? "context-graph-impact-edge-highlighted"
          : "context-graph-impact-edge",
        data: {
          kind: "impact",
          importSpecifier: relation.importSpecifier,
          isHighlighted,
        },
        label: relation.importSpecifier,
        markerEnd: {
          type: "arrowclosed",
        },
        type: "smoothstep",
      };
    })
    .filter((edge): edge is ContextGraphEdge => edge !== null);

  return {
    nodes,
    containsEdges,
    impactEdges,
    activity: options.fileActivity,
    impactedRelations: options.fileActivity.impactedRelations,
  };
}

function buildActivityByPath(
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

function collectVisibleNodeIds({
  activeFilePaths,
  architectureGraph,
  fileNodeIdByPathKey,
  includeEntireWorkspace,
}: {
  activeFilePaths: Set<string>;
  architectureGraph: ArchitectureGraph;
  fileNodeIdByPathKey: Map<string, string>;
  includeEntireWorkspace: boolean;
}) {
  const visibleNodeIds = new Set<string>();
  const parentByChildId = new Map<string, string>();

  for (const edge of architectureGraph.edges) {
    if (edge.kind === "contains") {
      parentByChildId.set(edge.target, edge.source);
    }
  }

  if (includeEntireWorkspace) {
    for (const node of architectureGraph.nodes) {
      if (isRenderableNode(node)) {
        visibleNodeIds.add(node.id);
      }
    }
    return visibleNodeIds;
  }

  for (const filePath of activeFilePaths) {
    const fileNodeId = fileNodeIdByPathKey.get(filePath);

    if (!fileNodeId) {
      continue;
    }

    let currentNodeId: string | undefined = fileNodeId;
    while (currentNodeId) {
      visibleNodeIds.add(currentNodeId);
      currentNodeId = parentByChildId.get(currentNodeId);
    }
  }

  if (visibleNodeIds.size === 0) {
    for (const node of architectureGraph.nodes) {
      if (node.kind === "repo") {
        visibleNodeIds.add(node.id);
      }
    }
  }

  return visibleNodeIds;
}

function buildChildActivityCounts(
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

function toContextGraphNode({
  activities,
  childActivityCount,
  isPinned,
  isSelected,
  node,
  workspacePath,
}: {
  activities: ActivitySectionKey[];
  childActivityCount: number;
  isPinned: boolean;
  isSelected: boolean;
  node: ArchitectureNode;
  workspacePath: string;
}): ContextGraphNode {
  return {
    id: node.id,
    position: { x: 0, y: 0 },
    type: CONTEXT_NODE_TYPE,
    className: isSelected ? "context-graph-node-selected" : undefined,
    data: {
      activities,
      childActivityCount,
      displayPath: displayPathForNode(node, workspacePath),
      isPinned,
      isSelected,
      kind: node.kind as ContextGraphNodeData["kind"],
      label: node.label,
      path: node.path ?? "",
    },
  };
}

function toContainsEdge(edge: ArchitectureEdge): ContextGraphEdge {
  return {
    id: edge.id,
    source: edge.source,
    target: edge.target,
    className: "context-graph-contains-edge",
    data: {
      kind: "contains",
      isHighlighted: false,
    },
    markerEnd: {
      type: "arrowclosed",
    },
    type: "smoothstep",
  };
}

function isRenderableNode(node: ArchitectureNode) {
  return node.kind === "repo" || node.kind === "directory" || node.kind === "file";
}

function displayPathForNode(node: ArchitectureNode, workspacePath: string) {
  if (!node.path) {
    return node.label;
  }

  const normalizedWorkspacePath = workspacePath.replace(/\/+$/, "");
  const normalizedNodePath = node.path.replace(/\/+$/, "");
  const prefix = `${normalizedWorkspacePath}/`;

  if (normalizedNodePath === normalizedWorkspacePath) {
    return ".";
  }

  if (normalizedNodePath.startsWith(prefix)) {
    return normalizedNodePath.slice(prefix.length);
  }

  return node.path;
}

function normalizeWorkspacePath(filePath: string, workspacePath: string) {
  if (!filePath) {
    return filePath;
  }

  const normalizedWorkspacePath = normalizeSlashes(workspacePath).replace(/\/+$/, "");
  const normalizedFilePath = normalizeSlashes(filePath).replace(/\/+$/, "");
  const prefix = `${normalizedWorkspacePath}/`;

  if (normalizedFilePath === normalizedWorkspacePath) {
    return ".";
  }

  if (normalizedFilePath.startsWith(prefix)) {
    return normalizedFilePath.slice(prefix.length);
  }

  return normalizedFilePath.replace(/^\.\//, "");
}

function normalizeSlashes(path: string) {
  return path.replace(/\\/g, "/");
}
