import { DiffEditor, Editor } from "@monaco-editor/react";
import { X } from "lucide-react";
import { useEffect, useState } from "react";

import type { AgentSessionFileDiff } from "@/features/session-dashboard/lib/session-watch";
import { Button } from "@/shared/components/ui/button";
import {
  ensureShikiMonaco,
  resolveMonacoLanguage,
  SHIKI_DARK_THEME,
  SHIKI_LIGHT_THEME,
} from "@/shared/lib/editor/monaco-shiki";

import styles from "./FileDiffViewer.module.css";

export function FileDiffViewer({
  diff,
  isLoading,
  errorMessage,
  onClose,
  theme,
  viewerMode,
}: {
  diff: AgentSessionFileDiff | null;
  isLoading: boolean;
  errorMessage: string;
  onClose: () => void;
  theme: typeof SHIKI_LIGHT_THEME | typeof SHIKI_DARK_THEME;
  viewerMode: "diff" | "read";
}) {
  const [isMonacoReady, setIsMonacoReady] = useState(false);
  const [monacoErrorMessage, setMonacoErrorMessage] = useState("");
  const language = diff ? resolveMonacoLanguage(diff.filePath) : "plaintext";

  useEffect(() => {
    let disposed = false;

    async function initializeMonaco() {
      try {
        await ensureShikiMonaco();

        if (!disposed) {
          setIsMonacoReady(true);
          setMonacoErrorMessage("");
        }
      } catch (error) {
        if (!disposed) {
          setMonacoErrorMessage(
            error instanceof Error ? error.message : "Failed to initialize syntax highlighting."
          );
        }
      }
    }

    void initializeMonaco();

    return () => {
      disposed = true;
    };
  }, []);

  return (
    <section className="border-border bg-background flex min-h-[32rem] flex-col overflow-hidden rounded-lg border">
      <div className="border-border bg-muted/30 flex items-center justify-between gap-3 border-b px-4 py-3">
        <div className="min-w-0">
          <p className="truncate text-sm font-medium">
            {diff?.displayPath ?? (viewerMode === "read" ? "Read viewer" : "Diff viewer")}
          </p>
          <p className="text-muted-foreground truncate text-xs">
            {diff
              ? viewerMode === "read"
                ? "Read-only workspace file"
                : `${diff.diffBaseLabel} -> ${diff.diffTargetLabel}${diff.isTracked ? "" : " (untracked)"}`
              : "Select a file to inspect changes."}
          </p>
        </div>
        <Button variant="ghost" size="icon" onClick={onClose} className="shrink-0 rounded-md">
          <X className="size-4" />
          <span className="sr-only">Close diff viewer</span>
        </Button>
      </div>
      <div className={`${styles.editorSurface} bg-muted/10 min-h-0 flex-1`}>
        {isLoading || !isMonacoReady ? (
          <div className="text-muted-foreground flex h-full items-center justify-center text-sm">
            {isLoading
              ? viewerMode === "read"
                ? "Loading file..."
                : "Loading diff..."
              : "Loading syntax highlighting..."}
          </div>
        ) : null}
        {!isLoading && isMonacoReady && (errorMessage || monacoErrorMessage) ? (
          <div className="flex h-full items-center justify-center px-6 text-center">
            <p className="text-muted-foreground text-sm">{errorMessage || monacoErrorMessage}</p>
          </div>
        ) : null}
        {!isLoading &&
        isMonacoReady &&
        !errorMessage &&
        !monacoErrorMessage &&
        diff &&
        viewerMode === "read" ? (
          <Editor
            height="100%"
            theme={theme}
            language={language}
            value={diff.modifiedContent}
            path={`file://${diff.filePath}?side=read`}
            options={{
              readOnly: true,
              automaticLayout: true,
              minimap: { enabled: false },
              scrollBeyondLastLine: false,
              wordWrap: "on",
              fontSize: 13,
              glyphMargin: false,
              lineNumbersMinChars: 3,
            }}
          />
        ) : null}
        {!isLoading &&
        isMonacoReady &&
        !errorMessage &&
        !monacoErrorMessage &&
        diff &&
        viewerMode === "diff" ? (
          <DiffEditor
            height="100%"
            theme={theme}
            language={language}
            originalLanguage={language}
            modifiedLanguage={language}
            original={diff.originalContent}
            modified={diff.modifiedContent}
            originalModelPath={`file://${diff.filePath}?side=original`}
            modifiedModelPath={`file://${diff.filePath}?side=modified`}
            options={{
              readOnly: true,
              renderSideBySide: true,
              automaticLayout: true,
              minimap: { enabled: false },
              scrollBeyondLastLine: false,
              wordWrap: "on",
              fontSize: 13,
              glyphMargin: false,
              lineNumbersMinChars: 3,
              hideUnchangedRegions: { enabled: true },
            }}
          />
        ) : null}
      </div>
    </section>
  );
}
