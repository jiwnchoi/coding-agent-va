import { useSyncExternalStore } from "react";

import { SHIKI_DARK_THEME, SHIKI_LIGHT_THEME } from "@/shared/lib/editor/monaco-shiki";

function subscribe(callback: () => void) {
  const darkModeMediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
  darkModeMediaQuery.addEventListener("change", callback);

  return () => {
    darkModeMediaQuery.removeEventListener("change", callback);
  };
}

function getSnapshot() {
  return document.documentElement.classList.contains("dark") ? SHIKI_DARK_THEME : SHIKI_LIGHT_THEME;
}

export function useEditorTheme(): typeof SHIKI_LIGHT_THEME | typeof SHIKI_DARK_THEME {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}
