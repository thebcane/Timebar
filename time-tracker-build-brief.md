# Build Brief: Menu-Bar Time Tracker (working title: TimeBar)

A macOS menu-bar time tracking app inspired by Timemator. This brief is the single
source of truth for an autonomous coding agent. Build it in the order given.
Rename "TimeBar" throughout to whatever the owner chooses.

---

## 1. What we are building

A native-feeling Mac menu-bar utility for **manual** time tracking. The core loop:

1. Start and stop tasks instantly from the macOS menu bar.
2. See recent tasks with one-tap play/pause and running totals.
3. Review everything in a dashboard: a task tree organized into folders, a
   day-grouped list of entries, a day/week calendar timeline, and reports with a
   monthly chart and billing totals.

The app is local-first. All data lives in a local SQLite database. Optional
cloud sync is a later milestone (see Section 12), so the schema is designed to be
sync-friendly from the start (string UUID ids, `updated_at` on every row).

### In scope for v1
- Menu-bar tray with a live running timer in the tray title.
- Quick panel (popover-style window) anchored under the tray icon.
- Main window with three sections: Tracking, Timeline (Day/Week), Reports.
- Folders, tasks, sessions, manual entry, editing, notes, per-task hourly rates,
  billable flags, and revenue math.
- Time and money totals across day / week / month / custom ranges.
- CSV export of entries.

### Explicitly NOT in scope for v1 (do not build these)
- **Automatic activity tracking** (the "Activities" column in the reference
  screenshots that shows tracked apps, files, and URLs). This is the hard macOS
  Accessibility/AppleScript engine and the owner does not want it yet. Where a
  reference screenshot shows an Activities column next to Tracked Time Entries,
  build only the Tracked Time Entries side. Leave a clean seam so an
  `activity_events` source can be added later (see Section 12).
- iOS, iPadOS, and watchOS apps.
- PDF invoices. CSV export only in v1.
- Cloud sync, accounts, multi-user, teams.
- Idle detection / auto-pause.

---

## 2. Tech stack (use exactly this)

- **Tauri v2** (Rust core + web frontend). Chosen for a real menu-bar tray with a
  live title, a native window for review, small bundle, and a frontend that ports
  directly if the app ever moves.
- **Frontend:** React 18 + TypeScript + Vite.
- **Styling:** Tailwind CSS. All colors come from the token set in Section 9,
  defined as CSS variables and mapped into `tailwind.config` so nothing is
  hard-coded.
- **State:** Zustand for UI/app state. SQLite is the source of truth; the store
  holds the loaded working set and the active-session pointer, and re-queries on
  mutation. Do not add TanStack Query; the data layer is local and synchronous
  enough that a thin store plus explicit reloads is simpler.
- **Database:** SQLite via `tauri-plugin-sql` (sqlite feature). Migrations live in
  the plugin's migration list, versioned and append-only.
- **Tray window positioning:** `tauri-plugin-positioner` (TrayCenter anchor) plus
  tray click handling to toggle the quick panel and hide it on blur.
- **Charts:** Recharts (stacked bar chart for the monthly report).
- **Dates:** `date-fns` on the frontend. Store timestamps as UTC epoch
  milliseconds (integer). Group into local days in queries/UI using the user's
  local timezone.
- **IDs:** UUID v4 strings (`uuid` crate on the Rust side, generated at insert).
- **Icons:** `lucide-react`.

Rust crates: `tauri`, `tauri-plugin-sql`, `tauri-plugin-positioner`, `uuid`,
`serde`, `serde_json`, `chrono`, `tokio`.

---

## 3. Architecture

```
┌──────────────────────────────────────────────────────────┐
│  Rust core (Tauri)                                        │
│   • SQLite (tauri-plugin-sql) — source of truth           │
│   • Tray icon + live title updater (1s loop while active) │
│   • IPC commands (Section 8)                              │
│   • Emits `active-session-changed` events to frontend     │
└──────────────────────────────────────────────────────────┘
        ▲  IPC (invoke)            │  events
        │                          ▼
┌──────────────────────────────────────────────────────────┐
│  React frontend                                           │
│   • Quick panel window (borderless, always-on-top)        │
│   • Main window (Tracking / Timeline / Reports)           │
│   • Zustand store: tasks, folders, activeSession, ticks   │
└──────────────────────────────────────────────────────────┘
```

