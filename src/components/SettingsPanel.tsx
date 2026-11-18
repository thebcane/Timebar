import { useState, type ReactNode } from "react";
import { Modal, Toggle } from "./ui";
import * as ipc from "../lib/ipc";
import { quickPanelShortcutLabel } from "../lib/platform";
import { useStore } from "../store/store";

function SettingRow({
  title,
  children,
  checked,
  busy,
  onChange,
}: {
  title: string;
  children: ReactNode;
  checked: boolean;
  busy: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="flex items-start justify-between gap-4 py-1">
      <div className="min-w-0">
        <div className="text-[14px] font-medium text-text-primary">{title}</div>
        <div className="mt-0.5 text-[12px] leading-relaxed text-text-secondary">{children}</div>
      </div>
      <div className={busy ? "pointer-events-none opacity-60" : ""}>
        <Toggle checked={checked} onChange={onChange} label="" />
      </div>
    </div>
  );
}

export function SettingsPanel({ onClose }: { onClose: () => void }) {
  const settings = useStore((s) => s.settings);
  const reloadSettings = useStore((s) => s.reloadSettings);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async (fn: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await fn();
      await reloadSettings();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal title="Settings" onClose={onClose}>
      <div className="flex flex-col gap-4">
        <SettingRow
          title="Global shortcut"
          checked={settings?.globalShortcutEnabled ?? true}
          busy={busy}
          onChange={(v) => run(() => ipc.setGlobalShortcut(v))}
        >
          Toggle the quick panel from anywhere with{" "}
          <span className="rounded bg-panel-raised px-1.5 py-0.5 font-medium text-text-primary">
            {quickPanelShortcutLabel}
          </span>
          . Turn this off if it conflicts with another app.
        </SettingRow>
        <SettingRow
          title="Open at login"
          checked={settings?.launchAtLogin ?? false}
          busy={busy}
          onChange={(v) => run(() => ipc.setLaunchAtLogin(v))}
        >
          Start TimeBar in the background when you sign in to this computer.
        </SettingRow>
        {error && (
          <div className="rounded-pill bg-panel-raised px-3 py-2 text-[12px] text-task-pink">{error}</div>
        )}
      </div>
    </Modal>
  );
}
