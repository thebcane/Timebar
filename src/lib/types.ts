// TypeScript mirror of the Rust models (Section 8). All camelCase to match the
// serde `rename_all = "camelCase"` on the Rust view structs.

export interface Folder {
  id: string;
  name: string;
  color: string;
  parentId: string | null;
  sortOrder: number;
  archived: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface Task {
  id: string;
  name: string;
  color: string;
  folderId: string | null;
  hourlyRate: number;
  currency: string;
  billable: boolean;
  sortOrder: number;
  archived: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface SessionView {
  id: string;
  taskId: string;
  taskName: string;
  taskColor: string;
  folderName: string | null;
  startedAt: number;
  endedAt: number | null;
  note: string | null;
  rateSnapshot: number;
  billable: boolean;
  billed: boolean;
  source: string;
  durationMs: number;
  revenue: number;
  isActive: boolean;
}

export interface EntryView {
  sessionId: string;
  taskId: string;
  taskName: string;
  taskColor: string;
  folderName: string | null;
  startedAt: number;
  endedAt: number;
  note: string | null;
  billable: boolean;
  billed: boolean;
  source: string;
  durationMs: number;
  revenue: number;
  isActive: boolean;
}

export interface DayGroup {
  date: string; // YYYY-MM-DD local
  totalMs: number;
  totalRevenue: number;
  entries: EntryView[];
}

export interface TreeNode {
  kind: "folder" | "task";
  id: string;
  name: string;
  color: string;
  folderId: string | null;
  archived: boolean;
  billable: boolean;
  hourlyRate: number;
  currency: string;
  isActive: boolean;
  totalMs: number;
  totalRevenue: number;
  children: TreeNode[];
}

export interface TimelineBlock {
  sessionId: string;
  taskId: string;
  taskName: string;
  taskColor: string;
  note: string | null;
  date: string;
  startedAt: number;
  endedAt: number;
  startMin: number;
  endMin: number;
  durationMs: number;
  isActive: boolean;
}

export interface ReportSegment {
  taskId: string;
  taskName: string;
  color: string;
  ms: number;
  revenue: number;
}

export interface ReportDay {
  date: string;
  totalMs: number;
  totalRevenue: number;
  segments: ReportSegment[];
}

export interface ReportTaskTotal {
  id: string;
  name: string;
  color: string;
  ms: number;
  revenue: number;
  archived: boolean;
}

export interface ReportView {
  days: ReportDay[];
  tasks: ReportTaskTotal[];
  archived: ReportTaskTotal[];
  billingPaidMs: number;
  billingPaidRevenue: number;
  totalMs: number;
  totalRevenue: number;
}

export interface Settings {
  firstDayOfWeek: number;
  defaultCurrency: string;
  activeSessionId: string | null;
  theme: string;
  globalShortcutEnabled: boolean;
}

export interface SyncStatus {
  signedIn: boolean;
  email: string | null;
  lastSync: number | null;
  lastError: string | null;
}

export interface Range {
  from: number;
  to: number;
}

export interface TaskPatch {
  name?: string;
  color?: string;
  folderId?: string | null;
  hourlyRate?: number;
  currency?: string;
  billable?: boolean;
  sortOrder?: number;
  archived?: boolean;
}

export interface FolderPatch {
  name?: string;
  color?: string;
  parentId?: string | null;
  sortOrder?: number;
  archived?: boolean;
}

export interface SessionPatch {
  taskId?: string;
  startedAt?: number;
  endedAt?: number | null;
  note?: string | null;
  billed?: boolean;
  rateSnapshot?: number;
  resnapshotRate?: boolean;
}
