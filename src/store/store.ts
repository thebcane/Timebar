// Thin Zustand store (Section 2): SQLite is the source of truth. The store
// holds the active-session pointer, a 1s tick for live elapsed rendering, and
// a `dataVersion` counter that views watch to know when to re-query.

import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import * as ipc from "../lib/ipc";
import type { SessionView, Settings } from "../lib/types";

/** In-app dialog state (native window.confirm/alert are unreliable in an
 * accessory-app webview, so we render our own). */
interface DialogState {
  message: string;
  kind: "confirm" | "alert";
  confirmLabel: string;
  danger: boolean;
  resolve: (ok: boolean) => void;
}

interface AppState {
  activeSession: SessionView | null;
  settings: Settings | null;
  /** Wall-clock ms, bumped every second so live timers re-render. */
  nowTick: number;
  /** Incremented on any data mutation so views re-fetch. */
  dataVersion: number;
  ready: boolean;

  dialog: DialogState | null;
  requestConfirm: (message: string, opts?: { confirmLabel?: string; danger?: boolean }) => Promise<boolean>;
  notify: (message: string) => void;
  closeDialog: (ok: boolean) => void;

  init: () => Promise<() => void>;
  refreshActive: () => Promise<void>;
  reloadSettings: () => Promise<void>;
  start: (taskId: string) => Promise<void>;
  stop: () => Promise<void>;
  /** Toggle: start the task, or stop it if it is already running. */
  toggle: (taskId: string) => Promise<void>;
  bump: () => void;
}

export const useStore = create<AppState>((set, get) => ({
  activeSession: null,
  settings: null,
  nowTick: Date.now(),
  dataVersion: 0,
  ready: false,

  dialog: null,
  requestConfirm: (message, opts) =>
    new Promise<boolean>((resolve) => {
      set({
        dialog: {
          message,
          kind: "confirm",
          confirmLabel: opts?.confirmLabel ?? "Delete",
          danger: opts?.danger ?? true,
          resolve,
        },
      });
    }),
  notify: (message) =>
    set({
      dialog: { message, kind: "alert", confirmLabel: "OK", danger: false, resolve: () => {} },
    }),
  closeDialog: (ok) => {
    const d = get().dialog;
    set({ dialog: null });
    d?.resolve(ok);
  },

  init: async () => {
    const [settings, active] = await Promise.all([
      ipc.getSettings(),
      ipc.getActiveSession(),
    ]);
    set({ settings, activeSession: active, ready: true });

    const unlistenActive: UnlistenFn = await listen<SessionView | null>(
      "active-session-changed",
      (event) => {
        set((s) => ({
          activeSession: event.payload,
          dataVersion: s.dataVersion + 1,
        }));
      },
    );

    // Sync pulled remote changes → refresh every view.
    const unlistenData: UnlistenFn = await listen("data-changed", () => {
      set((s) => ({ dataVersion: s.dataVersion + 1 }));
    });

    const interval = window.setInterval(() => {
      set({ nowTick: Date.now() });
    }, 1000);

    return () => {
      unlistenActive();
      unlistenData();
      window.clearInterval(interval);
    };
  },

  refreshActive: async () => {
    const active = await ipc.getActiveSession();
    set({ activeSession: active });
  },

  reloadSettings: async () => {
    set({ settings: await ipc.getSettings() });
  },

  start: async (taskId) => {
    try {
      const view = await ipc.startTask(taskId);
      set((s) => ({ activeSession: view, dataVersion: s.dataVersion + 1 }));
    } catch (err) {
      console.error("start_task failed:", err);
      get().notify(`Couldn't start the timer:\n${err}`);
    }
  },

  stop: async () => {
    try {
      await ipc.stopActive();
      set((s) => ({ activeSession: null, dataVersion: s.dataVersion + 1 }));
    } catch (err) {
      console.error("stop_active failed:", err);
      get().notify(`Couldn't stop the timer:\n${err}`);
    }
  },

  toggle: async (taskId) => {
    const { activeSession, start, stop } = get();
    if (activeSession && activeSession.taskId === taskId) {
      await stop();
    } else {
      await start(taskId);
    }
  },

  bump: () => set((s) => ({ dataVersion: s.dataVersion + 1 })),
}));

/** Live elapsed ms for the active session (derived from startedAt). */
export function useElapsed(): number {
  const active = useStore((s) => s.activeSession);
  const tick = useStore((s) => s.nowTick);
  if (!active) return 0;
  return Math.max(0, tick - active.startedAt);
}
