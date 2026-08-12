import { useState } from "react";
import { FileText, DollarSign, Pencil, Trash2 } from "lucide-react";
import { format, parseISO } from "date-fns";
import { Eyebrow, EmptyState } from "./primitives";
import { ContextMenu } from "./ContextMenu";
import { useElapsed } from "../store/store";
import { formatClock, formatDuration, formatMoney, secondsOf } from "../lib/time";
import type { DayGroup, EntryView } from "../lib/types";

function EntryRow({
  entry,
  liveSecs,
  onClick,
  onContextMenu,
}: {
  entry: EntryView;
  liveSecs?: string;
  onClick: () => void;
  onContextMenu?: (e: React.MouseEvent) => void;
}) {
  return (
    <div
      onClick={onClick}
      onContextMenu={onContextMenu}
      className={`group flex cursor-pointer items-center gap-3 rounded-row px-2 py-2 transition-colors ${
        entry.isActive ? "bg-accent/10" : "hover:bg-panel-raised"
      }`}
    >
      <div className="tabular w-11 shrink-0 text-right text-[12px] leading-tight text-text-secondary">
        <div>{formatClock(entry.startedAt)}</div>
        <div className="text-text-muted">{formatClock(entry.endedAt)}</div>
      </div>
      <div
        className="w-[3px] self-stretch rounded"
        style={{ background: entry.isActive ? "var(--accent)" : entry.taskColor }}
      />
      <div className="min-w-0 flex-[2]">
        {entry.folderName ? <Eyebrow>{entry.folderName}</Eyebrow> : null}
        <div
          className="truncate text-[14px] font-medium"
          style={{ color: entry.isActive ? "var(--accent-bright)" : "var(--text-primary)" }}
        >
          {entry.taskName}
        </div>
      </div>

      <div className="hidden min-w-0 flex-1 truncate text-[12px] text-text-secondary sm:block">
        {entry.note ?? ""}
      </div>

      <FileText size={14} className="shrink-0 text-text-muted opacity-60" />

      {entry.billable && entry.revenue > 0 ? (
        <div className="flex shrink-0 items-center gap-1 text-[12px] text-text-secondary">
          <DollarSign size={12} className="text-text-muted" />
          <span className="tabular">{formatMoney(entry.revenue)}</span>
        </div>
      ) : null}

      <div className="tabular w-16 shrink-0 text-right text-[14px] font-semibold text-text-primary">
        {formatDuration(entry.durationMs)}
        {entry.isActive && liveSecs ? <span className="text-text-muted">:{liveSecs}</span> : null}
      </div>
    </div>
  );
}

function DayHeader({ date, totalMs, totalRevenue }: { date: string; totalMs: number; totalRevenue: number }) {
  let weekday = date;
  let rest = "";
  try {
    const d = parseISO(date);
    weekday = format(d, "EEEE");
    rest = format(d, "MMMM d");
  } catch {
    /* keep raw */
  }
  return (
    <div className="mb-1 flex items-baseline justify-between border-b border-hairline px-2 pb-1.5">
      <span className="text-[15px]">
        <span className="font-semibold text-text-primary">{weekday},</span>{" "}
        <span className="text-text-secondary">{rest}</span>
      </span>
      <span className="flex items-baseline gap-3">
        <span className="tabular text-[13px] text-text-muted">{formatMoney(totalRevenue)}</span>
        <span className="tabular text-[14px] font-semibold text-text-secondary">
          {formatDuration(totalMs)}
        </span>
      </span>
    </div>
  );
}

export function EntriesFeed({
  days,
  onEntryClick,
  onEntryDelete,
  emptyText = "No entries in this range. Start a task from the menu bar.",
}: {
  days: DayGroup[];
  onEntryClick: (sessionId: string) => void;
  onEntryDelete?: (sessionId: string) => void;
  emptyText?: string;
}) {
  const elapsed = useElapsed();
  const liveSecs = secondsOf(elapsed);
  const [menu, setMenu] = useState<{ x: number; y: number; sessionId: string } | null>(null);

  if (days.length === 0) return <EmptyState>{emptyText}</EmptyState>;

  return (
    <div className="flex flex-col gap-5">
      {days.map((day) => (
        <div key={day.date}>
          <DayHeader date={day.date} totalMs={day.totalMs} totalRevenue={day.totalRevenue} />
          {day.entries.map((en) => (
            <EntryRow
              key={en.sessionId + en.startedAt}
              entry={en}
              liveSecs={en.isActive ? liveSecs : undefined}
              onClick={() => onEntryClick(en.sessionId)}
              onContextMenu={
                onEntryDelete
                  ? (e) => {
                      e.preventDefault();
                      setMenu({ x: e.clientX, y: e.clientY, sessionId: en.sessionId });
                    }
                  : undefined
              }
            />
          ))}
        </div>
      ))}
      {menu ? (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          items={[
            {
              label: "Edit…",
              icon: <Pencil size={14} />,
              onClick: () => onEntryClick(menu.sessionId),
            },
            {
              label: "Delete",
              icon: <Trash2 size={14} />,
              danger: true,
              onClick: () => onEntryDelete?.(menu.sessionId),
            },
          ]}
        />
      ) : null}
    </div>
  );
}
