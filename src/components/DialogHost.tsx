import { useEffect } from "react";
import { useStore } from "../store/store";

/**
 * Renders the store's confirm/alert dialog. Replaces native
 * window.confirm/alert, which don't reliably present in an accessory-app
 * webview (there's no key window to attach the sheet to). Mount once per window.
 */
export function DialogHost() {
  const dialog = useStore((s) => s.dialog);
  const close = useStore((s) => s.closeDialog);

  useEffect(() => {
    if (!dialog) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close(false);
      if (e.key === "Enter") close(true);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dialog, close]);

  if (!dialog) return null;

  return (
    <div
      className="fixed inset-0 z-[70] grid place-items-center bg-overlay p-6"
      onClick={() => close(false)}
    >
      <div
        className="w-[380px] max-w-full rounded-panel border border-hairline bg-panel p-5 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="whitespace-pre-line text-[14px] leading-relaxed text-text-primary">
          {dialog.message}
        </div>
        <div className="mt-5 flex justify-end gap-2">
          {dialog.kind === "confirm" ? (
            <button
              type="button"
              onClick={() => close(false)}
              className="rounded-pill border border-hairline bg-panel-raised px-4 py-2 text-[13px] font-medium text-text-primary hover:border-accent"
            >
              Cancel
            </button>
          ) : null}
          <button
            type="button"
            autoFocus
            onClick={() => close(true)}
            className={`rounded-pill px-4 py-2 text-[13px] font-medium text-white ${
              dialog.danger ? "bg-task-pink" : "bg-accent-grad"
            }`}
          >
            {dialog.confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
