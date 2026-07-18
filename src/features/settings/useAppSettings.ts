import { useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState } from "react";

import { loadAppSettings, saveAppSettings } from "@/shared/lib/agent-api";
import type { AppFont, AppSettings, AppTheme, MonacoTheme } from "@/shared/lib/generated/bindings";
import { logger } from "@/shared/lib/logger";

export type { AppFont, AppSettings, AppTheme, MonacoTheme };

export const DEFAULT_APP_SETTINGS: AppSettings = {
  theme: "system",
  font: "geist",
  monacoTheme: "system",
  hideCommittedFiles: true,
  showReadFiles: false,
  keyboardShortcuts: {},
  runtimeHomes: { claude: "", codex: "", pi: "" },
  descriptions: {
    codex: { model: "gpt-5.6-luna", reasoning: "none" },
    claude: { model: "claude-haiku-4-5", reasoning: "none" },
    pi: { model: "", reasoning: "none" },
  },
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
  const [settingsError, setSettingsError] = useState("");
  const settingsQuery = useQuery({
    queryKey: ["app-settings"],
    queryFn: loadAppSettings,
    staleTime: Infinity,
    refetchOnMount: false,
  });
  const saveChainRef = useRef(Promise.resolve());

  useEffect(() => {
    if (settingsQuery.data) {
      setSettings(settingsQuery.data);
      setSettingsError("");
      void logger.info("Loaded application settings");
    }
    if (settingsQuery.error) {
      setSettingsError(String(settingsQuery.error));
      void logger.error("Failed to load application settings", {
        error: String(settingsQuery.error),
      });
    }
  }, [settingsQuery.data, settingsQuery.error]);

  useEffect(() => {
    applyTheme(settings.theme);
    applyFont(settings.font);

    if (!settingsQuery.data) return;
    let disposed = false;
    const timeoutId = window.setTimeout(() => {
      saveChainRef.current = saveChainRef.current
        .catch(() => undefined)
        .then(() => saveAppSettings(settings))
        .then(() => {
          if (!disposed) {
            setSettingsError("");
            void logger.debug("Saved application settings");
          }
        })
        .catch((error) => {
          if (!disposed) {
            setSettingsError(error instanceof Error ? error.message : String(error));
            void logger.error("Failed to save application settings", { error: String(error) });
          }
        });
    }, 150);

    return () => {
      disposed = true;
      window.clearTimeout(timeoutId);
    };
  }, [settings, settingsQuery.data]);

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
