import { useCallback, useEffect, useMemo, useState } from "react";
import { Share, DollarSign, Clock } from "lucide-react";
import { useStore } from "../store/store";
import * as ipc from "../lib/ipc";
import {
  getTz,
  formatDuration,
  formatMoney,
  rangeToday,
  rangeThisWeek,
  rangeThisMonth,
  rangeThisYear,
  fromDatetimeLocal,
} from "../lib/time";
import type { DayGroup, Range, ReportTaskTotal, ReportView, TreeNode } from "../lib/types";
import { MonthChart } from "../components/MonthChart";
import { EntriesFeed } from "../components/EntriesFeed";
import { Eyebrow, ColorDot } from "../components/primitives";
import { EntryForm } from "../components/EntryForm";
import type { Task } from "../lib/types";

type Scope = "day" | "week" | "month" | "year" | "custom";
type Mode = "time" | "money";
const SCOPES: Scope[] = ["day", "week", "month", "year", "custom"];

export default function Reports() {
  const dataVersion = useStore((s) => s.dataVersion);
  const active = useStore((s) => s.activeSession);
  const settings = useStore((s) => s.settings);
  const bump = useStore((s) => s.bump);
  const requestConfirm = useStore((s) => s.requestConfirm);
  const notify = useStore((s) => s.notify);
  const fdow = settings?.firstDayOfWeek ?? 1;

  const [scope, setScope] = useState<Scope>("month");
  const [mode, setMode] = useState<Mode>("time");
  const [customFrom, setCustomFrom] = useState("");
  const [customTo, setCustomTo] = useState("");
  const [report, setReport] = useState<ReportView | null>(null);
  const [days, setDays] = useState<DayGroup[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [folders, setFolders] = useState<{ node: TreeNode; depth: number }[]>([]);
  const [editing, setEditing] = useState<string | null>(null);

  const range: Range = useMemo(() => {
    switch (scope) {
      case "day":
        return rangeToday();
      case "week":
        return rangeThisWeek(fdow);
      case "year":
        return rangeThisYear();
      case "custom":
        if (customFrom && customTo)
          return { from: fromDatetimeLocal(customFrom + "T00:00"), to: fromDatetimeLocal(customTo + "T23:59") };
        return rangeThisMonth();
      case "month":
      default:
        return rangeThisMonth();
    }
  }, [scope, fdow, customFrom, customTo]);

  const load = useCallback(async () => {
    const tz = getTz();
    const [rep, entries, allTasks, tree] = await Promise.all([
      ipc.getReport(range, tz, mode),
      ipc.getTimeEntries(range, tz),
      ipc.listTasks(true),
      ipc.getTaskTreeWithTotals(range, tz, true),
    ]);
    setReport(rep);
    setDays(entries);
    setTasks(allTasks);
    // All folders (nested included) with rolled-up totals, kept in tree order
    // with a depth for indentation.
    const flat: { node: TreeNode; depth: number }[] = [];
    const walk = (nodes: TreeNode[], depth: number) => {
      for (const n of nodes) {
        if (n.kind !== "folder") continue;
        if (n.totalMs > 0) flat.push({ node: n, depth });
        walk(n.children, depth + 1);
      }
    };
    walk(tree, 0);
    setFolders(flat);
  }, [range, mode]);

  useEffect(() => {
    load();
  }, [load, dataVersion, active?.id]);

  const val = (ms: number, rev: number) => (mode === "money" ? formatMoney(rev) : formatDuration(ms));

  const exportCsv = async () => {
    const path = await ipc.exportCsv(range, getTz());
    notify(`Exported to:\n${path}`);
  };

  const SidebarRow = ({ t }: { t: ReportTaskTotal }) => (
    <div className="flex items-center gap-2 px-1 py-1.5">
      <ColorDot color={t.color} />
      <span className="min-w-0 flex-1 truncate text-[13px] text-text-primary">{t.name}</span>
      <span className="tabular text-[13px] font-medium text-text-secondary">{val(t.ms, t.revenue)}</span>
    </div>
  );

  return (
    <div className="flex h-full">
      {/* Main */}
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex items-center justify-between gap-3 px-6 pb-3 pt-4">
          <div data-tauri-drag-region className="flex flex-1 items-center gap-2">
            <div className="flex gap-1 rounded-pill bg-panel-raised p-1">
              {SCOPES.map((s) => (
                <button
                  key={s}
                  type="button"
                  onClick={() => setScope(s)}
                  className={`rounded-pill px-3 py-1 text-[12px] font-medium capitalize transition-colors ${
                    scope === s ? "bg-panel text-text-primary" : "text-text-secondary"
                  }`}
                >
                  {s}
                </button>
              ))}
            </div>
            {scope === "custom" ? (
              <div className="flex items-center gap-1">
                <input
                  type="date"
                  value={customFrom}
                  onChange={(e) => setCustomFrom(e.target.value)}
                  className="rounded-pill border border-hairline bg-panel-raised px-2 py-1 text-[12px] text-text-primary"
                />
                <span className="text-text-muted">–</span>
                <input
                  type="date"
                  value={customTo}
                  onChange={(e) => setCustomTo(e.target.value)}
                  className="rounded-pill border border-hairline bg-panel-raised px-2 py-1 text-[12px] text-text-primary"
                />
              </div>
            ) : null}
          </div>
          <div className="flex items-center gap-2">
            <div className="flex gap-1 rounded-pill bg-panel-raised p-1">
              <button
                type="button"
                onClick={() => setMode("money")}
                className={`grid h-7 w-7 place-items-center rounded-full ${
                  mode === "money" ? "bg-accent-grad text-white" : "text-text-secondary"
                }`}
                title="Show money"
              >
                <DollarSign size={14} />
              </button>
              <button
                type="button"
                onClick={() => setMode("time")}
                className={`grid h-7 w-7 place-items-center rounded-full ${
                  mode === "time" ? "bg-accent-grad text-white" : "text-text-secondary"
                }`}
                title="Show time"
              >
                <Clock size={14} />
              </button>
            </div>
            <button
              type="button"
              onClick={exportCsv}
              title="Export CSV"
              className="grid h-8 w-8 place-items-center rounded-full bg-panel-raised text-text-secondary hover:text-text-primary"
            >
              <Share size={15} />
            </button>
          </div>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-6">
          <div className="rounded-panel border border-hairline bg-panel p-3">
            {report ? <MonthChart report={report} mode={mode} /> : null}
          </div>
          <div className="mt-5">
            <EntriesFeed
              days={days}
              onEntryClick={(sid) => setEditing(sid)}
              onEntryDelete={async (sid) => {
                if (!(await requestConfirm("Delete this entry?"))) return;
                await ipc.deleteSession(sid);
                bump();
              }}
            />
          </div>
        </div>
      </div>

      {/* Right sidebar */}
      <div className="flex w-[260px] shrink-0 flex-col overflow-y-auto border-l border-hairline px-3 py-4">
        {folders.length ? (
          <>
            <Eyebrow className="px-1 pb-1">Folders</Eyebrow>
            {folders.map(({ node: f, depth }) => (
              <div
                key={f.id}
                className="flex items-center gap-2 py-1.5 pr-1"
                style={{ paddingLeft: 4 + depth * 16 }}
              >
                <ColorDot color={f.color} />
                <span
                  className={`min-w-0 flex-1 truncate text-[13px] ${
                    depth === 0 ? "font-semibold text-text-primary" : "font-medium text-text-secondary"
                  }`}
                >
                  {f.name}
                </span>
                <span
                  className={`tabular text-[13px] ${
                    depth === 0 ? "font-semibold text-text-primary" : "font-medium text-text-secondary"
                  }`}
                >
                  {val(f.totalMs, f.totalRevenue)}
                </span>
              </div>
            ))}
            <div className="mt-4" />
          </>
        ) : null}

        <Eyebrow className="px-1 pb-1">Tasks</Eyebrow>
        {report?.tasks.length ? report.tasks.map((t) => <SidebarRow key={t.id} t={t} />) : (
          <div className="px-1 py-1 text-[12px] text-text-muted">No time in range.</div>
        )}

        {report?.archived.length ? (
          <>
            <Eyebrow className="mt-4 px-1 pb-1">Archived</Eyebrow>
            {report.archived.map((t) => (
              <SidebarRow key={t.id} t={t} />
            ))}
          </>
        ) : null}

        <Eyebrow className="mt-4 px-1 pb-1">Billing</Eyebrow>
        <div className="flex items-center justify-between px-1 py-1.5">
          <span className="text-[13px] text-text-primary">Paid</span>
          <span className="tabular text-[13px] font-medium text-text-secondary">
            {mode === "money"
              ? formatMoney(report?.billingPaidRevenue ?? 0)
              : formatDuration(report?.billingPaidMs ?? 0)}
          </span>
        </div>

        <div className="mt-auto rounded-panel bg-panel-raised p-3">
          <Eyebrow className="pb-2">Summary</Eyebrow>
          <div className="flex items-baseline justify-between">
            <span className="text-[13px] text-text-secondary">Total time</span>
            <span className="tabular text-[15px] font-semibold text-text-primary">
              {formatDuration(report?.totalMs ?? 0)}
            </span>
          </div>
          <div className="mt-1 flex items-baseline justify-between">
            <span className="text-[13px] text-text-secondary">Total earnings</span>
            <span className="tabular text-[15px] font-semibold text-accent-bright">
              {formatMoney(report?.totalRevenue ?? 0)}
            </span>
          </div>
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
