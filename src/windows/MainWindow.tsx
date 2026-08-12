import { useState } from "react";
import { useAppInit } from "../store/useInit";
import { Sidebar, type Section } from "../components/Sidebar";
import { DialogHost } from "../components/DialogHost";
import { SyncPanel } from "../components/SyncPanel";
import { SettingsPanel } from "../components/SettingsPanel";
import Tracking from "../views/Tracking";
import Timeline from "../views/Timeline";
import Reports from "../views/Reports";

export default function MainWindow() {
  useAppInit();
  const [section, setSection] = useState<Section>("tracking");
  const [syncOpen, setSyncOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);

  return (
    <div className="flex h-screen overflow-hidden bg-app text-text-primary">
      <Sidebar
        section={section}
        onChange={setSection}
        onOpenSync={() => setSyncOpen(true)}
        onOpenSettings={() => setSettingsOpen(true)}
      />
      <main className="min-w-0 flex-1 overflow-hidden">
        {section === "tracking" && <Tracking />}
        {section === "timeline" && <Timeline />}
        {section === "reports" && <Reports />}
      </main>
      {syncOpen ? <SyncPanel onClose={() => setSyncOpen(false)} /> : null}
      {settingsOpen ? <SettingsPanel onClose={() => setSettingsOpen(false)} /> : null}
      <DialogHost />
    </div>
  );
}