Two windows, one shared frontend build:
- **`quick`**: small (approx 360 x 620), `decorations: false`, `transparent`
  optional, `alwaysOnTop: true`, `skipTaskbar: true`, hidden on launch, hidden on
  blur. Shown/positioned under the tray on tray click.
- **`main`**: standard window (approx 1200 x 820, resizable), the dashboard. Opened
  from the expand button in the quick panel and from a tray menu item.

The app is an accessory app (no Dock icon by default): set
`"macOSPrivateApi"` as needed and the activation policy to Accessory so it lives in
the menu bar. Provide a tray menu item to quit and to open the main window.

### The live timer (signature behavior)
There is at most **one active session** at any time. When a session is active:
- The Rust core runs a 1-second loop that sets the tray title to the running
  elapsed time formatted `H:MM:SS` (e.g. `2:14:12`), with a small pause glyph icon.
- The frontend independently ticks a 1s interval to render the same elapsed time in
  the quick panel hero and the bottom-left chip, computed as `now - started_at`.
- Source of truth is always `started_at` in the DB, never an in-memory counter, so
  restarts and sleep never drift.

When no session is active, the tray shows the icon only (no title, or `0:00`
if the owner prefers; default to icon only).

---

## 4. Domain model and rules

**Entities:** Folder, Task, Session, Setting.

- A **Task** belongs to zero or one Folder (`folder_id` null means top level).
- A **Folder** may nest under one parent Folder (`parent_id`). Support up to two
  levels of nesting. Folders contain tasks and, optionally, subfolders.
- A **Session** is one continuous tracked interval on one task:
  `started_at`, `ended_at` (null while running), optional `note`.
- **Duration** of a session = `ended_at - started_at`. For the active session,
  duration is computed live as `now - started_at`.
- **Pause/resume:** pausing the active session sets its `ended_at = now`. Resuming a
  task creates a **new** session. A day therefore accumulates multiple sessions per
  task; totals are the sum.
- **Single active session:** starting task B while task A is running first stops A
  (`ended_at = now`), then starts B. Enforce this in the `start_task` command
  atomically.
- **Rate snapshot:** on session creation, copy the task's current `hourly_rate` into
  `session.rate_snapshot`. Revenue for a session = `(duration_hours) *
  rate_snapshot` when the task is billable. Editing a task's rate later must not
  rewrite historical earnings.
- **Manual entry:** a session created with explicit `started_at`/`ended_at` and an
  optional note, not via the live timer.
- **Archive vs delete:** archiving hides a task/folder from active lists but keeps
  its sessions in reports (see the ARCHIVED section in the report sidebar). Deleting
  a task requires confirmation and cascade-deletes its sessions.

### Edge cases the agent must handle
- **Crash/quit while running:** on launch, if a session has `ended_at IS NULL`, keep
  it running (do not close or discard it). The live timer resumes from
  `started_at`. Persist an `active_session_id` in settings as a fast lookup, but the
  null-`ended_at` row is authoritative.
- **System sleep:** duration is wall-clock, so time during sleep counts. This is
  intended for v1 (idle detection is out of scope).
- **Midnight crossing:** a session that spans midnight must be attributed correctly
  per day in day-grouped totals and in the day/week timeline. Split by clamping to
  local-day boundaries in the query/aggregation layer; do not store split rows.
- **Timezone:** store UTC epoch ms; compute day buckets in the user's local
  timezone. A `first_day_of_week` setting controls week grouping (default Monday).

---

## 5. Data model (SQLite DDL)

Use these as the initial migration. Timestamps are integer UTC epoch milliseconds.
Colors are hex strings from the task palette in Section 9.

