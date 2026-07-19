import {
  ChevronLeft,
  FileText,
  FolderCog,
  Keyboard,
  Monitor,
  Palette,
  Search,
  Sparkles,
} from "lucide-react";
import { useState } from "react";

import type { AgentRuntimeSource } from "@/shared/lib/generated/bindings";
import { cn } from "@/shared/lib/utils";

import { KeyboardSettings } from "./settings-keyboard";
import { LogsSettings } from "./settings-logs";
import {
  AppearanceSettings,
  DescriptionSettings,
  EditorSettings,
  RuntimeSettings,
} from "./settings-panels";
import type { AppSettings } from "./useAppSettings";

type SettingGroup = "appearance" | "descriptions" | "editor" | "keyboard" | "logs" | "runtimes";

const groups: {
  id: SettingGroup;
  label: string;
  section: "Personal" | "Coding";
  icon: typeof Palette;
}[] = [
  { id: "appearance", label: "Appearance", section: "Personal", icon: Palette },
  { id: "editor", label: "Editor", section: "Personal", icon: Monitor },
  { id: "keyboard", label: "Keyboard shortcuts", section: "Personal", icon: Keyboard },
  { id: "descriptions", label: "Descriptions", section: "Coding", icon: Sparkles },
  { id: "runtimes", label: "Agent runtimes", section: "Coding", icon: FolderCog },
];

export function SettingsView({
  runtimeSources,
  settings,
  settingsError,
  onClose,
  onSettingsChange,
}: {
  runtimeSources: AgentRuntimeSource[];
  settings: AppSettings;
  settingsError: string;
  onClose: () => void;
  onSettingsChange: (update: Partial<AppSettings>) => void;
}) {
  const [group, setGroup] = useState<SettingGroup>("appearance");
  const [searchQuery, setSearchQuery] = useState("");
  const title =
    group === "logs" ? "Logs" : (groups.find((item) => item.id === group)?.label ?? "Settings");
  const normalizedSearchQuery = searchQuery.trim().toLowerCase();

  return (
    <main data-testid="settings-view" className="bg-background flex h-full min-h-0 w-full">
      <aside className="border-border bg-sidebar flex w-[18rem] shrink-0 flex-col border-r px-3 pt-5 pb-4">
        <button
          type="button"
          onClick={onClose}
          className="text-muted-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground mb-4 flex items-center gap-2 rounded-lg px-2 py-2 text-sm transition-colors">
          <ChevronLeft className="size-4" /> Back to sessions
        </button>
        <label className="border-sidebar-border bg-background/40 focus-within:border-ring mb-5 flex h-9 items-center gap-2 rounded-lg border px-3 transition-colors">
          <Search className="text-muted-foreground size-4" />
          <input
            aria-label="Search settings"
            value={searchQuery}
            onChange={(event) => setSearchQuery(event.target.value)}
            placeholder="Search settings..."
            className="placeholder:text-muted-foreground min-w-0 flex-1 bg-transparent text-sm outline-none"
          />
        </label>
        <nav className="min-h-0 flex-1 overflow-y-auto" aria-label="Settings sections">
          {(["Personal", "Coding"] as const).map((section) => {
            const sectionGroups = groups.filter(
              (item) =>
                item.section === section &&
                (!normalizedSearchQuery || item.label.toLowerCase().includes(normalizedSearchQuery))
            );

            if (sectionGroups.length === 0) return null;

            return (
              <div className="mb-5" key={section}>
                <p className="text-muted-foreground mb-1 px-2 text-xs font-medium">{section}</p>
                <div className="space-y-0.5">
                  {sectionGroups.map(({ id, label, icon: Icon }) => (
                    <button
                      key={id}
                      type="button"
                      onClick={() => setGroup(id)}
                      className={cn(
                        "hover:bg-sidebar-accent flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-sm transition-colors",
                        group === id &&
                          "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
                      )}>
                      <Icon className="text-muted-foreground size-[1.05rem]" />
                      {label}
                    </button>
                  ))}
                </div>
              </div>
            );
          })}
        </nav>
        <div className="border-sidebar-border mt-3 border-t pt-3">
          <button
            type="button"
            onClick={() => setGroup("logs")}
            className={cn(
              "hover:bg-sidebar-accent flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-sm transition-colors",
              group === "logs" && "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
            )}>
            <FileText className="text-muted-foreground size-[1.05rem]" />
            Logs
          </button>
        </div>
        <div className="text-muted-foreground mt-auto flex items-center justify-between px-2 pt-4 text-xs">
          <span className="truncate" title="~/.config/coding-agent-va/config.toml">
            config.toml
          </span>
          <kbd className="border-sidebar-border bg-background/40 rounded border px-1.5 py-0.5">
            ⌘,
          </kbd>
        </div>
      </aside>
      <section className="min-w-0 flex-1 overflow-y-auto">
        <div className="mx-auto w-full max-w-4xl px-10 py-16 lg:px-16">
          <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
          {settingsError ? (
            <div className="border-destructive/40 bg-destructive/10 text-destructive mt-5 rounded-lg border px-4 py-3 text-sm">
              Settings could not be saved: {settingsError}
            </div>
          ) : null}
          <div className="mt-12">
            {group === "appearance" ? (
              <AppearanceSettings settings={settings} onChange={onSettingsChange} />
            ) : null}
            {group === "editor" ? (
              <EditorSettings settings={settings} onChange={onSettingsChange} />
            ) : null}
            {group === "descriptions" ? (
              <DescriptionSettings settings={settings} onChange={onSettingsChange} />
            ) : null}
            {group === "keyboard" ? (
              <KeyboardSettings settings={settings} onChange={onSettingsChange} />
            ) : null}
            {group === "logs" ? <LogsSettings /> : null}
            {group === "runtimes" ? (
              <RuntimeSettings
                runtimeSources={runtimeSources}
                settings={settings}
                onChange={onSettingsChange}
              />
            ) : null}
          </div>
        </div>
      </section>
    </main>
  );
}
