import { ListChecks, CalendarRange, BarChart3, Cloud, Settings } from "lucide-react";
import { RunningChip } from "./RunningChip";

export type Section = "tracking" | "timeline" | "reports";

const NAV: { key: Section; label: string; icon: typeof ListChecks }[] = [
  { key: "tracking", label: "Tracking", icon: ListChecks },
  { key: "timeline", label: "Timeline", icon: CalendarRange },
  { key: "reports", label: "Reports", icon: BarChart3 },
];

export function Sidebar({
  section,
  onChange,
  onOpenSync,
  onOpenSettings,
}: {
  section: Section;
  onChange: (s: Section) => void;
  onOpenSync: () => void;
  onOpenSettings: () => void;
}) {
  return (
    <div className="flex w-[200px] shrink-0 flex-col bg-sidebar-grad">
      {/* Room for the macOS traffic lights; shorter on Windows (tokens.css). */}
      <div data-tauri-drag-region className="titlebar-spacer h-10 shrink-0" />
      <nav className="flex flex-col gap-1 px-3">
        {NAV.map(({ key, label, icon: Icon }) => {
          const active = key === section;
          return (
            <button
              key={key}
              type="button"
              onClick={() => onChange(key)}
              className={`flex items-center gap-3 rounded-row px-3 py-2 text-[14px] transition-colors ${
                active
                  ? "bg-panel-raised text-text-primary"
                  : "text-text-secondary hover:bg-panel-raised/60 hover:text-text-primary"
              }`}
            >
              <Icon size={17} className="text-task-blue" />
              {label}
            </button>
          );
        })}
      </nav>
      <div className="mt-auto flex flex-col gap-2 p-3">
        <button
          type="button"
          onClick={onOpenSync}
          className="flex items-center gap-3 rounded-row px-3 py-2 text-[14px] text-text-secondary transition-colors hover:bg-panel-raised/60 hover:text-text-primary"
        >
          <Cloud size={17} className="text-task-blue" />
          Sync
        </button>
        <button
          type="button"
          onClick={onOpenSettings}
          className="flex items-center gap-3 rounded-row px-3 py-2 text-[14px] text-text-secondary transition-colors hover:bg-panel-raised/60 hover:text-text-primary"
        >
          <Settings size={17} className="text-task-blue" />
          Settings
        </button>
        <RunningChip />
      </div>
    </div>
  );
}
