import { useSyncExternalStore } from "react";

import type { MonacoTheme } from "@/features/settings";
import { SHIKI_DARK_THEME, SHIKI_LIGHT_THEME } from "@/shared/lib/editor/monaco-shiki";

function subscribe(callback: () => void) {
  const observer = new MutationObserver(callback);
  observer.observe(document.documentElement, { attributeFilter: ["class"], attributes: true });
  return () => observer.disconnect();
}

export function useEditorTheme(
  theme: MonacoTheme
): typeof SHIKI_LIGHT_THEME | typeof SHIKI_DARK_THEME {
  const appTheme = useSyncExternalStore(
    subscribe,
    () => (document.documentElement.classList.contains("dark") ? "dark" : "light"),
    () => "light"
  );

  if (theme === "light") return SHIKI_LIGHT_THEME;
  if (theme === "dark") return SHIKI_DARK_THEME;
  return appTheme === "dark" ? SHIKI_DARK_THEME : SHIKI_LIGHT_THEME;
}
