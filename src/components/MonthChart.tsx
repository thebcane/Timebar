import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  LabelList,
} from "recharts";
import { format, parseISO } from "date-fns";
import { formatDuration, formatMoney } from "../lib/time";
import type { ReportView } from "../lib/types";

type Mode = "time" | "money";

/** Stacked bar chart, one bar per day, segments colored by task (§6.5). */
export function MonthChart({ report, mode }: { report: ReportView; mode: Mode }) {
  const toVal = (ms: number, rev: number) => (mode === "money" ? rev : ms / 3_600_000);
  const fmt = (v: number) => (mode === "money" ? formatMoney(v) : `${v.toFixed(1)}h`);

  const keyMeta = new Map<string, { name: string; color: string }>();
  for (const d of report.days)
    for (const s of d.segments) keyMeta.set(s.taskId, { name: s.taskName, color: s.color });
  const keys = [...keyMeta.keys()];

  const data = report.days.map((d) => {
    const row: Record<string, number | string> = {
      date: format(parseISO(d.date), "MMM d"),
      __total: toVal(d.totalMs, d.totalRevenue),
    };
    for (const s of d.segments) row[s.taskId] = toVal(s.ms, s.revenue);
    return row;
  });

  if (data.length === 0) {
    return (
      <div className="grid h-[260px] place-items-center text-[13px] text-text-secondary">
        No data in this range.
      </div>
    );
  }

  return (
    <ResponsiveContainer width="100%" height={260}>
      <BarChart data={data} margin={{ top: 22, right: 8, left: 0, bottom: 0 }}>
        <XAxis
          dataKey="date"
          tick={{ fontSize: 11, fill: "var(--text-muted)" }}
          axisLine={false}
          tickLine={false}
        />
        <YAxis
          tick={{ fontSize: 11, fill: "var(--text-muted)" }}
          axisLine={false}
          tickLine={false}
          width={44}
          tickFormatter={(v) => (mode === "money" ? formatMoney(v) : `${v}h`)}
        />
        <Tooltip
          cursor={{ fill: "rgba(255,255,255,0.04)" }}
          contentStyle={{
            background: "var(--panel-raised)",
            border: "1px solid var(--hairline)",
            borderRadius: 10,
            fontSize: 12,
          }}
          labelStyle={{ color: "var(--text-secondary)" }}
          formatter={((value: number, key: string) => [fmt(value), keyMeta.get(key)?.name ?? key]) as never}
        />
        {keys.map((k, i) => (
          <Bar
            key={k}
            dataKey={k}
            stackId="a"
            fill={keyMeta.get(k)!.color}
            radius={i === keys.length - 1 ? [4, 4, 0, 0] : 0}
            isAnimationActive={false}
          >
            {i === keys.length - 1 ? (
              <LabelList
                dataKey="__total"
                position="top"
                formatter={((v: number) =>
                  mode === "money" ? formatMoney(v) : formatDuration(v * 3_600_000)) as never}
                style={{ fontSize: 11, fill: "var(--text-secondary)" }}
              />
            ) : null}
          </Bar>
        ))}
      </BarChart>
    </ResponsiveContainer>
  );
}
