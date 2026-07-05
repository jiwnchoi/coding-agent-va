import { getCurrentWindow } from "@tauri-apps/api/window";
import type { MouseEvent } from "react";

function isWindowControlExcluded(target: EventTarget | null) {
  return target instanceof Element
    ? target.closest(
        [
          "[data-window-control-exclusion]",
          "a",
          "button",
          "input",
          "select",
          "textarea",
          "[role='button']",
          "[role='menu']",
          "[role='menuitem']",
        ].join(",")
      ) !== null
    : false;
}

export function handleTitlebarMouseDown(event: MouseEvent<HTMLElement>) {
  if (event.button !== 0 || isWindowControlExcluded(event.target)) {
    return;
  }

  try {
    const appWindow = getCurrentWindow();

    if (event.detail === 2) {
      void appWindow.toggleMaximize();
      return;
    }

    if (event.detail === 1) {
      void appWindow.startDragging();
    }
  } catch {
    // Ignore titlebar behavior in plain browser mode.
  }
}
