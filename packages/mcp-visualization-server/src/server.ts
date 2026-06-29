import { createPlaceholderEvent } from "./placeholder_tool";
import type { PlaceholderVisualizationInput, VisualizationEvent } from "./placeholder_tool";

export type VisualizationForwarder = (event: VisualizationEvent) => Promise<void>;

export async function handlePlaceholderVisualization(
  input: PlaceholderVisualizationInput,
  forward: VisualizationForwarder,
): Promise<VisualizationEvent> {
  const event = createPlaceholderEvent(input);
  await forward(event);
  return event;
}
