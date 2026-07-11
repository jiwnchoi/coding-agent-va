export const NODE_WIDTH = 156;
export const NODE_HEIGHT = 44;
export const HORIZONTAL_GAP = 28;
export const VERTICAL_GAP = 16;
export const IMPACT_EDGE_LANE_OFFSET_STEP = 16;
export const EDGE_COLLISION_PADDING = 8;
export const EDGE_COLLISION_PENALTY = 10_000;
export const HANDLE_PREFIXES = {
  source: "source",
  target: "target",
} as const;
export const EDGE_SIDES: EdgeSide[] = ["top", "right", "bottom", "left"];

export type EdgeSide = "top" | "right" | "bottom" | "left";
