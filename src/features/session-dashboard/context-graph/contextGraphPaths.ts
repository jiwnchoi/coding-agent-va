import type { ArchitectureNode } from "./types";

export function displayPathForNode(node: ArchitectureNode, workspacePath: string) {
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

export function normalizeWorkspacePath(filePath: string, workspacePath: string) {
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
