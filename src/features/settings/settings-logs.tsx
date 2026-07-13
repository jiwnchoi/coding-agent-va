import { useVirtualizer } from "@tanstack/react-virtual";
import { RefreshCw, Search, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import type { LogEntry, LogLevel } from "@/shared/lib/generated/bindings";
import { logger } from "@/shared/lib/logger";
import { cn } from "@/shared/lib/utils";

import { SelectControl } from "./settings-controls";

export function LogsSettings() {
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [levelFilter, setLevelFilter] = useState<LogLevel | "all">("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [error, setError] = useState("");
  const logScrollRef = useRef<HTMLDivElement>(null);

  async function loadLogs() {
    try {
      setEntries(await logger.entries());
      setError("");
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    }
  }

  useEffect(() => {
    void loadLogs();
  }, []);

  async function clearLogs() {
    try {
      await logger.clear();
      setEntries([]);
      setError("");
    } catch (clearError) {
      setError(clearError instanceof Error ? clearError.message : String(clearError));
    }
  }

  const normalizedSearchQuery = searchQuery.trim().toLowerCase();
  const visibleEntries = entries.filter((entry) => {
    if (levelFilter !== "all" && entry.level !== levelFilter) return false;
    if (!normalizedSearchQuery) return true;

    return [entry.message, entry.context ? JSON.stringify(entry.context) : ""]
      .join(" ")
      .toLowerCase()
      .includes(normalizedSearchQuery);
  });
  const logVirtualizer = useVirtualizer({
    count: visibleEntries.length,
    getScrollElement: () => logScrollRef.current,
    estimateSize: () => 58,
    getItemKey: (index) => `${visibleEntries[index]?.timestamp ?? "log"}-${index}`,
    overscan: 12,
  });

  return (
    <div>
      <div className="border-border flex items-center justify-between gap-4 border-b pb-4">
        <div>
          <h2 className="text-sm font-medium">Application logs</h2>
          <p className="text-muted-foreground mt-1 text-sm leading-5">
            Logs are stored in ~/.config/coding-agent-va/app.log.
          </p>
        </div>
        <div className="flex shrink-0 gap-2">
          <button
            type="button"
            onClick={() => void loadLogs()}
            className="border-input hover:bg-accent inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm">
            <RefreshCw className="size-3.5" /> Refresh
          </button>
          <button
            type="button"
            onClick={() => void clearLogs()}
            className="border-destructive/40 text-destructive hover:bg-destructive/10 inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm">
            <Trash2 className="size-3.5" /> Clear
          </button>
        </div>
      </div>
      <div className="border-border flex flex-nowrap gap-3 border-b py-4">
        <label className="border-input bg-background focus-within:border-ring flex h-9 min-w-0 flex-1 items-center gap-2 rounded-md border px-3">
          <Search className="text-muted-foreground size-4" />
          <input
            aria-label="Search logs"
            value={searchQuery}
            onChange={(event) => setSearchQuery(event.target.value)}
            placeholder="Search logs..."
            className="placeholder:text-muted-foreground min-w-0 flex-1 bg-transparent text-sm outline-none"
          />
        </label>
        <div className="w-36 shrink-0">
          <SelectControl<LogLevel | "all">
            value={levelFilter}
            onChange={setLevelFilter}
            choices={[
              { value: "all", label: "All levels" },
              { value: "debug", label: "Debug" },
              { value: "info", label: "Info" },
              { value: "warn", label: "Warn" },
              { value: "error", label: "Error" },
            ]}
          />
        </div>
      </div>
      {error ? <p className="text-destructive py-4 text-sm">{error}</p> : null}
      <div
        ref={logScrollRef}
        className="border-border mt-5 h-[min(60vh,42rem)] overflow-y-auto rounded-lg border">
        {visibleEntries.length === 0 ? (
          <p className="text-muted-foreground px-4 py-8 text-center text-sm">No log entries.</p>
        ) : (
          <div className="relative w-full" style={{ height: `${logVirtualizer.getTotalSize()}px` }}>
            {logVirtualizer.getVirtualItems().map((virtualItem) => {
              const entry = visibleEntries[virtualItem.index];
              return (
                <div
                  key={virtualItem.key}
                  data-index={virtualItem.index}
                  ref={logVirtualizer.measureElement}
                  className="border-border absolute top-0 left-0 w-full border-b px-4 py-3 font-mono text-xs"
                  style={{ transform: `translateY(${virtualItem.start}px)` }}>
                  <div className="flex gap-3">
                    <span className="text-muted-foreground shrink-0">
                      {formatLogTimestamp(entry.timestamp)}
                    </span>
                    <span
                      className={cn(
                        "h-fit rounded px-1.5 py-0.5 text-[0.65rem] font-semibold tracking-wide",
                        logLevelBadgeClass(entry.level)
                      )}>
                      {entry.level.toUpperCase()}
                    </span>
                    <span className="min-w-0 break-words">{entry.message}</span>
                  </div>
                  {entry.context ? (
                    <pre className="text-muted-foreground mt-1 ml-[7.5rem] break-words whitespace-pre-wrap">
                      {JSON.stringify(entry.context)}
                    </pre>
                  ) : null}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

function logLevelBadgeClass(level: LogLevel) {
  return level === "error"
    ? "bg-destructive/15 text-destructive"
    : level === "warn"
      ? "bg-amber-500/15 text-amber-700 dark:text-amber-400"
      : level === "info"
        ? "bg-blue-500/15 text-blue-700 dark:text-blue-400"
        : "bg-muted text-muted-foreground";
}

function formatLogTimestamp(timestamp: string) {
  return new Date(timestamp).toLocaleString();
}
