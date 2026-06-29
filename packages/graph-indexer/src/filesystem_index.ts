import type { ArchitectureGraph } from "./graph_model";

export function createWorkspaceGraph(workspacePath: string): ArchitectureGraph {
  return {
    nodes: [
      {
        id: "workspace",
        kind: "repo",
        label: workspacePath.split("/").filter(Boolean).at(-1) ?? workspacePath,
        path: workspacePath,
      },
      {
        id: "app",
        kind: "directory",
        label: "app",
        path: `${workspacePath}/app`,
      },
      {
        id: "packages",
        kind: "directory",
        label: "packages",
        path: `${workspacePath}/packages`,
      },
    ],
    edges: [
      { id: "workspace-app", source: "workspace", target: "app", label: "contains" },
      { id: "workspace-packages", source: "workspace", target: "packages", label: "contains" },
    ],
  };
}
