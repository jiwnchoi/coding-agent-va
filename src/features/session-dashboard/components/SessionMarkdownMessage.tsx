import type { ReactNode } from "react";

import { normalizeWorkspacePath } from "@/features/session-dashboard/context-graph/contextGraphPaths";
import type { SelectedActivityFile } from "@/features/session-dashboard/lib/session-watch";
import type { AgentSessionFileActivity } from "@/shared/lib/generated/bindings";
import { cn } from "@/shared/lib/utils";

import { MdxDescription } from "./MdxDescription";
import styles from "./SessionMarkdownMessage.module.css";

export function SessionMarkdownMessage({
  source,
  isExpanded,
  isPressed,
  className,
  header,
  fileActivity,
  workspacePath,
  onToggle,
  onOpenFile,
}: {
  source: string;
  isExpanded: boolean;
  isPressed?: boolean;
  className?: string;
  header: ReactNode;
  fileActivity: AgentSessionFileActivity;
  workspacePath: string | null;
  onToggle: () => void;
  onOpenFile: (selection: SelectedActivityFile) => void;
}) {
  return (
    <div className={className}>
      <button
        type="button"
        className={cn(
          "hover:bg-accent w-full px-3 text-left transition-colors",
          isExpanded ? "pt-3 pb-2" : "py-3"
        )}
        aria-expanded={isExpanded}
        aria-pressed={isPressed}
        onClick={onToggle}>
        {header}
        {!isExpanded ? (
          <span className="mt-1 block text-sm whitespace-pre-wrap">{messagePreview(source)}</span>
        ) : null}
      </button>
      {isExpanded ? (
        <div className={cn(styles.markdown, "px-3 pb-3 text-sm leading-6")}>
          <MdxDescription
            source={source}
            onOpenFile={(filePath) =>
              onOpenFile(fileSelectionForPath(fileActivity, filePath, workspacePath))
            }
          />
        </div>
      ) : null}
    </div>
  );
}

function messagePreview(text: string) {
  const firstLine = text.split(/\r?\n/, 1)[0]?.trim() ?? "";
  const preview = firstLine.slice(0, 120).trimEnd();
  return preview.length < firstLine.length || text.trim() !== firstLine ? `${preview}…` : preview;
}

function fileSelectionForPath(
  fileActivity: AgentSessionFileActivity,
  linkedPath: string,
  workspacePath: string | null
): SelectedActivityFile {
  const normalizedLinkedPath = normalizeWorkspacePath(linkedPath, workspacePath ?? "");
  const activityGroups = [
    ["edited", fileActivity.editedFiles],
    ["deleted", fileActivity.deletedFiles],
    ["impacted", fileActivity.impactedFiles],
    ["read", fileActivity.readFiles],
  ] as const;

  for (const [activityKey, filePaths] of activityGroups) {
    const filePath = filePaths.find((candidate) => {
      const normalizedCandidate = normalizeWorkspacePath(candidate, workspacePath ?? "");
      return (
        normalizedCandidate === normalizedLinkedPath ||
        normalizedCandidate.endsWith(`/${normalizedLinkedPath}`)
      );
    });
    if (filePath) return { activityKey, filePath };
  }

  return { activityKey: "read", filePath: linkedPath };
}
