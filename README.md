# TimeBar

A menu-bar (macOS) and system-tray (Windows) time tracker. Start and stop tasks from the menu bar, organize
them into folders, and review everything in a dashboard with a day/week timeline
and billing reports.

Local-first: all data lives in a local SQLite database. Cloud sync via Supabase is
optional and off until you sign in.

Built with Tauri 2, React 19, TypeScript, and Vite.

## Features

- Menu-bar tray with a live running timer in the tray title
- Quick panel anchored under the tray icon (global shortcut: `Cmd+Shift+T`)
- Three main views: Tracking, Timeline (day/week), and Reports
- Folders, tasks, sessions, manual entry, notes, and per-task hourly rates
- Billable flags and revenue math; time and money totals across day/week/month/custom ranges
- CSV export of entries
- Optional multi-device sync through Supabase

Automatic activity tracking (recording which apps, files, and URLs you used) is
deliberately not implemented.

## Prerequisites

These are system-level and not installed by `npm install`:

| Requirement | Notes |
| --- | --- |
| **Node** | 20.19+ or 22.12+ (Vite 7 requirement) |
| **Rust** | Install via [rustup](https://rustup.rs) |
| **Xcode Command Line Tools** (macOS) | `xcode-select --install` — provides the C toolchain and WebKit headers |
| **VS Build Tools** (Windows) | "Desktop development with C++" workload — see Platform support |

## Getting started

```bash
git clone https://github.com/thebcane/Timebar.git
cd Timebar
npm install
npm run tauri dev
```

There is no `.env` to create and no database to set up.

The **first** `npm run tauri dev` takes roughly 10–20 minutes. It downloads about
300 MB of Rust crates and cold-compiles 560 dependencies. Later starts take
seconds.

You do not need to start Vite separately — Tauri runs it for you via
`beforeDevCommand` in [`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json).

### Building an installer

```bash
npm run tauri build
```

Outputs a ~5.5 MB `.dmg` to `src-tauri/target/release/bundle/dmg/`.

## Scripts

| Command | Does |
| --- | --- |
| `npm run tauri dev` | Run the full desktop app in development |
| `npm run tauri build` | Build and bundle the installer |
| `npm run dev` | Vite dev server only (browser, no Tauri APIs) |
| `npm run build` | Type-check and build the frontend |

## Data and storage

The SQLite database is created on first launch at the OS app-data directory —
on macOS, `~/Library/Application Support/com.studio.timebar/timebar.db`. Migrations
in [`src-tauri/migrations/`](src-tauri/migrations/) run automatically at startup.

The database lives outside the project directory by design, so **it does not travel
with the repo**. A fresh clone starts with an empty database. To carry existing
entries to another machine, either sign in to sync on both, or copy `timebar.db`
across manually.

## Sync

Sync is optional. Sign up or sign in from the Sync panel, and folders, tasks, and
sessions replicate through Supabase. The project URL and anon key are compiled into
[`src-tauri/src/sync.rs`](src-tauri/src/sync.rs).

The anon key is a publishable client key, safe to commit. Every table has row-level
security enabled with per-user policies — see
[`supabase/schema.sql`](supabase/schema.sql). Never put a `service_role` key in
this repo.

To point at your own Supabase project, apply `supabase/schema.sql` and then
`supabase/migrations/20261118_sync_v2.sql` to it, and update `SUPABASE_URL` and
`ANON_KEY` in `sync.rs`.

### How sync works

- Every local write marks the row `dirty`; each cycle pushes dirty rows and clears
  the flag. Writes also trigger a sync within about a second, and a background
  cycle runs every 4 seconds.
- The server stamps each accepted write with a `server_seq` number. Devices pull
  everything above the last number they saw, so a device with a wrong clock
  can't cause missed changes.
- Conflicts are last-write-wins by `updated_at`, enforced on the server too: a
  push older than the stored row is ignored. Local edits always move
  `updated_at` forward, even on a device whose clock is behind.
- Only one timer runs at a time across all devices. If two devices start timers
  before syncing, the one started last wins and the other ends when it started.
- Sample data from a fresh install is discarded on first sign-in if you haven't
  used it, so it never reaches your other devices.

### Testing sync with two copies on one machine

`TIMEBAR_DATA_DIR` points the app at a different database. With `npm run tauri dev`
running, start a second copy as another "device":

```bash
TIMEBAR_DATA_DIR=/tmp/timebar-device-2 ./src-tauri/target/debug/timebar
```

Sign both in to the same account. `cargo test` in `src-tauri` covers the local
half of sync (dirty tracking, clock skew, the one-running-timer rule, seed cleanup).

## Project layout

```
src/                  React frontend
  components/         Shared UI
  views/              Tracking, Timeline, Reports
  windows/            MainWindow and QuickPanel entry points
  store/              Zustand state
  lib/                Types, IPC bindings, time and color helpers
src-tauri/            Rust backend
  src/commands/       Tauri IPC commands (tasks, sessions, queries, export, settings)
  src/db.rs           SQLite pool, migrations, seed data
  src/sync.rs         Supabase sync
  src/tray.rs         Menu-bar tray
  migrations/         SQL migrations
supabase/schema.sql   Cloud schema and RLS policies
```

## Disk usage

A working clone grows to roughly 7.5 GB, which surprises people. Almost none of it
is downloaded — it is generated by the Rust compiler:

| Path | Size | Purpose |
| --- | --- | --- |
| `src-tauri/target/debug` | ~6 GB | Dev build artifacts and incremental cache |
| `src-tauri/target/release` | ~1.3 GB | Release build artifacts |
| `node_modules` | ~212 MB | Frontend dependencies |
| Source | ~940 KB | The actual repo |

All of it regenerates. To reclaim the space:

```bash
cargo clean --manifest-path src-tauri/Cargo.toml
```

The cost is one slow rebuild. `target/` and `node_modules/` are gitignored, so the
repository itself stays under 1 MB.

## Platform support

macOS and Windows, from the same code.

| | macOS | Windows |
| --- | --- | --- |
| Where it lives | Menu bar, no Dock icon | System tray (may start in the ^ overflow; drag it onto the taskbar to keep it visible) |
| Running timer | `H:MM:SS` next to the icon | Icon turns teal; hover for task and elapsed time |
| Quick panel | Opens under the icon | Opens above the icon |
| Shortcut | `Cmd+Shift+T` | `Ctrl+Alt+T` |
| Title bar | Hidden, content runs under it | Native Windows title bar |

Windows-only window settings live in `src-tauri/tauri.windows.conf.json`, which
Tauri merges over `tauri.conf.json` when building for Windows. Its `windows`
array replaces the base one wholesale, so a change to either window's size or
behavior needs making in both files.

### Windows prerequisites

- [Rust](https://rustup.rs) (MSVC toolchain, the default)
- Visual Studio Build Tools with the **Desktop development with C++** workload
- Node 20.19+ or 22.12+
- WebView2, already part of Windows 10 and 11

Then `npm install` and `npm run tauri dev`, same as on the Mac.

### Installers

Push a tag like `v0.2.0` and GitHub Actions builds a `.dmg` and Windows `.msi`/`.exe`
onto a draft release (`.github/workflows/release.yml`). Builds are unsigned: on
macOS right-click → Open the first time; on Windows SmartScreen asks you to click
More info → Run anyway. Every push also runs the tests on both systems
(`.github/workflows/check.yml`).
