// Typed wrappers over Tauri `invoke` (Section 8). This is the whole frontend
// contract with the Rust core; nothing else calls invoke directly.

import { invoke } from "@tauri-apps/api/core";
import type {
  DayGroup,
  Folder,
  FolderPatch,
  Range,
  ReportView,
  SessionPatch,
  SessionView,
  Settings,
  SyncStatus,
  Task,
  TaskPatch,
  TimelineBlock,
  TreeNode,
} from "./types";

// --- Tasks & folders -------------------------------------------------------

export const listFolders = () => invoke<Folder[]>("list_folders");

export const createFolder = (name: string, color: string, parentId?: string | null) =>
  invoke<Folder>("create_folder", { name, color, parentId: parentId ?? null });

export const updateFolder = (id: string, patch: FolderPatch) =>
  invoke<Folder>("update_folder", { id, patch });

export const archiveFolder = (id: string, archived: boolean) =>
  invoke<void>("archive_folder", { id, archived });

export const deleteFolder = (id: string) => invoke<void>("delete_folder", { id });

export const listTasks = (includeArchived = false) =>
  invoke<Task[]>("list_tasks", { includeArchived });

export const createTask = (args: {
  name: string;
  color: string;
  folderId?: string | null;
  hourlyRate?: number;
  currency?: string;
  billable?: boolean;
}) =>
  invoke<Task>("create_task", {
    name: args.name,
    color: args.color,
    folderId: args.folderId ?? null,
    hourlyRate: args.hourlyRate ?? null,
    currency: args.currency ?? null,
    billable: args.billable ?? null,
  });

export const updateTask = (id: string, patch: TaskPatch) =>
  invoke<Task>("update_task", { id, patch });

export const archiveTask = (id: string, archived: boolean) =>
  invoke<void>("archive_task", { id, archived });

export const deleteTask = (id: string) => invoke<void>("delete_task", { id });

export const reorder = (entity: "task" | "folder", orderedIds: string[]) =>
  invoke<void>("reorder", { entity, orderedIds });

// --- Sessions & timer ------------------------------------------------------

export const getActiveSession = () =>
  invoke<SessionView | null>("get_active_session");

export const getSession = (id: string) =>
  invoke<SessionView | null>("get_session", { id });

export const startTask = (taskId: string) =>
  invoke<SessionView>("start_task", { taskId });

export const stopActive = () => invoke<SessionView | null>("stop_active");

export const createManualSession = (args: {
  taskId: string;
  startedAt: number;
  endedAt: number;
  note?: string | null;
  billable?: boolean | null;
}) =>
  invoke<SessionView>("create_manual_session", {
    taskId: args.taskId,
    startedAt: args.startedAt,
    endedAt: args.endedAt,
    note: args.note ?? null,
    billable: args.billable ?? null,
  });

export const updateSession = (id: string, patch: SessionPatch) =>
  invoke<SessionView>("update_session", { id, patch });

export const deleteSession = (id: string) =>
  invoke<void>("delete_session", { id });

// --- Queries / aggregation -------------------------------------------------

export const getTimeEntries = (range: Range, tz: string) =>
  invoke<DayGroup[]>("get_time_entries", { range, tz });

export const getTaskTreeWithTotals = (
  range: Range,
  tz: string,
  includeArchived: boolean,
) =>
  invoke<TreeNode[]>("get_task_tree_with_totals", { range, tz, includeArchived });

export const getReport = (
  range: Range,
  tz: string,
  mode: "time" | "money",
) => invoke<ReportView>("get_report", { range, tz, mode });

export const getTimeline = (range: Range, tz: string) =>
  invoke<TimelineBlock[]>("get_timeline", { range, tz });

// --- Settings & export -----------------------------------------------------

export const getSettings = () => invoke<Settings>("get_settings");

export const updateSetting = (key: string, value: string) =>
  invoke<void>("update_setting", { key, value });

export const setGlobalShortcut = (enabled: boolean) =>
  invoke<void>("set_global_shortcut", { enabled });

export const exportCsv = (range: Range, tz: string) =>
  invoke<string>("export_csv", { range, tz });

// --- Windows ---------------------------------------------------------------

export const openDashboard = () => invoke<void>("open_dashboard");
export const hideQuick = () => invoke<void>("hide_quick");

// --- Sync ------------------------------------------------------------------

export const syncStatus = () => invoke<SyncStatus>("sync_status");

export const syncSignIn = (email: string, password: string) =>
  invoke<void>("sync_sign_in", { email, password });

/** Returns "signed-in" or "confirm-email". */
export const syncSignUp = (email: string, password: string) =>
  invoke<string>("sync_sign_up", { email, password });

export const syncSignOut = () => invoke<void>("sync_sign_out");

export const syncChangePassword = (password: string) =>
  invoke<void>("sync_change_password", { password });

/** Returns the number of local rows changed by the pull. */
export const syncNow = () => invoke<number>("sync_now");
