import { scaleLinear } from "@visx/scale";
import { Circle } from "@visx/shape";
import { useMemo, useRef } from "react";

import { normalizeWorkspacePath } from "@/features/session-dashboard/context-graph/contextGraphPaths";
import { useElementSize } from "@/features/session-dashboard/hooks/useElementSize";
import { buildSessionScopeSelection } from "@/features/session-dashboard/lib/session-scope";
import type {
  SelectedActivityFile,
  SessionScopeSelection,
} from "@/features/session-dashboard/lib/session-watch";
import type {
  AgentSessionDetails,
  AgentSessionFileActivity,
} from "@/shared/lib/generated/bindings";
import { cn } from "@/shared/lib/utils";

import { isHoveredFile } from "./event-sequence-geometry";
import { buildEventSequenceFileLayout } from "./event-sequence-layout";
import { EventSequenceConnections, type EventSequencePoint } from "./EventSequenceConnections";
import styles from "./EventSequenceVisualization.module.css";

const ROW_HEIGHT = 36;
const HEADER_HEIGHT = 4;
const SIDE_PADDING = 12;
const CIRCLE_RADIUS = 6;
const MIN_COLUMN_SPACING = 1;
const MAX_COLUMN_SPACING = 36;
const MIN_CHART_WIDTH = 360;

const COLUMNS = [
  { key: "edited", label: "Edited", color: "var(--activity-edited)" },
  { key: "readImpacted", label: "Impacted & read", color: "var(--activity-read-impacted)" },
  { key: "impacted", label: "Impacted", color: "var(--activity-unread-impacted)" },
  { key: "read", label: "Read", color: "var(--muted-foreground)" },
] as const;

type EventColumn = (typeof COLUMNS)[number]["key"];

type EventRow = {
  id: string;
  activity: AgentSessionFileActivity;
  selection: SessionScopeSelection;
};

type EventPoint = EventSequencePoint & { activityKey: SelectedActivityFile["activityKey"] };

export function EventSequenceVisualization({
  rows,
  showReadFiles,
  selectedScope,
  hoveredFilePaths,
  workspacePath,
  onSelectScope,
  onSelectFile,
  onHoverFilePaths,
}: {
  rows: EventRow[];
  showReadFiles: boolean;
  selectedScope: SessionScopeSelection | null;
  hoveredFilePaths: string[] | null;
  workspacePath: string | null;
  onSelectScope: (selection: SessionScopeSelection | null) => void;
  onSelectFile: (selection: SelectedActivityFile) => void;
  onHoverFilePaths: (filePaths: string[] | null) => void;
}) {
  const scrollAreaRef = useRef<HTMLDivElement>(null);
  const { width } = useElementSize(scrollAreaRef);
  const rowGroups = useMemo(
    () => rows.map((row) => buildEventGroups(row.activity, workspacePath, showReadFiles)),
    [rows, showReadFiles, workspacePath]
  );
  const { columnByFile, columnCount } = useMemo(
    () =>
      buildEventSequenceFileLayout(
        rowGroups.map((groups) =>
          COLUMNS.flatMap((column) => groups[column.key].map((event) => event.key))
        )
      ),
    [rowGroups]
  );
  const columnTrackCount = Math.max(0, columnCount - 1);
  const availableTrackWidth = Math.max(0, width - SIDE_PADDING * 2);
  const columnSpacing =
    columnTrackCount > 0
      ? scaleLinear({
          domain: [columnTrackCount * MIN_COLUMN_SPACING, columnTrackCount * MAX_COLUMN_SPACING],
          range: [MIN_COLUMN_SPACING, MAX_COLUMN_SPACING],
          clamp: true,
        })(availableTrackWidth)
      : 0;
  const contentWidth = SIDE_PADDING * 2 + columnTrackCount * columnSpacing;
  const chartWidth = Math.max(width, contentWidth, MIN_CHART_WIDTH);
  const height = HEADER_HEIGHT + Math.max(rows.length, 1) * ROW_HEIGHT;

  const points = useMemo(() => {
    const nextPoints: EventPoint[] = [];

    rows.forEach((_, rowIndex) => {
      const groups = rowGroups[rowIndex];
      if (!groups) return;
      COLUMNS.forEach((column) => {
        for (const event of groups[column.key]) {
          nextPoints.push({
            ...event,
            column: column.key,
            x:
              columnCount > 1
                ? SIDE_PADDING + (columnByFile.get(event.key) ?? 0) * columnSpacing
                : chartWidth / 2,
            y: HEADER_HEIGHT + ROW_HEIGHT * rowIndex + ROW_HEIGHT / 2,
            rowIndex,
          });
        }
      });
    });
    return nextPoints;
  }, [chartWidth, columnByFile, columnCount, columnSpacing, rowGroups, rows]);

  const connections = useMemo(() => {
    const lastPointByFile = new Map<string, EventPoint>();
    const nextConnections: Array<[EventPoint, EventPoint]> = [];
    for (const point of points) {
      const previousPoint = lastPointByFile.get(point.key);
      if (previousPoint && previousPoint.rowIndex !== point.rowIndex) {
        nextConnections.push([previousPoint, point]);
      }
      lastPointByFile.set(point.key, point);
    }
    return nextConnections;
  }, [points]);

  function handleEventClick(point: EventPoint) {
    const row = rows[point.rowIndex];
    if (!row) return;
    onSelectScope(row.selection);
    onSelectFile({ activityKey: point.activityKey, filePath: point.filePath });
  }

  if (rows.length === 0) {
    return <p className="text-muted-foreground px-2 py-4 text-sm">No activity found.</p>;
  }

  return (
    <div className={styles.root}>
      <div
        ref={scrollAreaRef}
        className={styles.scrollArea}
        role="img"
        aria-label="Event sequence file activity visualization">
        <svg
          className={styles.chart}
          width={chartWidth}
          height={height}
          viewBox={`0 0 ${chartWidth} ${height}`}>
          {rows.map((row, rowIndex) => {
            const y = HEADER_HEIGHT + ROW_HEIGHT * rowIndex;
            const isSelected =
              selectedScope?.turnId === row.selection.turnId &&
              selectedScope.taskId === row.selection.taskId;
            const selectRow = () => onSelectScope(isSelected ? null : row.selection);
            return (
              <g key={row.id}>
                <rect
                  x={0}
                  y={y + 1}
                  width={chartWidth}
                  height={ROW_HEIGHT - 2}
                  rx={6}
                  className={cn(styles.rowBackground, isSelected && styles.rowBackgroundSelected)}
                  role="button"
                  tabIndex={0}
                  aria-pressed={isSelected}
                  aria-label="Select event row"
                  onClick={selectRow}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      selectRow();
                    }
                  }}
                />
              </g>
            );
          })}
          <EventSequenceConnections
            connections={connections}
            hoveredFilePaths={hoveredFilePaths}
            workspacePath={workspacePath}
            circleRadius={CIRCLE_RADIUS}
          />
          {points.map((point) => {
            const column = COLUMNS.find((candidate) => candidate.key === point.column);
            const isDimmed = !isHoveredFile(point.filePath, hoveredFilePaths, workspacePath);
            return (
              <g
                key={`${point.rowIndex}:${point.column}:${point.key}`}
                className={styles.event}
                role="button"
                tabIndex={0}
                aria-label={`${point.filePath}, ${column?.label ?? point.column}`}
                onClick={() => handleEventClick(point)}
                onMouseEnter={() => onHoverFilePaths([point.filePath])}
                onMouseLeave={() => onHoverFilePaths(null)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    handleEventClick(point);
                  }
                }}>
                <Circle
                  cx={point.x}
                  cy={point.y}
                  r={CIRCLE_RADIUS}
                  style={{
                    fill: isDimmed
                      ? `color-mix(in srgb, ${column?.color ?? "var(--muted-foreground)"} 42%, var(--background))`
                      : column?.color,
                  }}
                />
                <title>{`${point.filePath} · ${column?.label ?? point.column}`}</title>
              </g>
            );
          })}
        </svg>
      </div>
    </div>
  );
}

