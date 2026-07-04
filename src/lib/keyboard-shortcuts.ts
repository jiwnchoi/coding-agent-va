import { useEffect, useRef } from "react";

type ShortcutHandler = (event: KeyboardEvent) => void;

export type KeyboardShortcutConfig = {
  id: string;
  key: string;
  ctrlKey?: boolean;
  shiftKey?: boolean;
  altKey?: boolean;
  metaKey?: boolean;
  preventDefault?: boolean;
  allowInEditable?: boolean;
  action: string;
};

export type KeyboardShortcut = {
  id: string;
  key: string;
  ctrlKey?: boolean;
  shiftKey?: boolean;
  altKey?: boolean;
  metaKey?: boolean;
  preventDefault?: boolean;
  allowInEditable?: boolean;
  handler: ShortcutHandler;
};

function isEditableTarget(target: EventTarget | null) {
  return target instanceof HTMLElement
    ? target.isContentEditable ||
        ["INPUT", "SELECT", "TEXTAREA"].includes(target.tagName) ||
        target.closest("[contenteditable='true']") !== null
    : false;
}

function matchesModifier(expected: boolean | undefined, actual: boolean) {
  return expected === undefined ? true : expected === actual;
}

function matchesShortcut(shortcut: KeyboardShortcut, event: KeyboardEvent) {
  return (
    shortcut.key.toLowerCase() === event.key.toLowerCase() &&
    matchesModifier(shortcut.ctrlKey, event.ctrlKey) &&
    matchesModifier(shortcut.shiftKey, event.shiftKey) &&
    matchesModifier(shortcut.altKey, event.altKey) &&
    matchesModifier(shortcut.metaKey, event.metaKey)
  );
}

export function useKeyboardShortcuts(shortcuts: KeyboardShortcut[]) {
  const shortcutsRef = useRef(shortcuts);

  shortcutsRef.current = shortcuts;

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      for (const shortcut of shortcutsRef.current) {
        if (!shortcut.allowInEditable && isEditableTarget(event.target)) {
          continue;
        }

        if (!matchesShortcut(shortcut, event)) {
          continue;
        }

        if (shortcut.preventDefault ?? true) {
          event.preventDefault();
        }

        shortcut.handler(event);
        return;
      }
    }

    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, []);
}
