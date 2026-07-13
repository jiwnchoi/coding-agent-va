import type { ActivitySectionKey } from "@/features/session-dashboard/lib/session-watch";

import { buildActivityByPath, buildChildActivityCounts } from "./contextGraphActivity";
import { displayPathForNode, normalizeWorkspacePath } from "./contextGraphPaths";
import styles from "./ContextGraphView.module.css";
import { collectVisibleNodeIds, isVisibleGraphNode } from "./contextGraphVisibility";
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
type ArchitectureGraphIndex = {
  containsEdges: ArchitectureEdge[];
  fileNodeIdByPathKey: Map<string, string>;
  nodeById: Map<string, ArchitectureNode>;
  parentByChildId: Map<string, string>;
  visibleNodes: ArchitectureNode[];
  workspacePath: string;
};
const architectureGraphIndexes = new WeakMap<ArchitectureGraph, ArchitectureGraphIndex>();

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
  const architectureIndex = getArchitectureGraphIndex(architectureGraph, workspacePath);
  const { containsEdges, fileNodeIdByPathKey, nodeById, parentByChildId, visibleNodes } =
    architectureIndex;
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

  const visibleNodeIds = collectVisibleNodeIds({
    activeFilePaths,
    architectureGraph,
    fileNodeIdByPathKey,
    includeEntireWorkspace: options.includeEntireWorkspace,
    parentByChildId,
  });

  const directoriesWithVisibleFiles = new Set(
    containsEdges
      .filter((edge) => visibleNodeIds.has(edge.source) && visibleNodeIds.has(edge.target))
      .filter((edge) => nodeById.get(edge.source)?.kind === "directory")
      .filter((edge) => nodeById.get(edge.target)?.kind === "file")
      .map((edge) => edge.source)
  );

  const childActivityCountByNodeId = buildChildActivityCounts(
    visibleNodeIds,
    nodeById,
    activityByPath,
    fileNodeIdByPathKey,
    parentByChildId
  );

  const nodes = visibleNodes
    .filter((node) => visibleNodeIds.has(node.id))
    .map((node) =>
      toContextGraphNode({
        activities: activityByPath.get(displayPathForNode(node, workspacePath)) ?? [],
        childActivityCount: childActivityCountByNodeId.get(node.id) ?? 0,
        hasDirectFiles: directoriesWithVisibleFiles.has(node.id),
        isPinned: pinnedFilePaths.includes(displayPathForNode(node, workspacePath)),
        isSelected: selectedFilePath === displayPathForNode(node, workspacePath),
        node,
        workspacePath,
      })
    );

  const visibleContainsEdges = containsEdges
    .filter((edge) => visibleNodeIds.has(edge.source) && visibleNodeIds.has(edge.target))
    .filter((edge) => {
      const source = nodeById.get(edge.source);
      const target = nodeById.get(edge.target);

      return Boolean(source && target && isVisibleGraphNode(source) && isVisibleGraphNode(target));
    })
    .map(toContainsEdge);

  const visibleHierarchy = pruneEmptyDirectories(nodes, visibleContainsEdges);
  const collapsedHierarchy = collapseSingleDirectoryChains(
    visibleHierarchy.nodes,
    visibleHierarchy.edges
  );

  const impactEdgesById = new Map(
    options.fileActivity.impactedRelations
      .map((relation): ContextGraphEdge | null => {
        const changedFile = normalizeWorkspacePath(relation.changedFile, workspacePath);
        const impactedFile = normalizeWorkspacePath(relation.impactedFile, workspacePath);
        const source = fileNodeIdByPathKey.get(changedFile);
        const target = fileNodeIdByPathKey.get(impactedFile);

        if (!source || !target || !visibleNodeIds.has(source) || !visibleNodeIds.has(target)) {
          return null;
        }

        const isHighlighted = selectedFilePath === changedFile || selectedFilePath === impactedFile;
        const isRead = activityByPath.get(impactedFile)?.includes("read") ?? false;
        const edgeId = `impact:${source}->${target}`;

        return {
          id: edgeId,
          source,
          target,
          className: isHighlighted
            ? isRead
              ? styles.highlightedImpactEdge
              : styles.highlightedUnreadImpactEdge
            : isRead
              ? styles.impactEdge
              : styles.unreadImpactEdge,
          data: {
            kind: "impact",
            importSpecifier: relation.importSpecifier,
            isHighlighted,
          },
          type: "contextGraphImpactEdge",
          zIndex: 20,
        };
      })
      .filter((edge): edge is ContextGraphEdge => edge !== null)
      .map((edge) => [edge.id, edge] as const)
  );
  const impactEdges = [...impactEdgesById.values()];
  const nodeIds = new Set(collapsedHierarchy.nodes.map((node) => node.id));

  return {
    nodes: collapsedHierarchy.nodes,
    containsEdges: collapsedHierarchy.edges.filter(
      (edge) => nodeIds.has(edge.source) && nodeIds.has(edge.target)
    ),
    impactEdges: impactEdges.filter((edge) => nodeIds.has(edge.source) && nodeIds.has(edge.target)),
    activity: options.fileActivity,
    impactedRelations: options.fileActivity.impactedRelations,
  };
}

