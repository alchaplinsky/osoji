import { $, expect } from "@wdio/globals";
import { elementWithText, pickRoot, waitForAppReady } from "../helpers/app";

describe("Scan", () => {
  before(async () => {
    await waitForAppReady();
  });

  it("offers the startup disk before any scan", async () => {
    await expect($("div=No scan yet")).toBeExisting();
    await elementWithText("button", "Scan ");
  });

  it("scans the home folder and lands on the breakdown", async () => {
    // HOME points at the fixture folder written by wdio.conf.ts, so this is small and quick.
    await pickRoot("Home folder");
    await (await elementWithText("button", "Scan ~")).click();

    await elementWithText("nav button", "Scan again", 60_000);
    await expect($("span*=Downloads")).toBeExisting();
    await expect($("span*=Documents")).toBeExisting();
  });
});
