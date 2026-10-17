// Native menu setup. We only customize one thing: a macOS-style "Check for
// Updates…" item in the application submenu (right under "About Unshift"). Since
// assigning a menu replaces the platform default wholesale, we start from that
// default and insert into it rather than rebuilding every standard item.

import { Menu, MenuItem, PredefinedMenuItem, Submenu } from "@tauri-apps/api/menu";

/**
 * Install the app menu with a "Check for Updates…" item that runs `onCheck`.
 * Safe to call at startup: any failure is logged and swallowed so a menu
 * problem can never block the app. No-op if the default menu has no application
 * submenu (e.g. the browser-only ?gallery mode, or non-macOS platforms).
 */
export async function setupAppMenu(onCheck: () => void): Promise<void> {
  try {
    const menu = await Menu.default();
    const [appMenu] = await menu.items();
    if (!(appMenu instanceof Submenu)) return;

    const separator = await PredefinedMenuItem.new({ item: "Separator" });
    const check = await MenuItem.new({ text: "Check for Updates…", action: onCheck });

    // After "About Unshift" (index 0): About / --- / Check for Updates… / …
    await appMenu.insert([separator, check], 1);
    await menu.setAsAppMenu();
  } catch (err) {
    console.warn("Failed to install app menu:", err);
  }
}
