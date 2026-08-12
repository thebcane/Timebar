import type { Config } from "tailwindcss";

/**
 * All colors are pulled from CSS variables defined in src/styles/tokens.css
 * (Section 9 of the build brief). Nothing here is a hard-coded hex value, so
 * the token file stays the single source of truth for the palette.
 */
const config: Config = {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        app: "var(--app-bg)",
        panel: {
          DEFAULT: "var(--panel)",
          raised: "var(--panel-raised)",
        },
        sidebar: "var(--sidebar)",
        hairline: "var(--hairline)",
        overlay: "var(--overlay)",
        text: {
          primary: "var(--text-primary)",
          secondary: "var(--text-secondary)",
          muted: "var(--text-muted)",
        },
        accent: {
          DEFAULT: "var(--accent)",
          bright: "var(--accent-bright)",
        },
        // Task palette — also available as raw hex via lib/colors.ts
        task: {
          teal: "var(--task-teal)",
          cyan: "var(--task-cyan)",
          blue: "var(--task-blue)",
          purple: "var(--task-purple)",
          magenta: "var(--task-magenta)",
          pink: "var(--task-pink)",
          lime: "var(--task-lime)",
          green: "var(--task-green)",
          amber: "var(--task-amber)",
          orange: "var(--task-orange)",
        },
      },
      backgroundImage: {
        "accent-grad": "var(--accent-grad)",
        "sidebar-grad": "var(--sidebar-grad)",
      },
      borderColor: {
        DEFAULT: "var(--hairline)",
      },
      borderRadius: {
        popover: "16px",
        panel: "14px",
        row: "10px",
        pill: "8px",
      },
      fontFamily: {
        sans: [
          "-apple-system",
          "SF Pro Text",
          "SF Pro Display",
          "Inter",
          "system-ui",
          "sans-serif",
        ],
      },
      fontSize: {
        eyebrow: ["11px", { lineHeight: "1", letterSpacing: "0.06em", fontWeight: "600" }],
        "timer-hero": ["56px", { lineHeight: "1", fontWeight: "700" }],
      },
    },
  },
  plugins: [],
};

export default config;
