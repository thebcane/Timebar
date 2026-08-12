import { useCallback, useEffect, useMemo, useState } from "react";
import { ChevronLeft, ChevronRight, Circle, Plus, Minus, Share } from "lucide-react";
import { addDays, addWeeks, startOfWeek, format } from "date-fns";
import { useStore } from "../store/store";
import * as ipc from "../lib/ipc";
import {
  getTz,
  formatDuration,
  formatMoney,
  rangeForDay,
  rangeForWeek,
} from "../lib/time";
import type { Range, TimelineBlock, Task } from "../lib/types";
import { TimelineGrid, type TimelineColumn } from "../components/TimelineGrid";
import { EntryForm } from "../components/EntryForm";

type Mode = "day" | "week";

export default function Timeline() {
  const dataVersion = useStore((s) => s.dataVersion);
  const settings = useStore((s) => s.settings);
  const bump = useStore((s) => s.bump);
  const notify = useStore((s) => s.notify);
  const fdow = settings?.firstDayOfWeek ?? 1;

  const [mode, setMode] = useState<Mode>("day");
  const [anchor, setAnchor] = useState<Date>(() => new Date());
  const [pxPerHour, setPxPerHour] = useState(52);
  const [blocks, setBlocks] = useState<TimelineBlock[]>([]);
  const [totals, setTotals] = useState({ ms: 0, rev: 0 });
  const [tasks, setTasks] = useState<Task[]>([]);
  const [editing, setEditing] = useState<string | null>(null);

  const range: Range = useMemo(
    () => (mode === "day" ? rangeForDay(anchor) : rangeForWeek(anchor, fdow)),
    [mode, anchor, fdow],
  );

  const load = useCallback(async () => {
    const tz = getTz();
    const [tl, rep, allTasks] = await Promise.all([
      ipc.getTimeline(range, tz),
      ipc.getReport(range, tz, "time"),
      ipc.listTasks(true),
    ]);
    setBlocks(tl);
    setTotals({ ms: rep.totalMs, rev: rep.totalRevenue });
    setTasks(allTasks);
  }, [range]);

  useEffect(() => {
    load();
  }, [load, dataVersion]);

  const [minHour, maxHour] = useMemo(() => {
    // Default to a full working day so the time-of-day context is always
    // visible; expand only if there's activity earlier/later.
    let lo = 7;
    let hi = 21;
    if (blocks.length) {
      lo = Math.min(lo, Math.floor(Math.min(...blocks.map((b) => b.startMin)) / 60));
      hi = Math.max(hi, Math.ceil(Math.max(...blocks.map((b) => b.endMin)) / 60));
    }
    return [Math.max(0, lo), Math.min(24, hi)];
  }, [blocks]);

  const columns: TimelineColumn[] = useMemo(() => {
    if (mode === "day") {
      const date = format(anchor, "yyyy-MM-dd");
      return [{ key: date, date, blocks: blocks.filter((b) => b.date === date) }];
    }
    const start = startOfWeek(anchor, { weekStartsOn: fdow as 0 | 1 | 2 | 3 | 4 | 5 | 6 });
    return Array.from({ length: 7 }, (_, i) => {
      const day = addDays(start, i);
      const date = format(day, "yyyy-MM-dd");
      return { key: date, label: format(day, "EEE d"), date, blocks: blocks.filter((b) => b.date === date) };
    });
  }, [mode, anchor, fdow, blocks]);

  const step = (dir: number) =>
    setAnchor((a) => (mode === "day" ? addDays(a, dir) : addWeeks(a, dir)));

  const title = useMemo(() => {
    if (mode === "day") return format(anchor, "d MMMM yyyy");
    const start = startOfWeek(anchor, { weekStartsOn: fdow as 0 | 1 | 2 | 3 | 4 | 5 | 6 });
    const end = addDays(start, 6);
    return `${format(start, "d MMMM yyyy")} – ${format(end, "d MMMM yyyy")}`;
  }, [mode, anchor, fdow]);

  return (
    <div className="flex h-full flex-col">
      {/* Controls */}
      <div className="flex items-center justify-between px-6 pb-2 pt-4" data-tauri-drag-region>
        <div className="flex gap-1 rounded-pill bg-panel-raised p-1">
          {(["day", "week"] as const).map((m) => (
            <button
              key={m}
              type="button"
              onClick={() => setMode(m)}
              className={`rounded-pill px-3 py-1 text-[12px] font-medium capitalize ${
                mode === m ? "bg-panel text-text-primary" : "text-text-secondary"
              }`}
            >
              {m}
            </button>
          ))}
        </div>

        <div className="flex items-center gap-3">
          <button type="button" onClick={() => step(-1)} className="text-text-secondary hover:text-text-primary">
            <ChevronLeft size={18} />
          </button>
          <button
            type="button"
            onClick={() => setAnchor(new Date())}
            title="Jump to today"
            className="text-text-secondary hover:text-accent"
          >
            <Circle size={9} fill="currentColor" />
          </button>
          <button type="button" onClick={() => step(1)} className="text-text-secondary hover:text-text-primary">
            <ChevronRight size={18} />
          </button>
          <span className="text-[17px] font-semibold text-text-primary">{title}</span>
        </div>

        <div className="flex items-center gap-3">
          <span className="tabular text-[14px] text-text-secondary">
            {formatMoney(totals.rev)} <span className="text-text-muted">|</span>{" "}
            <span className="font-semibold text-text-primary">{formatDuration(totals.ms)}</span>
          </span>
          <button
            type="button"
            title="Export CSV"
            onClick={async () => notify(`Exported to:\n${await ipc.exportCsv(range, getTz())}`)}
            className="grid h-8 w-8 place-items-center rounded-full bg-panel-raised text-text-secondary hover:text-text-primary"
          >
            <Share size={15} />
          </button>
        </div>
      </div>

      <div className="relative flex min-h-0 flex-1 flex-col">
        <TimelineGrid
          columns={columns}
          minHour={minHour}
          maxHour={maxHour}
          pxPerHour={pxPerHour}
          onBlockClick={(sid) => setEditing(sid)}
        />
        {/* Zoom controls */}
        <div className="absolute bottom-3 left-3 flex flex-col gap-1">
          <button
            type="button"
            onClick={() => setPxPerHour((p) => Math.min(140, p + 12))}
            className="grid h-7 w-7 place-items-center rounded-md border border-hairline bg-panel-raised text-text-secondary hover:text-text-primary"
          >
            <Plus size={14} />
          </button>
          <button
            type="button"
            onClick={() => setPxPerHour((p) => Math.max(28, p - 12))}
            className="grid h-7 w-7 place-items-center rounded-md border border-hairline bg-panel-raised text-text-secondary hover:text-text-primary"
          >
            <Minus size={14} />
          </button>
        </div>
      </div>

      {editing ? (
        <EntryForm
          tasks={tasks.filter((t) => !t.archived)}
          sessionId={editing}
          onClose={() => setEditing(null)}
          onSaved={bump}
        />
      ) : null}
    </div>
  );
}
