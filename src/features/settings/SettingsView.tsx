import {
  ChevronLeft,
  FolderCog,
  Keyboard,
  Monitor,
  Palette,
  Search,
  Waypoints,
} from "lucide-react";
import { useState } from "react";

import keyboardShortcuts from "@/features/session-dashboard/config/keyboard-shortcuts.json";
import type { AgentRuntimeSource } from "@/shared/lib/generated/bindings";
import { cn } from "@/shared/lib/utils";

import type { AppFont, AppSettings, AppTheme, MonacoTheme } from "./useAppSettings";

type SettingGroup = "appearance" | "editor" | "graph" | "keyboard" | "runtimes";

const groups: {
  id: SettingGroup;
  label: string;
  section: "Personal" | "Coding";
  icon: typeof Palette;
}[] = [
  { id: "appearance", label: "Appearance", section: "Personal", icon: Palette },
  { id: "editor", label: "Editor", section: "Personal", icon: Monitor },
  { id: "keyboard", label: "Keyboard shortcuts", section: "Personal", icon: Keyboard },
  { id: "graph", label: "Session graph", section: "Coding", icon: Waypoints },
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
  const title = groups.find((item) => item.id === group)?.label ?? "Settings";
  const normalizedSearchQuery = searchQuery.trim().toLowerCase();

  return (
    <main className="bg-background flex h-full min-h-0 w-full">
      <aside className="border-border bg-sidebar flex w-[21.5rem] shrink-0 flex-col border-r px-3 pt-5 pb-4">
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
        <nav className="overflow-y-auto" aria-label="Settings sections">
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
            {group === "graph" ? (
              <GraphSettings settings={settings} onChange={onSettingsChange} />
            ) : null}
            {group === "keyboard" ? (
              <KeyboardSettings settings={settings} onChange={onSettingsChange} />
            ) : null}
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

function SettingRow({
  control,
  description,
  title,
}: {
  control: React.ReactNode;
  description: string;
  title: string;
}) {
  return (
    <div className="border-border flex min-h-20 items-center justify-between gap-8 border-b py-4 last:border-b-0">
      <div className="min-w-0">
        <h2 className="text-sm font-medium">{title}</h2>
        <p className="text-muted-foreground mt-1 text-sm leading-5">{description}</p>
      </div>
      <div className="w-56 shrink-0">{control}</div>
    </div>
  );
}

function SelectControl<T extends string | number>({
  value,
  choices,
  onChange,
}: {
  value: T;
  choices: { label: string; value: T }[];
  onChange: (value: T) => void;
}) {
  return (
    <select
      value={value}
      onChange={(event) => {
        const choice = choices.find(
          ({ value: choiceValue }) => String(choiceValue) === event.target.value
        );
        if (choice) onChange(choice.value);
      }}
      className="border-input bg-background focus:ring-ring/50 h-9 w-full rounded-md border px-3 text-sm outline-none focus:ring-2">
      {choices.map((choice) => (
        <option key={choice.value} value={choice.value}>
          {choice.label}
        </option>
      ))}
    </select>
  );
}

function AppearanceSettings({
  settings,
  onChange,
}: {
  settings: AppSettings;
  onChange: (update: Partial<AppSettings>) => void;
}) {
  return (
    <div>
      <SettingRow
        title="Color theme"
        description="Choose how the application matches your preferred appearance."
        control={
          <SelectControl<AppTheme>
            value={settings.theme}
            onChange={(theme) => onChange({ theme })}
            choices={[
              { value: "system", label: "System" },
              { value: "light", label: "Light" },
              { value: "dark", label: "Dark" },
            ]}
          />
        }
      />
      <SettingRow
        title="Interface font"
        description="Choose the typeface used throughout the application."
        control={
          <SelectControl<AppFont>
            value={settings.font}
            onChange={(font) => onChange({ font })}
            choices={[
              { value: "geist", label: "Geist" },
              { value: "system-sans", label: "System Sans" },
              { value: "system-serif", label: "System Serif" },
            ]}
          />
        }
      />
    </div>
  );
}

function EditorSettings({
  settings,
  onChange,
}: {
  settings: AppSettings;
  onChange: (update: Partial<AppSettings>) => void;
}) {
  return (
    <SettingRow
      title="Monaco color theme"
      description="Control the syntax highlighting theme used by file and diff viewers."
      control={
        <SelectControl<MonacoTheme>
          value={settings.monacoTheme}
          onChange={(monacoTheme) => onChange({ monacoTheme })}
          choices={[
            { value: "system", label: "Match app" },
            { value: "light", label: "Vitesse Light" },
            { value: "dark", label: "Vitesse Dark" },
          ]}
        />
      }
    />
  );
}

function ToggleControl({
  checked,
  label,
  onChange,
}: {
  checked: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex cursor-pointer justify-end">
      <input
        aria-label={label}
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
        className="peer sr-only"
        type="checkbox"
      />
      <span className="bg-muted peer-checked:bg-primary relative block h-6 w-10 rounded-full transition-colors after:absolute after:top-1 after:left-1 after:size-4 after:rounded-full after:bg-white after:shadow-sm after:transition-transform peer-checked:after:translate-x-4" />
    </label>
  );
}

function GraphSettings({
  settings,
  onChange,
}: {
  settings: AppSettings;
  onChange: (update: Partial<AppSettings>) => void;
}) {
  return (
    <SettingRow
      title="Committed files"
      description="Hide committed files to keep activity and the graph focused on outstanding changes."
      control={
        <ToggleControl
          label="Hide committed files"
          checked={settings.hideCommittedFiles}
          onChange={(hideCommittedFiles) => onChange({ hideCommittedFiles })}
        />
      }
    />
  );
}

function KeyboardSettings({
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

function RuntimeSettings({
  runtimeSources,
  settings,
  onChange,
}: {
  runtimeSources: AgentRuntimeSource[];
  settings: AppSettings;
  onChange: (update: Partial<AppSettings>) => void;
}) {
  const homes = settings.runtimeHomes;
  const sourceByProvider = new Map(runtimeSources.map((source) => [source.provider, source]));
  return (
    <div>
      <p className="text-muted-foreground mb-5 text-sm">
        Leave a field empty to use the default directory. Values are stored in
        ~/.config/coding-agent-va/config.toml.
      </p>
      <div className="divide-border divide-y">
        {(["codex", "claude", "pi"] as const).map((provider) => (
          <label className="flex items-center justify-between gap-8 py-4" key={provider}>
            <span className="min-w-0 text-sm font-medium">
              <span>
                {provider === "pi" ? "Pi Agent" : provider === "claude" ? "Claude Code" : "Codex"}
              </span>
              <span className="text-muted-foreground ml-2 text-xs font-normal">
                {sourceByProvider.get(provider)?.available ? "Available" : "Not found"}
              </span>
            </span>
            <input
              value={homes[provider]}
              onChange={(event) =>
                onChange({ runtimeHomes: { ...homes, [provider]: event.target.value } })
              }
              placeholder={
                sourceByProvider.get(provider)?.runtimeHome ?? "Default runtime directory"
              }
              className="border-input bg-background focus:ring-ring/50 h-9 w-80 shrink-0 rounded-md border px-3 text-sm outline-none focus:ring-2"
            />
          </label>
        ))}
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
