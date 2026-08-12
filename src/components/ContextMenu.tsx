import { useEffect, type ReactNode } from "react";

export interface MenuItem {
  label: string;
  icon?: ReactNode;
  danger?: boolean;
  onClick: () => void;
}

/** Lightweight right-click menu anchored at the cursor. */
export function ContextMenu({
  x,
  y,
  items,
  onClose,
}: {
  x: number;
  y: number;
  items: MenuItem[];
  onClose: () => void;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  // Keep the menu on-screen.
  const width = 176;
  const height = items.length * 34 + 8;
  const left = Math.min(x, window.innerWidth - width - 8);
  const top = Math.min(y, window.innerHeight - height - 8);

  return (
    <div
      className="fixed inset-0 z-[60]"
      onClick={onClose}
      onContextMenu={(e) => {
        e.preventDefault();
        onClose();
      }}
    >
      <div
        className="absolute rounded-panel border border-hairline bg-panel-raised py-1 shadow-2xl"
        style={{ top, left, minWidth: width }}
        onClick={(e) => e.stopPropagation()}
      >
        {items.map((it, i) => (
          <button
            key={i}
            type="button"
            onClick={() => {
              it.onClick();
              onClose();
            }}
            className={`flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] transition-colors hover:bg-panel ${
              it.danger ? "text-task-pink" : "text-text-primary"
            }`}
          >
            {it.icon ? <span className="grid w-4 place-items-center">{it.icon}</span> : null}
            {it.label}
          </button>
        ))}
      </div>
    </div>
  );
}
