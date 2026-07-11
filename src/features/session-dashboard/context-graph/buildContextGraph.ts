import type { ActivitySectionKey } from "@/features/session-dashboard/lib/session-watch";

import { buildActivityByPath, buildChildActivityCounts } from "./contextGraphActivity";
import { displayPathForNode, normalizeWorkspacePath } from "./contextGraphPaths";
import styles from "./ContextGraphView.module.css";
import {
  collectVisibleNodeIds,
  expandHiddenRootChildren,
  isVisibleGraphNode,
} from "./contextGraphVisibility";
import type {
  ArchitectureEdge,
  ArchitectureNode,
  ContextGraphBuildOptions,
  ContextGraphEdge,
  ContextGraphModel,
  ContextGraphNode,
  ContextGraphNodeData,
} from "./types";

const CONTEXT_NODE_TYPE = "contextGraphNode" as const;
const IMPACT_EDGE_COLOR = "#ff7f0e";
const IMPACT_EDGE_MARKER_SIZE = 8;

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
  expandHiddenRootChildren(architectureGraph, visibleNodeIds);

  const childActivityCountByNodeId = buildChildActivityCounts(
    architectureGraph,
    visibleNodeIds,
    nodeById,
    workspacePath,
    activityByPath
  );

  const nodes = architectureGraph.nodes
    .filter((node) => isVisibleGraphNode(node))
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
    .filter((edge) => {
      const source = nodeById.get(edge.source);
      const target = nodeById.get(edge.target);

      return Boolean(source && target && isVisibleGraphNode(source) && isVisibleGraphNode(target));
    })
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

      const edgeId = `impact:${source}->${target}:${relation.importSpecifier}`;

      return {
        id: edgeId,
        source,
        target,
        animated: isHighlighted,
        className: isHighlighted ? styles.highlightedImpactEdge : styles.impactEdge,
        data: {
          kind: "impact",
          importSpecifier: relation.importSpecifier,
          isHighlighted,
        },
        markerEnd: {
          color: IMPACT_EDGE_COLOR,
          height: IMPACT_EDGE_MARKER_SIZE,
          strokeWidth: 1.5,
          type: "arrowclosed",
          width: IMPACT_EDGE_MARKER_SIZE,
        },
        type: "contextGraphImpactEdge",
        zIndex: 20,
      };
    })
    .filter((edge): edge is ContextGraphEdge => edge !== null);
  const nodeIds = new Set(nodes.map((node) => node.id));

  return {
    nodes,
    containsEdges: containsEdges.filter(
      (edge) => nodeIds.has(edge.source) && nodeIds.has(edge.target)
    ),
    impactEdges: impactEdges.filter((edge) => nodeIds.has(edge.source) && nodeIds.has(edge.target)),
    activity: options.fileActivity,
    impactedRelations: options.fileActivity.impactedRelations,
  };
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
    className: isSelected ? styles.selectedNode : undefined,
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
    className: styles.containsEdge,
    data: {
      kind: "contains",
      isHighlighted: false,
    },
    type: "contextGraphContainsEdge",
  };
}
