// Which OS the app is running on. WebKit (macOS) and WebView2 (Windows)
// both report it in the user agent, so no Tauri plugin is needed.
export const isMac = /Mac/.test(navigator.userAgent);
export const isWindows = /Windows/.test(navigator.userAgent);

/** The quick-panel shortcut as shown to the user. Matches
 *  `quick_toggle_shortcut` in src-tauri/src/commands/settings.rs. */
export const quickPanelShortcutLabel = isMac ? "⌘⇧T" : "Ctrl+Alt+T";

/** Tag <html> with the platform so CSS can adjust (e.g. title bar spacing). */
export function applyPlatformClass() {
  const root = document.documentElement;
  if (isMac) root.classList.add("platform-mac");
  if (isWindows) root.classList.add("platform-windows");
}
