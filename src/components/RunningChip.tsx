import { Pause } from "lucide-react";
import { useStore, useElapsed } from "../store/store";
import { formatTimer } from "../lib/time";

/**
 * Persistent teal running-timer chip shown at the bottom of the main rail in
 * all sections (§6.3). Hidden when nothing is running.
 */
export function RunningChip() {
  const active = useStore((s) => s.activeSession);
  const stop = useStore((s) => s.stop);
  const elapsed = useElapsed();

  if (!active) return null;

  return (
    <button
      type="button"
      onClick={() => stop()}
      title="Stop"
      className="flex w-full items-center gap-2.5 rounded-panel bg-accent-grad px-3 py-2.5 text-left text-white shadow-lg"
    >
      <span className="grid h-7 w-7 place-items-center rounded-full bg-white/90">
        <Pause size={13} className="text-app" fill="currentColor" strokeWidth={0} />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[12px] text-white/80">{active.taskName}</span>
        <span className="tabular block text-[15px] font-semibold leading-tight">
          {formatTimer(elapsed)}
        </span>
      </span>
    </button>
  );
}
