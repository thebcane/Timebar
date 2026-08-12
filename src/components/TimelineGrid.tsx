import { rgba } from "../lib/colors";
import { formatDuration } from "../lib/time";
import type { TimelineBlock } from "../lib/types";

export interface TimelineColumn {
  key: string;
  /** Column header, e.g. "Mon 24" (omitted in day mode). */
  label?: string;
  /** Local YYYY-MM-DD this column represents. */
  date: string;
  blocks: TimelineBlock[];
}

function Block({
  block,
  minHour,
  pxPerHour,
  onClick,
}: {
  block: TimelineBlock;
  minHour: number;
  pxPerHour: number;
  onClick: () => void;
}) {
  const top = ((block.startMin - minHour * 60) / 60) * pxPerHour;
  const height = Math.max(16, ((block.endMin - block.startMin) / 60) * pxPerHour);
  const color = block.isActive ? "var(--accent)" : block.taskColor;
  return (
    <button
      type="button"
      onClick={onClick}
      className="absolute left-1 right-1 overflow-hidden rounded-md px-2 py-1 text-left"
      style={{
        top,
        height,
        background: rgba(block.taskColor, 0.16),
        border: `1px solid ${rgba(block.taskColor, 0.4)}`,
        borderLeft: `3px solid ${color}`,
      }}
    >
      <div className="flex items-baseline justify-between gap-1">
        <span className="truncate text-[12px] font-semibold" style={{ color: block.taskColor }}>
          {block.taskName}
        </span>
        <span className="tabular shrink-0 text-[11px]" style={{ color: block.taskColor }}>
          {formatDuration(block.durationMs)}
        </span>
      </div>
      {height > 34 && block.note ? (
        <div className="truncate text-[11px]" style={{ color: rgba(block.taskColor, 0.7) }}>
          {block.note}
        </div>
      ) : null}
    </button>
  );
}

export function TimelineGrid({
  columns,
  minHour,
  maxHour,
  pxPerHour,
  onBlockClick,
}: {
  columns: TimelineColumn[];
  minHour: number;
  maxHour: number;
  pxPerHour: number;
  onBlockClick: (sessionId: string) => void;
}) {
  const hours: number[] = [];
  for (let h = minHour; h <= maxHour; h++) hours.push(h);
  const bodyHeight = (maxHour - minHour) * pxPerHour;

  return (
    <div className="flex min-h-0 flex-1 overflow-auto">
      {/* Hour axis */}
      <div className="sticky left-0 z-10 w-12 shrink-0 bg-app">
        <div className="h-8" />
        <div className="relative" style={{ height: bodyHeight }}>
          {hours.map((h) => (
            <div
              key={h}
              className="tabular absolute right-1 -translate-y-1/2 text-[11px] text-text-muted"
              style={{ top: (h - minHour) * pxPerHour }}
            >
              {String(h).padStart(2, "0")}:00
            </div>
          ))}
        </div>
      </div>

      {/* Columns */}
      <div className="flex min-w-0 flex-1">
        {columns.map((col) => (
          <div key={col.key} className="min-w-0 flex-1 border-l border-hairline">
            <div className="flex h-8 items-center justify-center text-[12px] font-medium text-text-secondary">
              {col.label}
            </div>
            <div className="relative" style={{ height: bodyHeight }}>
              {hours.map((h) => (
                <div
                  key={h}
                  className="absolute left-0 right-0 border-t border-hairline/60"
                  style={{ top: (h - minHour) * pxPerHour }}
                />
              ))}
              {col.blocks.map((b) => (
                <Block
                  key={b.sessionId + b.startMin}
                  block={b}
                  minHour={minHour}
                  pxPerHour={pxPerHour}
                  onClick={() => onBlockClick(b.sessionId)}
                />
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