function getArchitectureGraphIndex(architectureGraph: ArchitectureGraph, workspacePath: string) {
  const cached = architectureGraphIndexes.get(architectureGraph);
  if (cached?.workspacePath === workspacePath) {
    return cached;
  }

  const containsEdges = architectureGraph.edges.filter((edge) => edge.kind === "contains");
  const fileNodeIdByPathKey = new Map<string, string>();
  const nodeById = new Map(architectureGraph.nodes.map((node) => [node.id, node]));
  const parentByChildId = new Map(containsEdges.map((edge) => [edge.target, edge.source]));
  const visibleNodes = architectureGraph.nodes.filter(isVisibleGraphNode);

  for (const node of visibleNodes) {
    if (node.kind !== "file" || !node.path) {
      continue;
    }
    fileNodeIdByPathKey.set(displayPathForNode(node, workspacePath), node.id);
    fileNodeIdByPathKey.set(normalizeWorkspacePath(node.path, workspacePath), node.id);
  }

  const index = {
    containsEdges,
    fileNodeIdByPathKey,
    nodeById,
    parentByChildId,
    visibleNodes,
    workspacePath,
  };
  architectureGraphIndexes.set(architectureGraph, index);
  return index;
}

function pruneEmptyDirectories(
  nodes: ContextGraphNode[],
  edges: ContextGraphEdge[]
): { nodes: ContextGraphNode[]; edges: ContextGraphEdge[] } {
  const retainedNodeIds = new Set(
    nodes.filter((node) => node.data.kind !== "directory").map((node) => node.id)
  );
  const parentIdsByChildId = new Map<string, string[]>();

  for (const edge of edges) {
    parentIdsByChildId.set(edge.target, [
      ...(parentIdsByChildId.get(edge.target) ?? []),
      edge.source,
    ]);
  }

  const pendingNodeIds = [...retainedNodeIds];
  while (pendingNodeIds.length > 0) {
    const childId = pendingNodeIds.pop();
    if (!childId) {
      continue;
    }

    for (const parentId of parentIdsByChildId.get(childId) ?? []) {
      if (retainedNodeIds.has(parentId)) {
        continue;
      }
      retainedNodeIds.add(parentId);
      pendingNodeIds.push(parentId);
    }
  }

  return {
    nodes: nodes.filter((node) => retainedNodeIds.has(node.id)),
    edges: edges.filter(
      (edge) => retainedNodeIds.has(edge.source) && retainedNodeIds.has(edge.target)
    ),
  };
}

function toContextGraphNode({
  activities,
  childActivityCount,
  hasDirectFiles,
  isPinned,
  isSelected,
  node,
  workspacePath,
}: {
  activities: ActivitySectionKey[];
  childActivityCount: number;
  hasDirectFiles: boolean;
  isPinned: boolean;
  isSelected: boolean;
  node: ArchitectureNode;
  workspacePath: string;
}): ContextGraphNode {
  return {
    id: node.id,
    position: { x: 0, y: 0 },
    type: CONTEXT_NODE_TYPE,
    className: isSelected ? styles.selectedNode : undefined,
    data: {
      activities,
      childActivityCount,
      displayPath: displayPathForNode(node, workspacePath),
      hasDirectFiles,
      isPinned,
      isSelected,
      kind: node.kind as ContextGraphNodeData["kind"],
      label: node.label,
      language: node.metadata?.language ?? "",
      path: node.path ?? "",
    },
  };
}

function collapseSingleDirectoryChains(
  nodes: ContextGraphNode[],
  edges: ContextGraphEdge[]
): { nodes: ContextGraphNode[]; edges: ContextGraphEdge[] } {
  const nodeById = new Map(nodes.map((node) => [node.id, node]));
  const childrenByParentId = new Map<string, string[]>();
  const parentIdByChildId = new Map<string, string>();

  for (const edge of edges) {
    childrenByParentId.set(edge.source, [
      ...(childrenByParentId.get(edge.source) ?? []),
      edge.target,
    ]);
    parentIdByChildId.set(edge.target, edge.source);
  }

  const removedIds = new Set<string>();

  for (const node of nodes) {
    if (node.data.kind !== "directory" || removedIds.has(node.id)) {
      continue;
    }

    const labels = [node.data.label];
    let tailId = node.id;
    let children = childrenByParentId.get(tailId) ?? [];

    while (children.length === 1) {
      const child = nodeById.get(children[0]);

      if (
        !child ||
        child.data.kind !== "directory" ||
        hasDirectActivityFile(child.id, childrenByParentId, nodeById)
      ) {
        break;
      }

      removedIds.add(child.id);
      labels.push(child.data.label);
      tailId = child.id;
      children = childrenByParentId.get(tailId) ?? [];
    }

    if (tailId !== node.id) {
      node.data = { ...node.data, label: labels.join("/") };
      childrenByParentId.set(node.id, children);
      for (const childId of children) {
        parentIdByChildId.set(childId, node.id);
      }
    }
  }

  const collapsedNodes = nodes.filter((node) => !removedIds.has(node.id));
  const collapsedNodeIds = new Set(collapsedNodes.map((node) => node.id));
  const collapsedEdges: ContextGraphEdge[] = [];

  for (const [childId, parentId] of parentIdByChildId) {
    if (!collapsedNodeIds.has(parentId) || !collapsedNodeIds.has(childId)) {
      continue;
    }

    collapsedEdges.push(
      toContainsEdge({
        id: `contains:${parentId}->${childId}`,
        kind: "contains",
        source: parentId,
        target: childId,
      })
    );
  }

  return { nodes: collapsedNodes, edges: collapsedEdges };
}

function hasDirectActivityFile(
  directoryId: string,
  childrenByParentId: Map<string, string[]>,
  nodeById: Map<string, ContextGraphNode>
) {
  return (childrenByParentId.get(directoryId) ?? []).some((childId) => {
    const child = nodeById.get(childId);
    return child?.data.kind === "file" && child.data.activities.length > 0;
  });
}

function toContainsEdge(edge: ArchitectureEdge): ContextGraphEdge {
  return {
    id: edge.id,
    source: edge.source,
    target: edge.target,
    className: styles.containsEdge,
    data: {
      kind: "contains",
      isHighlighted: false,
    },
    type: "contextGraphContainsEdge",
  };
}
