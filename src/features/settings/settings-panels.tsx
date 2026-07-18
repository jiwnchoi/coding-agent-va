import type { AgentRuntimeSource, DescriptionReasoning } from "@/shared/lib/generated/bindings";

import { SelectControl, SettingRow } from "./settings-controls";
import type { AppSettings, AppFont, AppTheme, MonacoTheme } from "./useAppSettings";

export function AppearanceSettings({
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

export function EditorSettings({
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

export function GraphSettings({
  settings,
  onChange,
}: {
  settings: AppSettings;
  onChange: (update: Partial<AppSettings>) => void;
}) {
  return (
    <div className="divide-border divide-y">
      <SettingRow
        title="Read files"
        description="Show files that the agent read without editing."
        control={
          <ToggleControl
            label="Show read files"
            checked={settings.showReadFiles}
            onChange={(showReadFiles) => onChange({ showReadFiles })}
          />
        }
      />
    </div>
  );
}

export function DescriptionSettings({
  settings,
  onChange,
}: {
  settings: AppSettings;
  onChange: (update: Partial<AppSettings>) => void;
}) {
  return (
    <div className="divide-border divide-y">
      {(["codex", "claude", "pi"] as const).map((provider) => {
        const providerSettings = settings.descriptions[provider];
        const providerLabel =
          provider === "codex" ? "Codex" : provider === "claude" ? "Claude Code" : "Pi Agent";
        const reasoningChoices = descriptionReasoningChoices(provider);

        return (
          <section className="py-5 first:pt-0" key={provider}>
            <h2 className="text-sm font-medium">{providerLabel}</h2>
            <p className="text-muted-foreground mt-1 text-sm leading-5">
              Model and reasoning used when describing a session graph node.
            </p>
            <div className="mt-4 grid grid-cols-2 gap-4">
              <label className="space-y-1.5">
                <span className="text-muted-foreground text-xs font-medium">Model</span>
                <input
                  value={providerSettings.model}
                  onChange={(event) =>
                    onChange({
                      descriptions: {
                        ...settings.descriptions,
                        [provider]: { ...providerSettings, model: event.target.value },
                      },
                    })
                  }
                  placeholder={provider === "pi" ? "Current session model" : "Model ID"}
                  className="border-input bg-background focus:ring-ring/50 h-9 w-full rounded-md border px-3 text-sm outline-none focus:ring-2"
                />
              </label>
              <label className="space-y-1.5">
                <span className="text-muted-foreground text-xs font-medium">Reasoning</span>
                <SelectControl<DescriptionReasoning>
                  value={providerSettings.reasoning}
                  onChange={(reasoning) =>
                    onChange({
                      descriptions: {
                        ...settings.descriptions,
                        [provider]: { ...providerSettings, reasoning },
                      },
                    })
                  }
                  choices={reasoningChoices}
                />
              </label>
            </div>
          </section>
        );
      })}
    </div>
  );
}

function descriptionReasoningChoices(provider: "codex" | "claude" | "pi") {
  const choices: { label: string; value: DescriptionReasoning }[] = [
    { value: "none", label: "None" },
    { value: "minimal", label: "Minimal" },
    { value: "low", label: "Low" },
    { value: "medium", label: "Medium" },
    { value: "high", label: "High" },
    { value: "xhigh", label: "Extra high" },
    { value: "max", label: "Maximum" },
  ];

  return provider === "claude" ? choices.filter(({ value }) => value !== "minimal") : choices;
}

export function RuntimeSettings({
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
