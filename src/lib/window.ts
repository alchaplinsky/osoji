import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * Reveal the main window after the initial React commit. The window is created
 * hidden (`visible: false` in tauri.conf.json) so the bare white webview never
 * shows; its native `backgroundColor` covers the sub-frame gap between show and
 * the first paint.
 *
 * This must not wait on `requestAnimationFrame`: a hidden window produces no
 * frames, so the callback would never run and the window would never appear.
 * A macrotask fires regardless of visibility, and by then the DOM is committed.
 * Failures are swallowed: in a plain browser (the dev gallery) there is no
 * Tauri window, and startup must never hang on this.
 */
export function revealWindow(): void {
  setTimeout(() => {
    const win = getCurrentWindow();
    void win
      .show()
      .then(() => win.setFocus())
      .catch(() => {});
  }, 0);
}
