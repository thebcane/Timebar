import { useEffect, type ReactNode } from "react";
import { X, ChevronDown } from "lucide-react";

export function Modal({
  title,
  onClose,
  children,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-overlay p-6" onClick={onClose}>
      <div
        className="w-[440px] max-w-full rounded-panel border border-hairline bg-panel p-5 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-[15px] font-semibold text-text-primary">{title}</h2>
          <button type="button" onClick={onClose} className="text-text-muted hover:text-text-primary">
            <X size={18} />
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="mb-3 block">
      <span className="eyebrow mb-1.5 block">{label}</span>
      {children}
    </label>
  );
}

const inputCls =
  "w-full rounded-pill border border-hairline bg-panel-raised px-3 py-2 text-[13px] text-text-primary outline-none focus:border-accent";

export function TextInput(props: React.InputHTMLAttributes<HTMLInputElement>) {
  return <input {...props} className={`${inputCls} ${props.className ?? ""}`} />;
}

export function Select(props: React.SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <div className="relative">
      <select
        {...props}
        className={`${inputCls} cursor-pointer appearance-none pr-9 ${props.className ?? ""}`}
      />
      <ChevronDown
        size={15}
        className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-text-muted"
      />
    </div>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
}) {
  return (
    <button
      type="button"
      onClick={() => onChange(!checked)}
      className="flex items-center gap-2.5 text-[13px] text-text-primary"
    >
      <span
        className={`flex h-5 w-9 items-center rounded-full p-0.5 transition-colors ${
          checked ? "justify-end bg-accent" : "justify-start bg-panel-raised"
        }`}
      >
        <span className="h-4 w-4 rounded-full bg-white shadow-sm" />
      </span>
      {label}
    </button>
  );
}

export function Btn({
  children,
  variant = "ghost",
  ...props
}: {
  children: ReactNode;
  variant?: "primary" | "ghost" | "danger";
} & React.ButtonHTMLAttributes<HTMLButtonElement>) {
  const styles = {
    primary: "bg-accent-grad text-white",
    ghost: "border border-hairline bg-panel-raised text-text-primary hover:border-accent",
    danger: "text-task-pink hover:bg-panel-raised",
  }[variant];
  return (
    <button
      {...props}
      className={`rounded-pill px-4 py-2 text-[13px] font-medium transition-colors ${styles} ${
        props.className ?? ""
      }`}
    >
      {children}
    </button>
  );
}

/** Color picker row from the task palette. */
export function ColorPicker({
  value,
  onChange,
  colors,
}: {
  value: string;
  onChange: (hex: string) => void;
  colors: { name: string; hex: string }[];
}) {
  return (
    <div className="flex flex-wrap gap-2">
      {colors.map((c) => (
        <button
          key={c.hex}
          type="button"
          title={c.name}
          onClick={() => onChange(c.hex)}
          className={`h-6 w-6 rounded-full transition-transform hover:scale-110 ${
            value.toLowerCase() === c.hex.toLowerCase() ? "ring-2 ring-white ring-offset-2 ring-offset-panel" : ""
          }`}
          style={{ background: c.hex }}
        />
      ))}
    </div>
  );
}
