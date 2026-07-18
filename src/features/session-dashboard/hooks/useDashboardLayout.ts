import { number, object, safeParse } from "valibot";
import { create } from "zustand";
import { persist } from "zustand/middleware";

import {
  DEFAULT_DIFF_PANEL_WIDTH,
  DEFAULT_PROMPT_PANEL_WIDTH,
} from "@/features/session-dashboard/constants";

const dashboardLayoutSchema = object({
  diffPanelWidth: number(),
  promptPanelWidth: number(),
});

type DashboardLayoutState = {
  diffPanelWidth: number;
  promptPanelWidth: number;
  setDiffPanelWidth: (width: number) => void;
  setPromptPanelWidth: (width: number) => void;
};

export const useDashboardLayout = create<DashboardLayoutState>()(
  persist(
    (set) => ({
      diffPanelWidth: DEFAULT_DIFF_PANEL_WIDTH,
      promptPanelWidth: DEFAULT_PROMPT_PANEL_WIDTH,
      setDiffPanelWidth: (diffPanelWidth) => set({ diffPanelWidth }),
      setPromptPanelWidth: (promptPanelWidth) => set({ promptPanelWidth }),
    }),
    {
      name: "session-dashboard-layout",
      partialize: ({ diffPanelWidth, promptPanelWidth }) => ({
        diffPanelWidth,
        promptPanelWidth,
      }),
      merge: (persistedState, currentState) => {
        const result = safeParse(dashboardLayoutSchema, persistedState);

        return result.success ? { ...currentState, ...result.output } : currentState;
      },
    }
  )
);
