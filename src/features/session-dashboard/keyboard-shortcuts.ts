import type { RefObject } from "react";

import type { KeyboardShortcut, KeyboardShortcutConfig } from "@/shared/hooks/useKeyboardShortcuts";

import keyboardShortcutConfig from "./config/keyboard-shortcuts.json";
import { selectSessionByTabNumber } from "./session-tabs";

const APP_SHORTCUTS = keyboardShortcutConfig as KeyboardShortcutConfig[];

export function buildTabNumberShortcutActions(
  handleSelectSession: (sessionId: string) => void,
  openSessionIdsRef: RefObject<string[]>
): Record<string, KeyboardShortcut["handler"]> {
  return Object.fromEntries(
    Array.from({ length: 10 }, (_, index) => {
      const tabNumber = (index + 1) % 10;
      const actionName = `selectSessionTab${tabNumber}`;

      return [
        actionName,
        () => {
          const sessionId = selectSessionByTabNumber(openSessionIdsRef.current, tabNumber);

          if (!sessionId) {
            return;
          }

          handleSelectSession(sessionId);
        },
      ];
    })
  );
}

export function buildShortcuts(
  shortcutActions: Record<string, KeyboardShortcut["handler"]>
): KeyboardShortcut[] {
  return APP_SHORTCUTS.flatMap((shortcut) => {
    const handler = shortcutActions[shortcut.action];

    if (!handler) {
      return [];
    }

    return [
      {
        ...shortcut,
        handler,
      },
    ];
  });
}
