import keyboardShortcuts from "@/features/session-dashboard/config/keyboard-shortcuts.json";

import type { AppSettings } from "./useAppSettings";

export function KeyboardSettings({
  settings,
  onChange,
}: {
  settings: AppSettings;
  onChange: (update: Partial<AppSettings>) => void;
}) {
  return (
    <div>
      <p className="text-muted-foreground mb-5 text-sm">
        Focus a shortcut field and press a new key combination. Press Delete to restore its default.
      </p>
      <div className="divide-border divide-y">
        {keyboardShortcuts.map((shortcut) => {
          const storedShortcut = settings.keyboardShortcuts[shortcut.id];
          return (
            <label
              className="flex items-center justify-between gap-4 py-3.5 text-sm"
              key={shortcut.id}>
              <span>
                {shortcut.action
                  .replace(/([A-Z])/g, " $1")
                  .replace(/^./, (character) => character.toUpperCase())}
              </span>
              <input
                aria-label={`Shortcut for ${shortcut.action}`}
                readOnly
                value={formatStoredShortcut(storedShortcut) || formatShortcut(shortcut)}
                onFocus={(event) => event.currentTarget.select()}
                onKeyDown={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  if (event.key === "Escape") {
                    event.currentTarget.blur();
                    return;
                  }
                  if (event.key === "Backspace" || event.key === "Delete") {
                    const nextShortcuts = { ...settings.keyboardShortcuts };
                    delete nextShortcuts[shortcut.id];
                    onChange({ keyboardShortcuts: nextShortcuts });
                    return;
                  }
                  if (["Alt", "Control", "Meta", "Shift"].includes(event.key)) return;
                  onChange({
                    keyboardShortcuts: {
                      ...settings.keyboardShortcuts,
                      [shortcut.id]: keyboardEventToStoredShortcut(event),
                    },
                  });
                }}
                className="border-border bg-muted focus:border-ring focus:ring-ring/30 h-8 w-28 rounded-md border px-2 text-center text-xs outline-none focus:ring-2"
              />
            </label>
          );
        })}
      </div>
    </div>
  );
}

function formatShortcut(shortcut: {
  key: string;
  altKey?: boolean;
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
}) {
  return `${shortcut.ctrlKey ? "⌃" : ""}${shortcut.altKey ? "⌥" : ""}${shortcut.shiftKey ? "⇧" : ""}${shortcut.metaKey ? "⌘" : ""}${shortcut.key === "Tab" ? "Tab" : shortcut.key.toUpperCase()}`;
}

function keyboardEventToStoredShortcut(event: React.KeyboardEvent) {
  return [
    event.ctrlKey ? "Control" : "",
    event.altKey ? "Alt" : "",
    event.shiftKey ? "Shift" : "",
    event.metaKey ? "Meta" : "",
    event.key,
  ]
    .filter(Boolean)
    .join("+");
}

function formatStoredShortcut(value: string | undefined) {
  if (!value) return "";
  const parts = value.split("+");
  const key = parts.pop() ?? "";
  return `${parts.includes("Control") ? "⌃" : ""}${parts.includes("Alt") ? "⌥" : ""}${parts.includes("Shift") ? "⇧" : ""}${parts.includes("Meta") ? "⌘" : ""}${key === "Tab" ? "Tab" : key.toUpperCase()}`;
}