```sql
CREATE TABLE folders (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  color       TEXT NOT NULL,
  parent_id   TEXT REFERENCES folders(id) ON DELETE SET NULL,
  sort_order  INTEGER NOT NULL DEFAULT 0,
  archived    INTEGER NOT NULL DEFAULT 0,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE TABLE tasks (
  id           TEXT PRIMARY KEY,
  name         TEXT NOT NULL,
  color        TEXT NOT NULL,
  folder_id    TEXT REFERENCES folders(id) ON DELETE SET NULL,
  hourly_rate  REAL NOT NULL DEFAULT 0,
  currency     TEXT NOT NULL DEFAULT 'USD',
  billable     INTEGER NOT NULL DEFAULT 0,
  sort_order   INTEGER NOT NULL DEFAULT 0,
  archived     INTEGER NOT NULL DEFAULT 0,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);

CREATE TABLE sessions (
  id            TEXT PRIMARY KEY,
  task_id       TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  started_at    INTEGER NOT NULL,
  ended_at      INTEGER,                      -- NULL = running
  note          TEXT,
  rate_snapshot REAL NOT NULL DEFAULT 0,
  billed        INTEGER NOT NULL DEFAULT 0,   -- marked paid/billed
  source        TEXT NOT NULL DEFAULT 'timer',-- 'timer' | 'manual'
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL
);

CREATE INDEX idx_sessions_task    ON sessions(task_id);
CREATE INDEX idx_sessions_started ON sessions(started_at);
CREATE INDEX idx_sessions_running ON sessions(ended_at) WHERE ended_at IS NULL;

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
-- seed: first_day_of_week=1, default_currency=USD, active_session_id=(null),
--       theme=dark
```

Seed a small sample dataset on first run (a couple of folders, several tasks with
varied colors, and a week of sessions) so the UI is not empty during development.

---

## 6. Surfaces and layout

Reference screenshots provided by the owner show Timemator. Match structure and
feel, not pixel-for-pixel. Four surfaces.

### 6.1 Tray (menu bar)
- Icon plus live title `H:MM:SS` when a session runs; icon only when idle.
- Left click: toggle the quick panel, positioned centered under the tray icon.
- Right click (or a small menu): "Open Dashboard", "Quit".

### 6.2 Quick panel (popover window)
Top to bottom:
- **Hero:** large circular play/pause button on the left, big live timer on the
  right (tabular figures), the current task name below, and a mini session row
  showing the session start time, a vertical teal thread, and the current session's
  note or a short code. When idle, the hero shows `0:00`, a play button, and "No
  task running".
- **Search field** that filters the task list by name.
- **RECENT TASKS** (eyebrow label): a list of the most recently used tasks. Each
  row: a colored circular play/pause button, an optional folder eyebrow (e.g.
  "CLIENT WORK"), the task name, and the task's running total for the current period
  on the right. Clicking the button toggles start/stop for that task. The active
  task's row is highlighted with the teal active treatment and shows a pause glyph.
- **Footer stats:** three columns, TODAY / THIS WEEK / THIS MONTH. Each shows total
  time (large) and earnings (muted, below). Earnings hidden if no billable time.
- **Expand button** (bottom-right) opens the main window.

Behavior: selecting a task from the list or search starts it immediately (stopping
any running one first). The panel hides on blur.

### 6.3 Main window: Tracking
Left rail with three items and icons: **Tracking**, **Timeline**, **Reports**.
Bottom of the rail shows the persistent running-timer chip (teal) with elapsed time
and task name, visible in all three sections.

Tracking is two columns:
- **Left — task tree.** Tabs across the top: All Tasks / Recent / Archive. A "+"
  button adds a task or folder. Rows show a colored play/pause button, the task
  name, and its total for the selected scope on the right. Folders are collapsible
  and show a rolled-up total; expanding reveals child tasks (indented) and
  subfolders. The active task row uses the teal active treatment. Support drag to
  reorder and drag a task into a folder.
- **Right — Time Entries.** Header "Time Entries" with a scope subtitle (default
  "last 30 days"), a "+" to add a manual entry, and a share/export icon. Entries are
  grouped by day. Each day has a header with the weekday and date, the day's total
  earnings (muted), and the day's total time. Each entry row shows: start and end
  time stacked, a colored left thread, the task name with an optional folder
  eyebrow, an optional note, a note/document icon, a billable dollar amount when
  applicable, and the entry duration on the right. Clicking an entry opens an edit
  affordance (times, task reassignment, note, billable toggle, delete).

