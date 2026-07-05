import {
  DEFAULT_DIFF_PANEL_WIDTH,
  MAX_DIFF_PANEL_WIDTH,
  MIN_DIFF_PANEL_WIDTH,
} from "@/features/session-dashboard/constants";

export function clampNumber(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

export function getMaxDiffPanelWidth(viewportWidth: number) {
  if (viewportWidth <= 0) {
    return DEFAULT_DIFF_PANEL_WIDTH;
  }

  return Math.max(MIN_DIFF_PANEL_WIDTH, Math.min(MAX_DIFF_PANEL_WIDTH, viewportWidth - 640));
}
