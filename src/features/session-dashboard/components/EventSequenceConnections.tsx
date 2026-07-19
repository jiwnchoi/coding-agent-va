import { Line } from "@visx/shape";

import { connectionEndpoints, isHoveredFile } from "./event-sequence-geometry";
import styles from "./EventSequenceVisualization.module.css";

export type EventSequenceColumn = "edited" | "readImpacted" | "impacted" | "read";
export type EventSequencePoint = {
  key: string;
  filePath: string;
  column: EventSequenceColumn;
  x: number;
  y: number;
  rowIndex: number;
};

const COLORS: Record<EventSequenceColumn, string> = {
  edited: "var(--activity-edited)",
  readImpacted: "var(--activity-read-impacted)",
  impacted: "var(--activity-unread-impacted)",
  read: "var(--muted-foreground)",
};

export function EventSequenceConnections({
  connections,
  hoveredFilePaths,
  workspacePath,
  circleRadius,
}: {
  connections: Array<[EventSequencePoint, EventSequencePoint]>;
  hoveredFilePaths: string[] | null;
  workspacePath: string | null;
  circleRadius: number;
}) {
  return (
    <>
      <defs>
        {connections.map(([from, to], index) => {
          const fromColor = COLORS[from.column];
          const toColor = COLORS[to.column];
          if (fromColor === toColor) return null;
          const endpoints = connectionEndpoints(from, to, circleRadius);
          const isDimmed = !isHoveredFile(from.filePath, hoveredFilePaths, workspacePath);
          return (
            <linearGradient
              key={`connection-gradient-${index}`}
              id={`connection-gradient-${index}`}
              gradientUnits="userSpaceOnUse"
              x1={endpoints.from.x}
              y1={endpoints.from.y}
              x2={endpoints.to.x}
              y2={endpoints.to.y}>
              <stop
                className={styles.connectionGradientStop}
                offset="0%"
                style={{ stopColor: isDimmed ? blendColor(fromColor) : fromColor }}
              />
              <stop
                className={styles.connectionGradientStop}
                offset="100%"
                style={{ stopColor: isDimmed ? blendColor(toColor) : toColor }}
              />
            </linearGradient>
          );
        })}
      </defs>
      {connections.map(([from, to], index) => {
        const fromColor = COLORS[from.column];
        const toColor = COLORS[to.column];
        const endpoints = connectionEndpoints(from, to, circleRadius);
        const isDimmed = !isHoveredFile(from.filePath, hoveredFilePaths, workspacePath);
        return (
          <Line
            key={`${from.key}:${from.rowIndex}:${to.rowIndex}`}
            from={endpoints.from}
            to={endpoints.to}
            className={styles.connection}
            style={{
              stroke:
                fromColor === toColor
                  ? isDimmed
                    ? blendColor(fromColor)
                    : fromColor
                  : `url(#connection-gradient-${index})`,
            }}
          />
        );
      })}
    </>
  );
}

function blendColor(color: string) {
  return `color-mix(in srgb, ${color} 42%, var(--background))`;
}
