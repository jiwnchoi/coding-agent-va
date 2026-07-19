export type EventSequenceFileLayout = {
  columnByFile: Map<string, number>;
  columnCount: number;
};

type FileInterval = {
  key: string;
  firstRow: number;
  lastRow: number;
  order: number;
};

export function buildEventSequenceFileLayout(
  fileKeysByRow: ReadonlyArray<ReadonlyArray<string>>
): EventSequenceFileLayout {
  const intervalsByFile = new Map<string, FileInterval>();
  let order = 0;

  fileKeysByRow.forEach((fileKeys, rowIndex) => {
    for (const key of fileKeys) {
      const interval = intervalsByFile.get(key);
      if (interval) {
        interval.lastRow = rowIndex;
        continue;
      }

      intervalsByFile.set(key, {
        key,
        firstRow: rowIndex,
        lastRow: rowIndex,
        order,
      });
      order += 1;
    }
  });

  const intervals = [...intervalsByFile.values()].sort(
    (left, right) => left.firstRow - right.firstRow || left.order - right.order
  );
  const columnCount = findColumnCount(intervals, fileKeysByRow.length);
  const columnByFile = new Map<string, number>();
  const lastRowByColumn = Array.from({ length: columnCount }, () => -1);

  for (let start = 0; start < intervals.length; ) {
    const firstRow = intervals[start]?.firstRow;
    if (firstRow === undefined) break;

    let end = start + 1;
    while (intervals[end]?.firstRow === firstRow) end += 1;

    const startingIntervals = intervals.slice(start, end);
    const connectedIntervals = startingIntervals.filter(
      (interval) => interval.lastRow > interval.firstRow
    );
    const isolatedIntervals = startingIntervals.filter(
      (interval) => interval.lastRow === interval.firstRow
    );
    const availableColumns = lastRowByColumn
      .map((lastRow, column) => ({ column, lastRow }))
      .filter(({ lastRow }) => lastRow < firstRow)
      .map(({ column }) => column);

    assignColumns(connectedIntervals, availableColumns.slice(0, connectedIntervals.length));
    assignColumns(isolatedIntervals, availableColumns.slice(-isolatedIntervals.length));
    start = end;
  }

  return { columnByFile, columnCount };

  function assignColumns(assignedIntervals: FileInterval[], columns: number[]) {
    assignedIntervals.forEach((interval, index) => {
      const column = columns[index];
      if (column === undefined) return;
      columnByFile.set(interval.key, column);
      lastRowByColumn[column] = interval.lastRow;
    });
  }
}

function findColumnCount(intervals: FileInterval[], rowCount: number) {
  const rowChanges = Array.from({ length: rowCount + 1 }, () => 0);
  for (const interval of intervals) {
    rowChanges[interval.firstRow] += 1;
    rowChanges[interval.lastRow + 1] -= 1;
  }

  let activeCount = 0;
  let columnCount = 0;
  for (const change of rowChanges) {
    activeCount += change;
    columnCount = Math.max(columnCount, activeCount);
  }
  return columnCount;
}