### 6.4 Main window: Timeline
Top controls: Day / Week toggle, date navigation (`<  ●  >` where ● jumps to
today), the current date or week range as a title, and on the right the period
earnings and total time.

- **Day mode:** a single-column vertical day calendar with an hour axis on the left
  (e.g. 09:00 to 20:00, zoomable). Sessions render as colored blocks positioned by
  start and end time, showing the task name, note, and duration. (In the reference
  this sits to the right of an Activities column; that column is out of scope, so
  Day mode is a single column of tracked sessions.) Blocks are click-to-edit and
  drag-to-move / resize to change times.
- **Week mode:** a seven-column grid (one per day, honoring `first_day_of_week`),
  shared hour axis, colored session blocks placed by day and time with task name,
  duration, and note. Zoom in/out with +/- controls. Week navigation.

### 6.5 Main window: Reports
Top controls: Day / Week / Month / Year / Custom scope tabs, and a `$` / clock
toggle to switch chart and totals between money and time. Share/export icon.

- **Chart:** a stacked bar chart, one bar per day in the period. Each bar's segments
  are colored by task, stacked to the day's total; the value on top is the day total
  (time or money per the toggle). Use the task palette colors for segments.
- **Entries list:** below the chart, the same day-grouped entries feed as Tracking,
  scoped to the period.
- **Right sidebar:**
  - **TASKS:** every task with its total for the period, folders rolling up their
    children, expandable. Each row shows a color chip and the total.
  - **ARCHIVED:** archived tasks/folders that still have time in the period.
  - **BILLING:** a "Paid" total (sessions with `billed = 1`).
  - **SUMMARY:** the period's grand total money and time, pinned at the bottom.

---

## 7. Interactions and behaviors

- Starting/stopping is optimistic in the UI, then confirmed by the backend event.
- The active-session state is broadcast via a Tauri event `active-session-changed`;
  both windows subscribe and update the chip, tray, and any highlighted rows.
- Search in the quick panel filters live; Enter starts the top result.
- Manual entry form: task picker, start and end date/time, optional note, billable
  inherited from the task but overridable. Validate `ended_at > started_at`.
- Editing an entry's task reassigns `task_id` and re-snapshots the rate only if the
  user explicitly chooses to; default is to keep the original `rate_snapshot`.
- Keyboard: global shortcut to toggle the quick panel (make it configurable; default
  to a sensible combo). Escape hides the quick panel.
- Empty states use directive copy, not mood copy. Example for an empty task list:
  "No tasks yet. Add one with +." Example for the entries feed with no data in
  range: "No entries in this range. Start a task from the menu bar."

---

## 8. IPC commands (Tauri, Rust)

Name and shape them exactly so the frontend contract is stable. All return
`Result<T, String>`; all mutations set `updated_at = now`.

**Tasks & folders**
- `list_folders() -> Vec<Folder>`
- `create_folder(name, color, parent_id?) -> Folder`
- `update_folder(id, patch) -> Folder`
- `archive_folder(id, archived: bool) -> ()`
- `list_tasks(include_archived: bool) -> Vec<Task>`
- `create_task(name, color, folder_id?, hourly_rate?, currency?, billable?) -> Task`
- `update_task(id, patch) -> Task`
- `archive_task(id, archived: bool) -> ()`
- `delete_task(id) -> ()`  // cascade sessions, requires FE confirmation
- `reorder(entity, ordered_ids: Vec<String>) -> ()`

**Sessions & timer**
- `get_active_session() -> Option<SessionView>`
- `start_task(task_id) -> SessionView`  // stops any running session first, atomically
- `stop_active() -> Option<SessionView>`
- `create_manual_session(task_id, started_at, ended_at, note?, billable?) -> SessionView`
- `update_session(id, patch) -> SessionView`
- `delete_session(id) -> ()`

**Queries / aggregation** (all take a range `{ from: epoch_ms, to: epoch_ms }` in
UTC and a local tz offset or IANA tz string for correct day bucketing)
- `get_time_entries(range, tz) -> Vec<DayGroup>`  // grouped by local day, each with entries + day totals
- `get_task_tree_with_totals(range, tz, include_archived) -> Vec<TreeNode>`  // folders/tasks with rolled-up totals
- `get_report(scope, anchor_date, tz, mode: "time" | "money") -> ReportView`  // chart series + sidebar totals + summary
- `get_timeline(range, tz) -> Vec<TimelineBlock>`  // sessions with start/end for calendar rendering, midnight-split for per-day

