import type { ActivitySection, SelectedActivityFile } from "@/lib/session-watch";
import { cn } from "@/lib/utils";

export function FileActivityPanels({
  isLoading,
  sections,
  selectedFilePath,
  onSelectFile,
}: {
  isLoading: boolean;
  sections: ActivitySection[];
  selectedFilePath: string;
  onSelectFile: (selection: SelectedActivityFile) => void;
}) {
  return (
    <div className="grid gap-4 lg:grid-cols-3">
      {sections.map((section) => {
        const Icon = section.icon;

        return (
          <section key={section.key} className="border-border bg-muted/20 rounded-lg border p-4">
            <div className="mb-3 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Icon className="text-muted-foreground size-4" />
                <h2 className="text-sm font-medium">{section.title}</h2>
              </div>
              <span className="text-muted-foreground text-xs">{section.files.length}</span>
            </div>
            <div className="space-y-2">
              {section.files.slice(0, 12).map((filePath) => (
                <button
                  key={filePath}
                  type="button"
                  onClick={() => onSelectFile({ activityKey: section.key, filePath })}
                  className={cn(
                    "bg-background hover:bg-muted/60 w-full rounded-md border px-3 py-2 text-left text-sm transition-colors",
                    selectedFilePath === filePath && "border-foreground/20 bg-muted/80"
                  )}>
                  <p className="truncate font-medium">{filePath.split("/").pop() ?? filePath}</p>
                  <p className="text-muted-foreground truncate text-xs">{filePath}</p>
                </button>
              ))}
              {section.files.length === 0 && !isLoading ? (
                <p className="text-muted-foreground text-sm">No files</p>
              ) : null}
              {isLoading ? (
                <p className="text-muted-foreground text-sm">Loading file activity...</p>
              ) : null}
            </div>
          </section>
        );
      })}
    </div>
  );
}
