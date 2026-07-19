import { normalizeWorkspacePath } from "@/features/session-dashboard/context-graph/contextGraphPaths";

export type SequencePoint = { x: number; y: number };

export function connectionEndpoints(from: SequencePoint, to: SequencePoint, radius: number) {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const distance = Math.hypot(dx, dy);
  if (distance === 0) return { from, to };
  const offsetX = (dx / distance) * radius;
  const offsetY = (dy / distance) * radius;
  return {
    from: { x: from.x + offsetX, y: from.y + offsetY },
    to: { x: to.x - offsetX, y: to.y - offsetY },
  };
}

export function isHoveredFile(
  filePath: string,
  hoveredFilePaths: string[] | null,
  workspacePath: string | null
) {
  if (hoveredFilePaths === null) return true;
  const normalizedPath = normalizeWorkspacePath(filePath, workspacePath ?? "");
  return hoveredFilePaths.some(
    (hoveredFilePath) =>
      normalizeWorkspacePath(hoveredFilePath, workspacePath ?? "") === normalizedPath
  );
}