**Settings**
- `get_settings() -> Settings`
- `update_setting(key, value) -> ()`

**Export**
- `export_csv(range, tz) -> String (path)`  // writes CSV of entries, returns file path

`SessionView` includes the joined task name, color, folder eyebrow, computed
duration (with `ended_at` defaulting to now when null), and computed revenue.

Aggregation lives in Rust/SQL, not the frontend, so totals stay consistent and
midnight-splitting is done once. The month chart's per-day-per-task stacks come from
a single grouped query.

---

## 9. Design system (Timemator-inspired dark theme)

Define these as CSS variables and map into Tailwind. Do not invent other colors.

### Surfaces
```
--app-bg:        #0E1116   /* window background, near-black slate */
--panel:         #151A21   /* primary panels */
--panel-raised:  #1C222B   /* rows, cards, inputs */
--sidebar:       #101826   /* left rail, subtle vertical gradient to #0C121C */
--hairline:      rgba(255,255,255,0.06)
--overlay:       rgba(0,0,0,0.35)
```

### Text
```
--text-primary:   #F2F5F8
--text-secondary: #8A93A0
--text-muted:     #5A626E
```
Eyebrow labels (RECENT TASKS, TODAY, TASKS, BILLING): 11px, weight 600, uppercase,
letter-spacing 0.06em, color `--text-muted`.

### Accent (the "running" identity — use consistently in tray, popover hero, active
rows, and the bottom chip)
```
--accent:        #2CC7C4   /* teal */
--accent-bright: #34D3CF   /* glow / hover */
--accent-grad:   linear-gradient(90deg, #15A8A5, #1FBEB8)  /* active row / chip */
```

### Task color palette (assignable to tasks and folders)
```
teal    #2DD4BF
cyan    #38BDF8
blue    #3B82F6
purple  #8B5CF6
magenta #A855F7
pink    #EC4899
lime    #84CC16
green   #22C55E
amber   #F59E0B
orange  #FB923C
```
Session and calendar blocks: fill = task color at 14% opacity (22% on hover), a
solid 3px left accent bar in the full color, and the label text in the full color.
Play/pause circles are the full task color with a dark glyph.

### Typography
Font stack: `-apple-system, "SF Pro Text", "SF Pro Display", Inter, system-ui,
sans-serif`. Bundle Inter as the cross-platform dev fallback; SF renders natively on
Mac. All numeric timers, durations, and totals use `font-variant-numeric:
tabular-nums` so digits do not jitter while ticking.

Scale (approximate):
```
timer hero (popover)      56px / 700
timer chip + entry dur    15px / 600
task/entry name           14px / 500
body / secondary          13-14px / 400
eyebrow labels            11px / 600 uppercase
```

### Shape and spacing
Radius: popover 16px, panels 14px, rows 10px, pills/toggles 8px, play/pause buttons
fully round. Spacing on a 4px base scale. Keep the layout quiet and precise; the one
bold element is the teal running-state thread that appears everywhere a live session
is shown. Do not add gradients or glows anywhere except the active/running treatment.

### Quality floor
Dark theme only in v1. Visible keyboard focus rings (use `--accent`). Respect
`prefers-reduced-motion` (disable block/transition animations). The quick panel and
main window both usable at their minimum sizes without clipping.

---

## 10. Project structure

