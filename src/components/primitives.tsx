import { Play, Pause } from "lucide-react";
import type { ReactNode } from "react";

/** Round play/pause button in the full task color with a dark glyph (§9). */
export function PlayPauseButton({
  color,
  active,
  size = 36,
  glyph = "dark",
  onClick,
  title,
}: {
  color: string;
  active: boolean;
  size?: number;
  /** Glyph color: dark on full-color circles, light on teal running chips. */
  glyph?: "dark" | "light";
  onClick?: (e: React.MouseEvent) => void;
  title?: string;
}) {
  const Icon = active ? Pause : Play;
  return (
    <button
      type="button"
      title={title ?? (active ? "Stop" : "Start")}
      onClick={onClick}
      className="grid place-items-center rounded-full shrink-0 transition-transform hover:scale-105 active:scale-95"
      style={{ width: size, height: size, background: color }}
    >
      <Icon
        size={Math.round(size * 0.4)}
        className={glyph === "light" ? "text-white" : "text-app"}
        fill="currentColor"
        strokeWidth={0}
      />
    </button>
  );
}

export function Eyebrow({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <div className={`eyebrow ${className}`}>{children}</div>;
}

export function EmptyState({ children }: { children: ReactNode }) {
  return (
    <div className="px-4 py-8 text-center text-[13px] text-text-secondary leading-relaxed">
      {children}
    </div>
  );
}

/** A colored dot chip used next to task/folder names. */
export function ColorDot({ color, size = 10 }: { color: string; size?: number }) {
  return (
    <span
      className="inline-block rounded-full shrink-0"
      style={{ width: size, height: size, background: color }}
    />
  );
}