export function buildEventRows(
  turns: AgentSessionDetails["turns"],
  trackingMode: "prompts" | "tasks"
): EventRow[] {
  if (trackingMode === "prompts") {
    return turns.map((turn) => ({
      id: turn.id,
      activity: turn.fileActivity,
      selection: buildSessionScopeSelection(turn.id, null, turn),
    }));
  }

  return turns.flatMap((turn) =>
    turn.tasks.map((task) => ({
      id: task.id,
      activity: task.fileActivity,
      selection: buildSessionScopeSelection(turn.id, task.id, task),
    }))
  );
}

function buildEventGroups(
  fileActivity: AgentSessionFileActivity,
  workspacePath: string | null,
  showReadFiles: boolean
) {
  const normalize = (filePath: string) => normalizeWorkspacePath(filePath, workspacePath ?? "");
  const read = new Map(fileActivity.readFiles.map((filePath) => [normalize(filePath), filePath]));
  const impacted = new Map(
    fileActivity.impactedFiles.map((filePath) => [normalize(filePath), filePath])
  );
  const edited = new Map(
    fileActivity.editedFiles.map((filePath) => [normalize(filePath), filePath])
  );
  const seen = new Set<string>();
  const groups = {
    edited: [],
    readImpacted: [],
    impacted: [],
    read: [],
  } as Record<EventColumn, Array<Omit<EventPoint, "column" | "x" | "y" | "rowIndex">>>;

  function add(
    column: EventColumn,
    files: Map<string, string>,
    activityKey: SelectedActivityFile["activityKey"]
  ) {
    for (const [key, filePath] of files) {
      if (seen.has(key)) continue;
      seen.add(key);
      groups[column].push({ key, filePath, activityKey });
    }
  }

  add("edited", edited, "edited");
  add("readImpacted", new Map([...impacted].filter(([key]) => read.has(key))), "impacted");
  add("impacted", impacted, "impacted");
  if (showReadFiles) {
    add("read", read, "read");
  }
  return groups;
}
