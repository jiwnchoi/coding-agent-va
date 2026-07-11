import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";

import type { AppFont, AppSettings, AppTheme, MonacoTheme } from "@/shared/lib/generated/bindings";

export type { AppFont, AppSettings, AppTheme, MonacoTheme };

export const DEFAULT_APP_SETTINGS: AppSettings = {
  theme: "system",
  font: "geist",
  monacoTheme: "system",
  hideCommittedFiles: true,
  keyboardShortcuts: {},
  runtimeHomes: { claude: "", codex: "", pi: "" },
};

function applyTheme(theme: AppTheme) {
  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  document.documentElement.classList.toggle(
    "dark",
    theme === "dark" || (theme === "system" && prefersDark)
  );
}

function applyFont(font: AppFont) {
  document.documentElement.dataset.font = font;
}

export function useAppSettings() {
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_APP_SETTINGS);
  const [settingsLoaded, setSettingsLoaded] = useState(false);
  const [settingsError, setSettingsError] = useState("");

  useEffect(() => {
    let disposed = false;

    async function loadSettings() {
      try {
        const loadedSettings = await invoke<AppSettings>("load_app_settings");
        if (!disposed) {
          setSettings(loadedSettings);
          setSettingsLoaded(true);
          setSettingsError("");
        }
      } catch (error) {
        if (!disposed) {
          setSettingsError(error instanceof Error ? error.message : String(error));
        }
      }
    }

    void loadSettings();
    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    applyTheme(settings.theme);
    applyFont(settings.font);

    if (!settingsLoaded) return;
    const timeoutId = window.setTimeout(() => {
      void invoke("save_app_settings", { settings })
        .then(() => setSettingsError(""))
        .catch((error) => setSettingsError(error instanceof Error ? error.message : String(error)));
    }, 150);

    return () => window.clearTimeout(timeoutId);
  }, [settings, settingsLoaded]);

  useEffect(() => {
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const syncTheme = () => applyTheme(settings.theme);
    mediaQuery.addEventListener("change", syncTheme);
    return () => mediaQuery.removeEventListener("change", syncTheme);
  }, [settings.theme]);

  const updateSettings = useCallback((update: Partial<AppSettings>) => {
    setSettings((current) => ({ ...current, ...update }));
  }, []);

  return { settings, settingsError, updateSettings };
}
