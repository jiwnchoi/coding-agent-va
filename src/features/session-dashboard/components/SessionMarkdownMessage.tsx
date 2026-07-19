import { ChevronDown, ChevronRight } from "lucide-react";
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
  onPress,
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
  onPress?: () => void;
  onToggle: () => void;
  onOpenFile: (selection: SelectedActivityFile) => void;
}) {
  const preview = messagePreview(source);
  const isMessageExpanded = preview.isExpandable && isExpanded;
  const chevronClassName = cn(
    "text-muted-foreground size-4 shrink-0",
    header && "mt-1.5 self-start"
  );
  const content = (
    <>
      <span className="min-w-0 flex-1">
        {header}
        {!isMessageExpanded ? (
          <span className={cn("block text-sm whitespace-pre-wrap", header && "mt-1")}>
            {preview.text}
          </span>
        ) : null}
      </span>
      {preview.isExpandable ? (
        isMessageExpanded ? (
          <ChevronDown className={chevronClassName} aria-hidden="true" />
        ) : (
          <ChevronRight className={chevronClassName} aria-hidden="true" />
        )
      ) : null}
    </>
  );

  return (
    <div className={className}>
      {preview.isExpandable || onPress ? (
        <button
          type="button"
          className="hover:bg-accent flex w-full items-center gap-2 px-3 py-3 text-left transition-colors"
          aria-expanded={preview.isExpandable ? isMessageExpanded : undefined}
          aria-pressed={isPressed}
          onClick={() => {
            onPress?.();
            if (preview.isExpandable) onToggle();
          }}>
          {content}
          {preview.isExpandable ? (
            <span className="sr-only">{isMessageExpanded ? "Collapse" : "Expand"}</span>
          ) : null}
        </button>
      ) : (
        <div className="flex w-full items-center gap-2 px-3 py-3 text-left">{content}</div>
      )}
      {isMessageExpanded ? (
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
  const isExpandable = preview.length < firstLine.length || text.trim() !== firstLine;
  return { isExpandable, text: isExpandable ? `${preview}…` : preview };
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