```
timebar/
  src-tauri/
    src/
      main.rs            // app setup, tray, activation policy, window mgmt
      tray.rs            // tray icon, live-title loop, click handling
      commands/          // one module per command group (Section 8)
      db/
        migrations.rs    // versioned migrations (Section 5)
        queries.rs       // aggregation SQL (entries, tree, report, timeline)
      models.rs          // Folder, Task, Session, view structs (serde)
    tauri.conf.json      // two windows: quick + main
    Cargo.toml
  src/
    main.tsx
    windows/
      QuickPanel.tsx
      MainWindow.tsx
    views/
      Tracking.tsx
      TimelineDay.tsx
      TimelineWeek.tsx
      Reports.tsx
    components/          // TaskRow, EntryRow, DayCalendar, WeekCalendar,
                         // MonthChart, PlayPauseButton, TimerHero, StatsFooter,
                         // TaskTree, ManualEntryForm, Sidebar, RunningChip
    store/               // zustand store, active-session subscription
    lib/
      ipc.ts             // typed wrappers over invoke()
      time.ts            // formatting, day/week bucketing, tz helpers
      colors.ts          // palette + block-fill helpers
    styles/tokens.css    // the CSS variables from Section 9
    tailwind.config.ts
```

Frontend detects which window it is (`getCurrent().label`) and renders QuickPanel or
MainWindow accordingly, from one Vite build.

---

## 11. Build order and acceptance criteria

Ship in milestones; each must be runnable and demoable.

- **M1 — Data layer.** Migrations, models, seed data, all CRUD IPC commands, typed
  `ipc.ts`. *Accept:* can create/list/update/archive tasks and folders and CRUD
  sessions from a dev harness; DB persists across restarts.
- **M2 — Tray + timer + quick panel start/stop.** Tray icon with live title, quick
  panel window, hero, recent-tasks list with play/pause, single-active-session
  enforcement, footer stats, `active-session-changed` event. *Accept:* start a task
  from the menu bar, watch the tray title tick, stop it, and see it land in the DB;
  restarting the app mid-session keeps the timer running from `started_at`.
- **M3 — Tracking view.** Main window shell, left rail, running chip, task tree with
  folders and rolled-up totals, All/Recent/Archive tabs, Time Entries feed grouped
  by day, manual entry, entry editing, drag-reorder and drag-into-folder. *Accept:*
  full manual workflow works end to end and totals match hand calculation.
- **M4 — Reports.** Scope tabs, money/time toggle, stacked month chart, entries
  feed, right sidebar (Tasks, Archived, Billing, Summary), CSV export. *Accept:*
  chart and sidebar totals reconcile with the entries feed for the same range.
- **M5 — Timeline.** Day and week calendars, block rendering with midnight-split,
  click-to-edit, drag move/resize, zoom, navigation. *Accept:* a session spanning
  midnight appears correctly on both days and its edits round-trip to the DB.
- **M6 — Polish + package.** Empty states, keyboard shortcut for the quick panel,
  focus states, reduced-motion, app icon, `tauri build`. *Accept:* a signed,
  notarized `.app`/`.dmg` (or at minimum a locally runnable build with signing/
  notarization documented) that launches into the menu bar with no Dock icon.

---

## 12. Designed-in seams for later (do not build now, but do not block)

- **Auto-tracking (Activities).** Add later as an `activity_events` table
  (`app_bundle_id`, `window_title`, `url`, `file_path`, `started_at`, `ended_at`)
  fed by a separate capture module, plus a rules table mapping event patterns to
  tasks. The Timeline Day view is built as two columns conceptually; v1 renders only
  the right column, so the left Activities column can slot in without a rewrite. A
  faster path is to consume ActivityWatch's local REST API instead of writing the
  macOS capture code. Keep the session model independent of activity events.
- **Cloud sync (Supabase).** The schema already uses UUID ids and `updated_at` on
  every row. Add a `deleted_at` soft-delete column and a `sync` queue when sync is
  built; mirror the four tables to Postgres and reconcile last-write-wins by
  `updated_at`. Do not add sync plumbing in v1, but do not use auto-increment ids
  anywhere.
- **PDF invoices.** Billing data (rate snapshot, billable, billed flags) is already
  captured, so invoicing is a reporting-layer addition later.

---

## 13. Notes for the agent

- Prefer clarity over cleverness. This is a tool the owner will read and extend.
- Do all time aggregation in Rust/SQL; the frontend formats but does not compute
  totals, so numbers never disagree between views.
- Never store an in-memory elapsed counter as truth; always derive from
  `started_at`.
- Keep copy plain, active-voice, sentence case (see the design system).
- Match the reference feel: dark, quiet, precise, with the teal running-state thread
  as the one bold, consistent signature. No decorative gradients elsewhere.
