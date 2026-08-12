// Formatting + range helpers. Timestamps are UTC epoch ms; day/week bucketing
// happens in Rust, but the frontend builds the ranges it asks for.

import {
  startOfDay,
  endOfDay,
  startOfWeek,
  endOfWeek,
  startOfMonth,
  endOfMonth,
  startOfYear,
  endOfYear,
  subDays,
} from "date-fns";
import type { Range } from "./types";

/** The user's IANA timezone, passed to every aggregation query. */
export function getTz(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
}

/** Elapsed ms as `H:MM:SS`, unpadded hours (matches the tray title). */
export function formatHMS(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

/** Big hero/chip timer as `HH:MM:SS`, zero-padded hours (e.g. `02:14:23`). */
export function formatTimer(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

/**
 * Duration/total as `H:MM` (hours uncapped, minutes padded) — the app-wide
 * format for totals and entry durations (e.g. `4:21`, `50:33`, `120:21`).
 */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  return `${h}:${String(m).padStart(2, "0")}`;
}

/** The trailing seconds (2-digit) of a duration, for the faded live suffix. */
export function secondsOf(ms: number): string {
  return String(Math.max(0, Math.floor(ms / 1000)) % 60).padStart(2, "0");
}

/** Split a duration into `H:MM` and a 2-digit `SS`, so the seconds can be
 * rendered faded on a live/running total (e.g. `50:33` + `:06`). */
export function formatDurationParts(ms: number): { hm: string; ss: string } {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  return { hm: `${h}:${String(m).padStart(2, "0")}`, ss: String(s).padStart(2, "0") };
}

/** Local clock like `09:24`. */
export function formatClock(ms: number): string {
  const d = new Date(ms);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

export function formatMoney(amount: number, currency = "USD"): string {
  try {
    return new Intl.NumberFormat(undefined, {
      style: "currency",
      currency,
      maximumFractionDigits: amount % 1 === 0 ? 0 : 2,
    }).format(amount);
  } catch {
    return `$${amount.toFixed(2)}`;
  }
}

// --- Range builders --------------------------------------------------------

const ms = (d: Date) => d.getTime();

export function rangeToday(now = new Date()): Range {
  return { from: ms(startOfDay(now)), to: ms(endOfDay(now)) };
}

export function rangeThisWeek(firstDayOfWeek = 1, now = new Date()): Range {
  const opts = { weekStartsOn: firstDayOfWeek as 0 | 1 | 2 | 3 | 4 | 5 | 6 };
  return { from: ms(startOfWeek(now, opts)), to: ms(endOfWeek(now, opts)) };
}

export function rangeThisMonth(now = new Date()): Range {
  return { from: ms(startOfMonth(now)), to: ms(endOfMonth(now)) };
}

export function rangeThisYear(now = new Date()): Range {
  return { from: ms(startOfYear(now)), to: ms(endOfYear(now)) };
}

export function rangeLastNDays(n: number, now = new Date()): Range {
  return { from: ms(startOfDay(subDays(now, n - 1))), to: ms(endOfDay(now)) };
}

export function rangeForDay(day: Date): Range {
  return { from: ms(startOfDay(day)), to: ms(endOfDay(day)) };
}

// --- <input type="datetime-local"> bridging (local time) -------------------

export function toDatetimeLocal(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function fromDatetimeLocal(value: string): number {
  return new Date(value).getTime();
}

export function rangeForWeek(day: Date, firstDayOfWeek = 1): Range {
  const opts = { weekStartsOn: firstDayOfWeek as 0 | 1 | 2 | 3 | 4 | 5 | 6 };
  return { from: ms(startOfWeek(day, opts)), to: ms(endOfWeek(day, opts)) };
}
