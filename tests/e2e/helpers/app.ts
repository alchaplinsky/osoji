import { $, $$, browser } from "@wdio/globals";

/**
 * tauri-plugin-webdriver's WebKit driver rejects WebdriverIO's combined css+text selectors and
 * cannot pass element handles back into scripts (so `isDisplayed` is unreliable). Helpers here
 * stick to plain CSS plus `getText`, and assert existence rather than visibility.
 */

/** The Scan screen is the landing screen; its title is the first reliable landmark. */
export async function waitForAppReady(timeout = 15_000) {
  await $("div*=Ready to sweep").waitForExist({ timeout });
}

/** The first element matching `selector` whose text contains `text`, waiting for it to appear. */
export async function elementWithText(selector: string, text: string, timeout = 5_000) {
  let found: WebdriverIO.Element | undefined;
  await browser.waitUntil(
    async () => {
      for (const el of await $$(selector)) {
        if ((await el.getText()).includes(text)) {
          found = el;
          return true;
        }
      }
      return false;
    },
    { timeout, timeoutMsg: `no "${selector}" containing "${text}" within ${timeout}ms` },
  );
  return found!;
}

/** Open the disk card menu in the sidebar and pick an entry by its label. */
export async function pickRoot(label: string) {
  await $('button[aria-haspopup="menu"]').click();
  await (await elementWithText('[role="menu"] button', label)).click();
}

/** Click a sidebar navigation item by its label. */
export async function navigateTo(label: string) {
  await (await elementWithText("nav button", label)).click();
  await browser.pause(200);
}
