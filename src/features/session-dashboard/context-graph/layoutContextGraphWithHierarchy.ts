import { NODE_HEIGHT, NODE_WIDTH } from "./layoutConstants";
import { assignContainsEdgeHandles, assignImpactEdgeHandles } from "./layoutHandles";
import { distributeImpactEdgeTargets } from "./layoutImpactLanes";
import type { ContextGraphModel, ContextGraphNode } from "./types";

const FOLDER_HEADER_HEIGHT = 30;
const FOLDER_PADDING = 14;
const FILE_GAP = 12;
const FOLDER_HORIZONTAL_GAP = 52;
const FOLDER_VERTICAL_GAP = 24;
const ROOT_GAP = 36;

type NodeSize = { width: number; height: number };

export function layoutContextGraphWithHierarchy(model: ContextGraphModel) {
  if (model.nodes.length === 0) {
    return model;
  }

  const nodeById = new Map(model.nodes.map((node) => [node.id, node]));
  const filesByFolderId = new Map<string, ContextGraphNode[]>();
  const foldersByParentId = new Map<string, ContextGraphNode[]>();
  const childIds = new Set<string>();

  for (const edge of model.containsEdges) {
    const parent = nodeById.get(edge.source);
    const child = nodeById.get(edge.target);

    if (!parent || !child || parent.data.kind !== "directory") {
      continue;
    }

    const targetMap = child.data.kind === "file" ? filesByFolderId : foldersByParentId;
    targetMap.set(parent.id, [...(targetMap.get(parent.id) ?? []), child]);
    childIds.add(child.id);
  }

  for (const children of [...filesByFolderId.values(), ...foldersByParentId.values()]) {
    children.sort(compareNodes);
  }

  const sizeById = new Map<string, NodeSize>();
  for (const node of model.nodes) {
    if (node.data.kind === "file") {
      sizeById.set(node.id, { width: NODE_WIDTH, height: NODE_HEIGHT });
      continue;
    }

    const fileCount = filesByFolderId.get(node.id)?.length ?? 0;
    sizeById.set(node.id, {
      width: NODE_WIDTH + FOLDER_PADDING * 2,
      height:
        FOLDER_HEADER_HEIGHT +
        FOLDER_PADDING +
        fileCount * NODE_HEIGHT +
        Math.max(0, fileCount - 1) * FILE_GAP,
    });
  }

  const subtreeHeightById = new Map<string, number>();
  const measureSubtree = (folder: ContextGraphNode): number => {
    const folderHeight = sizeById.get(folder.id)?.height ?? NODE_HEIGHT;
    const childHeights = (foldersByParentId.get(folder.id) ?? []).map(measureSubtree);
    const childrenHeight =
      childHeights.reduce((height, childHeight) => height + childHeight, 0) +
      Math.max(0, childHeights.length - 1) * FOLDER_VERTICAL_GAP;
    const subtreeHeight = Math.max(folderHeight, childrenHeight);
    subtreeHeightById.set(folder.id, subtreeHeight);
    return subtreeHeight;
  };

  const roots = model.nodes.filter((node) => !childIds.has(node.id)).sort(compareNodes);
  roots.filter(isFolder).forEach(measureSubtree);

  const positionedNodes: ContextGraphNode[] = [];
  const positionFolder = (folder: ContextGraphNode, x: number, subtreeY: number) => {
    const size = sizeById.get(folder.id) ?? { width: NODE_WIDTH, height: NODE_HEIGHT };
    const subtreeHeight = subtreeHeightById.get(folder.id) ?? size.height;
    const folderY = subtreeY + (subtreeHeight - size.height) / 2;

    positionedNodes.push(positionedNode(folder, x, folderY, size, 0));

    let fileY = folderY + FOLDER_HEADER_HEIGHT;
    for (const file of filesByFolderId.get(folder.id) ?? []) {
      const fileSize = sizeById.get(file.id) ?? { width: NODE_WIDTH, height: NODE_HEIGHT };
      positionedNodes.push(positionedNode(file, x + FOLDER_PADDING, fileY, fileSize, 10));
      fileY += fileSize.height + FILE_GAP;
    }

    let childY = subtreeY;
    for (const childFolder of foldersByParentId.get(folder.id) ?? []) {
      positionFolder(childFolder, x + size.width + FOLDER_HORIZONTAL_GAP, childY);
      childY += (subtreeHeightById.get(childFolder.id) ?? NODE_HEIGHT) + FOLDER_VERTICAL_GAP;
    }
  };

  let rootY = 0;
  for (const root of roots) {
    if (isFolder(root)) {
      positionFolder(root, 0, rootY);
      rootY += (subtreeHeightById.get(root.id) ?? NODE_HEIGHT) + ROOT_GAP;
      continue;
    }

    const size = sizeById.get(root.id) ?? { width: NODE_WIDTH, height: NODE_HEIGHT };
    positionedNodes.push(positionedNode(root, 0, rootY, size, 10));
    rootY += size.height + ROOT_GAP;
  }

  const positionById = new Map(positionedNodes.map((node) => [node.id, node.position]));
  const fileNodes = positionedNodes.filter((node) => node.data.kind === "file");

  return {
    ...model,
    nodes: positionedNodes,
    containsEdges: model.containsEdges.map(assignContainsEdgeHandles),
    impactEdges: distributeImpactEdgeTargets(
      model.impactEdges.map((edge) => assignImpactEdgeHandles(edge, positionById, fileNodes)),
      positionById
    ),
  };
}

function positionedNode(
  node: ContextGraphNode,
  x: number,
  y: number,
  size: NodeSize,
  zIndex: number
): ContextGraphNode {
  return {
    ...node,
    position: { x, y },
    style: { height: size.height, width: size.width },
    zIndex,
  };
}

function isFolder(node: ContextGraphNode) {
  return node.data.kind === "directory";
}

function compareNodes(left: ContextGraphNode, right: ContextGraphNode) {
  if (left.data.kind !== right.data.kind) {
    return left.data.kind === "directory" ? -1 : 1;
  }

  return left.data.label.localeCompare(right.data.label, undefined, {
    numeric: true,
    sensitivity: "base",
  });
}
