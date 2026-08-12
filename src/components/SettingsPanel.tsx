import { useState } from "react";
import { Modal, Toggle } from "./ui";
import * as ipc from "../lib/ipc";
import { useStore } from "../store/store";

export function SettingsPanel({ onClose }: { onClose: () => void }) {
  const settings = useStore((s) => s.settings);
  const reloadSettings = useStore((s) => s.reloadSettings);
  const [busy, setBusy] = useState(false);

  const shortcutOn = settings?.globalShortcutEnabled ?? true;

  const toggleShortcut = async (enabled: boolean) => {
    setBusy(true);
    try {
      await ipc.setGlobalShortcut(enabled);
      await reloadSettings();
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal title="Settings" onClose={onClose}>
      <div className="flex items-start justify-between gap-4 py-1">
        <div className="min-w-0">
          <div className="text-[14px] font-medium text-text-primary">Global shortcut</div>
          <div className="mt-0.5 text-[12px] leading-relaxed text-text-secondary">
            Toggle the quick panel from anywhere with{" "}
            <span className="rounded bg-panel-raised px-1.5 py-0.5 font-medium text-text-primary">
              ⌘⇧T
            </span>
            . Turn this off if it conflicts with another app.
          </div>
        </div>
        <div className={busy ? "pointer-events-none opacity-60" : ""}>
          <Toggle checked={shortcutOn} onChange={toggleShortcut} label="" />
        </div>
      </div>
    </Modal>
  );
}
