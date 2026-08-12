// Task palette (Section 9) and block-fill helpers. Colors are stored on tasks
// as hex strings; these helpers derive the calendar/entry treatments.

export interface PaletteColor {
  name: string;
  hex: string;
}

export const PALETTE: PaletteColor[] = [
  { name: "teal", hex: "#2DD4BF" },
  { name: "cyan", hex: "#38BDF8" },
  { name: "blue", hex: "#3B82F6" },
  { name: "purple", hex: "#8B5CF6" },
  { name: "magenta", hex: "#A855F7" },
  { name: "pink", hex: "#EC4899" },
  { name: "lime", hex: "#84CC16" },
  { name: "green", hex: "#22C55E" },
  { name: "amber", hex: "#F59E0B" },
  { name: "orange", hex: "#FB923C" },
];

export const DEFAULT_COLOR = PALETTE[0].hex;

function hexToRgb(hex: string): [number, number, number] {
  const clean = hex.replace("#", "");
  const full =
    clean.length === 3
      ? clean
          .split("")
          .map((c) => c + c)
          .join("")
      : clean;
  const n = parseInt(full, 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

export function rgba(hex: string, alpha: number): string {
  const [r, g, b] = hexToRgb(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/**
 * Session / calendar block treatment (Section 9): fill at 14% (22% on hover),
 * a solid 3px left accent bar, label text in the full color.
 */
export function blockStyle(hex: string, hover = false) {
  return {
    backgroundColor: rgba(hex, hover ? 0.22 : 0.14),
    borderLeft: `3px solid ${hex}`,
    color: hex,
  } as const;
}
