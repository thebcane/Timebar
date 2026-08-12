import { useCallback, useEffect, useMemo, useState } from "react";
import { Search, Maximize2 } from "lucide-react";
import { useStore, useElapsed } from "../store/store";
import { useAppInit } from "../store/useInit";
import * as ipc from "../lib/ipc";
import { flattenTaskTotals, type TaskTotal } from "../lib/tree";
import { PALETTE } from "../lib/colors";
import {
  getTz,
  formatTimer,
  formatDuration,
  formatMoney,
  formatClock,
  rangeToday,
  rangeThisWeek,
  rangeThisMonth,
  rangeLastNDays,
} from "../lib/time";
import type { Task } from "../lib/types";
import { PlayPauseButton, Eyebrow, EmptyState } from "../components/primitives";
import { TaskRow } from "../components/TaskRow";
import { DialogHost } from "../components/DialogHost";

interface PeriodStat {
  ms: number;
  rev: number;
}

interface QuickData {
  tasks: Task[];
  /** taskId -> totals over the same 30-day window the dashboard shows. */
  totalsMap: Map<string, TaskTotal>;
  folderName: Map<string, string>;
  recency: string[];
  today: PeriodStat;
  week: PeriodStat;
  month: PeriodStat;
}

export default function QuickPanel() {
  useAppInit();
  const active = useStore((s) => s.activeSession);
  const toggle = useStore((s) => s.toggle);
  const start = useStore((s) => s.start);
  const notify = useStore((s) => s.notify);
  const dataVersion = useStore((s) => s.dataVersion);
  const nowTick = useStore((s) => s.nowTick);
  const settings = useStore((s) => s.settings);
  const elapsed = useElapsed();

  const [data, setData] = useState<QuickData | null>(null);
  const [loadedAt, setLoadedAt] = useState(Date.now());
  const [query, setQuery] = useState("");
  const [newTaskMode, setNewTaskMode] = useState(false);
  const [newTaskName, setNewTaskName] = useState("");

  const fdow = settings?.firstDayOfWeek ?? 1;

  const load = useCallback(async () => {
    const tz = getTz();
    // Task totals use the dashboard's 30-day window so the two agree; the
    // footer keeps its own today/week/month reports.
    const range30 = rangeLastNDays(30);
    const [tasks, folders, totalsTree, entries30, today, week, month] = await Promise.all([
      ipc.listTasks(false),
      ipc.listFolders(),
      ipc.getTaskTreeWithTotals(range30, tz, false),
      ipc.getTimeEntries(range30, tz),
      ipc.getReport(rangeToday(), tz, "time"),
      ipc.getReport(rangeThisWeek(fdow), tz, "time"),
      ipc.getReport(rangeThisMonth(), tz, "time"),
    ]);

    const folderName = new Map(folders.map((f) => [f.id, f.name]));
    const flat = entries30.flatMap((d) => d.entries).sort((a, b) => b.startedAt - a.startedAt);
    const recency: string[] = [];
    for (const en of flat) if (!recency.includes(en.taskId)) recency.push(en.taskId);

    setData({
      tasks,
      totalsMap: flattenTaskTotals(totalsTree),
      folderName,
      recency,
      today: { ms: today.totalMs, rev: today.totalRevenue },
      week: { ms: week.totalMs, rev: week.totalRevenue },
      month: { ms: month.totalMs, rev: month.totalRevenue },
    });
    setLoadedAt(Date.now());
  }, [fdow]);

  useEffect(() => {
    load();
  }, [load, dataVersion]);

  // Recent tasks: recency order first, then remaining tasks by sort order.
  const recentTasks = useMemo(() => {
    if (!data) return [];
    const byId = new Map(data.tasks.map((t) => [t.id, t]));
    const ordered: Task[] = [];
    for (const id of data.recency) {
      const t = byId.get(id);
      if (t) ordered.push(t);
    }
    for (const t of data.tasks) if (!data.recency.includes(t.id)) ordered.push(t);
    const q = query.trim().toLowerCase();
    return q ? ordered.filter((t) => t.name.toLowerCase().includes(q)) : ordered;
  }, [data, query]);

  const onSelect = async (taskId: string) => {
    await toggle(taskId);
  };

  // The hero button's default action: create a brand-new task and start it.
  const createAndStart = async () => {
    const name = newTaskName.trim();
    if (!name) return;
    try {
      const color = PALETTE[Math.floor(Math.random() * PALETTE.length)].hex;
      const task = await ipc.createTask({ name, color, billable: false });
      await start(task.id);
    } catch (err) {
      notify(`Couldn't create the task:\n${err}`);
    }
    setNewTaskMode(false);
    setNewTaskName("");
  };

  const onHeroClick = () => {
    if (active) toggle(active.taskId);
    else setNewTaskMode(true);
  };

  const onSearchKey = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && recentTasks[0]) onSelect(recentTasks[0].id);
    if (e.key === "Escape") ipc.hideQuick();
  };

  const showEarnings = (s: PeriodStat) => s.rev > 0;

  return (
    <div className="flex h-screen flex-col overflow-hidden rounded-popover border border-hairline bg-panel">
      {/* Hero */}
      <div
        data-tauri-drag-region
        className="px-4 pb-4 pt-4"
        style={{
          background:
            "linear-gradient(160deg, rgba(44,199,196,0.16), rgba(44,199,196,0.04) 45%, transparent 75%)",
        }}
      >
        <div className="flex items-center gap-4">
          <PlayPauseButton
            color="var(--accent)"
            glyph="light"
            active={!!active}
            size={64}
            title={active ? "Stop" : "Start a new task"}
            onClick={onHeroClick}
          />
          <div className="min-w-0 flex-1">
            <div
              className="tabular text-text-primary"
              style={{ fontSize: 46, fontWeight: 700, lineHeight: 1, letterSpacing: "-0.02em" }}
            >
              {formatTimer(elapsed)}
            </div>
          </div>
        </div>

        {/* Task name, new-task input, or idle hint */}
        {active ? (
          <>
            <div className="mt-3 truncate text-[19px] font-bold text-text-primary">{active.taskName}</div>
            <div className="mt-2 flex items-stretch gap-3">
              <div className="w-12 shrink-0 text-right leading-tight">
                <div className="tabular text-[13px] text-text-secondary">{formatClock(active.startedAt)}</div>
                <div className="text-[10px] font-semibold tracking-wide text-text-muted">NOW</div>
              </div>
              <div className="w-[3px] rounded bg-accent" />
              <div className="min-w-0 flex-1 self-center text-[14px] text-text-primary">
                {active.note ?? active.folderName ?? "Running"}
              </div>
            </div>
          </>
        ) : newTaskMode ? (
          <input
            autoFocus
            value={newTaskName}
            onChange={(e) => setNewTaskName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") createAndStart();
              if (e.key === "Escape") {
                setNewTaskMode(false);
                setNewTaskName("");
              }
            }}
            onBlur={() => {
              if (!newTaskName.trim()) setNewTaskMode(false);
            }}
            placeholder="Name your task, then Enter…"
            className="mt-3 w-full rounded-pill border border-accent/50 bg-panel-raised px-3 py-2 text-[16px] font-semibold text-text-primary outline-none placeholder:font-normal placeholder:text-text-muted"
          />
        ) : (
          <button
            type="button"
            onClick={() => setNewTaskMode(true)}
            className="mt-3 block truncate text-[19px] font-bold text-text-secondary hover:text-text-primary"
          >
            Start a new task…
          </button>
        )}
      </div>

      {/* Search */}
      <div className="px-4 pb-2">
        <div className="flex items-center gap-2 rounded-pill bg-panel-raised px-3 py-2">
          <Search size={15} className="text-text-muted" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onSearchKey}
            placeholder="Search tasks…"
            className="w-full bg-transparent text-[13px] text-text-primary outline-none placeholder:text-text-muted"
          />
        </div>
      </div>

      {/* Recent tasks */}
      <div className="min-h-0 flex-1 overflow-y-auto px-3 pb-2">
        <Eyebrow className="px-1 py-1.5">Recent tasks</Eyebrow>
        {recentTasks.length === 0 ? (
          <EmptyState>No tasks yet. Add one from the dashboard.</EmptyState>
        ) : (
          recentTasks.map((t) => {
            const base = data?.totalsMap.get(t.id)?.ms ?? 0;
            const isActive = active?.taskId === t.id;
            // Extend the running task's total live so seconds tick and carry
            // across pause/resume (the DB already stores second precision).
            const totalMs = isActive ? base + Math.max(0, nowTick - loadedAt) : base;
            return (
              <TaskRow
                key={t.id}
                name={t.name}
                color={t.color}
                folderName={t.folderId ? data?.folderName.get(t.folderId) : null}
                totalMs={totalMs}
                active={isActive}
                onToggle={(e) => {
                  e.stopPropagation();
                  onSelect(t.id);
                }}
                onClick={() => onSelect(t.id)}
              />
            );
          })
        )}
      </div>

      {/* Footer stats */}
      <div className="relative border-t border-hairline px-4 py-3">
        <div className="grid grid-cols-3 gap-2">
          {(
            [
              ["Today", data?.today],
              ["This week", data?.week],
              ["This month", data?.month],
            ] as const
          ).map(([label, stat]) => (
            <div key={label}>
              <Eyebrow>{label}</Eyebrow>
              <div className="tabular mt-1 text-[16px] font-semibold text-text-primary">
                {formatDuration(stat?.ms ?? 0)}
              </div>
              {stat && showEarnings(stat) ? (
                <div className="tabular text-[11px] text-text-muted">{formatMoney(stat.rev)}</div>
              ) : null}
            </div>
          ))}
        </div>
        <button
          type="button"
          title="Open dashboard"
          onClick={() => ipc.openDashboard()}
          className="absolute -top-4 right-3 grid h-8 w-8 place-items-center rounded-full border border-hairline bg-panel-raised text-text-secondary hover:text-text-primary"
        >
          <Maximize2 size={14} />
        </button>
      </div>

      <DialogHost />
    </div>
  );
}
