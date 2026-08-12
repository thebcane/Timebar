import { PlayPauseButton, Eyebrow } from "./primitives";
import { formatDuration, formatDurationParts, formatMoney } from "../lib/time";

/**
 * A task list row: colored play/pause button, optional folder eyebrow, name,
 * and the task's total for the scope on the right. The active task gets the
 * teal running treatment (§6.2 / §6.3).
 */
export function TaskRow({
  name,
  color,
  folderName,
  totalMs,
  revenue,
  showRevenue,
  active,
  indent = 0,
  onToggle,
  onClick,
}: {
  name: string;
  color: string;
  folderName?: string | null;
  totalMs: number;
  revenue?: number;
  showRevenue?: boolean;
  active: boolean;
  indent?: number;
  onToggle: (e: React.MouseEvent) => void;
  onClick?: () => void;
}) {
  const parts = formatDurationParts(totalMs);
  return (
    <div
      onClick={onClick}
      className={`group flex items-center gap-3 rounded-row px-2.5 py-2 cursor-pointer transition-colors ${
        active ? "bg-accent-grad shadow-sm" : "hover:bg-panel-raised"
      }`}
      style={{ paddingLeft: 10 + indent * 18 }}
    >
      <PlayPauseButton
        color={active ? "rgba(255,255,255,0.18)" : color}
        glyph={active ? "light" : "dark"}
        active={active}
        size={30}
        onClick={onToggle}
      />
      <div className="min-w-0 flex-1">
        {folderName ? (
          <Eyebrow className={active ? "!text-white/70" : ""}>{folderName}</Eyebrow>
        ) : null}
        <div
          className={`truncate text-[14px] font-medium ${
            active ? "text-white" : "text-text-primary"
          }`}
        >
          {name}
        </div>
      </div>
      <div className="text-right">
        <div
          className={`tabular text-[14px] font-semibold ${active ? "text-white" : "text-text-primary"}`}
        >
          {active ? (
            <>
              {parts.hm}
              <span className="text-white/50">:{parts.ss}</span>
            </>
          ) : (
            formatDuration(totalMs)
          )}
        </div>
        {showRevenue && revenue ? (
          <div className={`tabular text-[11px] ${active ? "text-white/70" : "text-text-muted"}`}>
            {formatMoney(revenue)}
          </div>
        ) : null}
      </div>
    </div>
  );
}
