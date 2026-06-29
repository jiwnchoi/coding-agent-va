export type ArchitectureNode = {
  id: string;
  kind: "repo" | "directory" | "file" | "symbol" | "external" | "plan";
  label: string;
  path?: string;
  metadata?: Record<string, unknown>;
};

export type ArchitectureEdge = {
  id: string;
  source: string;
  target: string;
  label?: string;
};

export type ArchitectureGraph = {
  nodes: ArchitectureNode[];
  edges: ArchitectureEdge[];
};
