import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { mockBootstrap, mockSnapshot, nextMockWatchEvent } from "@/lib/mockVisualization";
import type {
  SessionVisualizationSnapshot,
  SessionWatchEventPayload,
  VisualAgentEvent,
  VisualAgentEventKind,
  VisualPhase,
  VisualStyle,
  VisualizerBootstrap,
} from "@/types/visualization";

type TauriWindow = Window & {
  __TAURI_INTERNALS__?: unknown;
};

export type VisualAgentEventDraft = {
  phase: VisualPhase;
  kind: VisualAgentEventKind;
  label: string;
  visualTargetHints?: string[];
  visualStyle?: VisualStyle;
  summary?: string;
  relatedHints?: string[];
  metadata?: Record<string, unknown>;
};

export function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in (window as TauriWindow);
}

export async function getVisualizerBootstrap(): Promise<VisualizerBootstrap> {
  if (!isTauriRuntime()) {
    return mockBootstrap;
  }

  return invoke<VisualizerBootstrap>("get_visualizer_bootstrap");
}

export async function loadVisualizationSnapshot(
  runtimeHome?: string
): Promise<SessionVisualizationSnapshot> {
  if (!isTauriRuntime()) {
    return mockSnapshot();
  }

  return invoke<SessionVisualizationSnapshot>("load_current_codex_visualization", {
    runtimeHome,
  });
}

export async function recordVisualAgentEvent(
  event: VisualAgentEventDraft
): Promise<VisualAgentEvent> {
  if (!isTauriRuntime()) {
    return {
      id: `browser-preview:${Date.now()}`,
      threadId: mockSnapshot().activeSessionId,
      turnId: undefined,
      phase: event.phase,
      kind: event.kind,
      label: event.label,
      visualTargetHints: event.visualTargetHints ?? [],
      visualStyle: event.visualStyle,
      summary: event.summary,
      relatedHints: event.relatedHints ?? [],
      metadata: event.metadata ?? {},
      timestampMs: Date.now(),
    };
  }

  return invoke<VisualAgentEvent>("record_visual_agent_event", { event });
}

export async function startRuntimeWatch(runtimeHome: string) {
  if (!isTauriRuntime()) {
    return {
      watchId: "browser-preview-watch",
      runtimeHome,
      watchTargets: mockSnapshot().watchPlan?.watchTargets ?? [],
    };
  }

  return invoke("start_codex_session_watch", { runtimeHome });
}

export function subscribeWatchEvents(
  onEvent: (event: SessionWatchEventPayload) => void
): Promise<() => void> {
  if (!isTauriRuntime()) {
    const intervalId = window.setInterval(() => onEvent(nextMockWatchEvent()), 9000);
    return Promise.resolve(() => window.clearInterval(intervalId));
  }

  return listen<SessionWatchEventPayload>("codex-session-watch-event", (event) => {
    onEvent(event.payload);
  });
}
