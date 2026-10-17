/**
 * E2E configuration: WebdriverIO driving the real app through tauri-plugin-webdriver, which
 * serves W3C WebDriver on port 4445 from inside a build made with `--features webdriver`.
 *
 *   cargo build --features webdriver --manifest-path src-tauri/Cargo.toml   # once, or after Rust changes
 *   bun run test:e2e                                                         # starts Vite and the app, runs the specs
 *
 * The app is launched with HOME pointing at a throwaway fixture folder, so a spec can pick
 * "Home folder" and scan something small and predictable instead of the real home directory.
 */

import { spawn, type ChildProcess } from "node:child_process";
import fs from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "../..");
const APP_BINARY = process.env.UNSHIFT_APP_BINARY ?? path.join(ROOT, "src-tauri", "target", "debug", "unshift-app");
const VITE_PORT = 1420;
const WEBDRIVER_PORT = 4445;

/** Fixture home folder the app scans; see `writeFixtureHome`. */
export const FIXTURE_HOME = path.join(os.tmpdir(), `unshift-e2e-${process.pid}`);

let viteProcess: ChildProcess | undefined;
let appProcess: ChildProcess | undefined;

/** A few folders with known contents so the breakdown has something to show. */
function writeFixtureHome(home: string) {
  fs.rmSync(home, { recursive: true, force: true });
  for (const [file, size] of [
    ["Documents/report.pdf", 4 * 1024 * 1024],
    ["Documents/notes.txt", 64 * 1024],
    ["Downloads/installer.dmg", 12 * 1024 * 1024],
    ["Pictures/holiday.jpg", 2 * 1024 * 1024],
  ] as const) {
    const target = path.join(home, file);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, Buffer.alloc(size));
  }
}

function pipe(child: ChildProcess, tag: string) {
  child.stdout?.on("data", (d: Buffer) => process.stdout.write(`[${tag}] ${d}`));
  child.stderr?.on("data", (d: Buffer) => process.stderr.write(`[${tag}] ${d}`));
}

/** `host` matters: Vite may listen on ::1 only, so probe it by name just as the app does. */
function isPortOpen(port: number, host: string) {
  return new Promise<boolean>((resolve) => {
    const s = net.connect({ port, host, autoSelectFamily: true });
    s.once("connect", () => (s.destroy(), resolve(true)));
    s.once("error", () => (s.destroy(), resolve(false)));
  });
}

/** Poll a TCP port until it accepts connections, the owning process exits, or we time out. */
async function waitForPort(port: number, host: string, timeoutMs: number, label: string, child: ChildProcess) {
  let exitCode: number | null = null;
  child.once("exit", (code) => {
    exitCode = code ?? 1;
  });
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (exitCode !== null) throw new Error(`[e2e] ${label} exited with code ${exitCode} before port ${port} opened`);
    if (await isPortOpen(port, host)) return;
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error(`[e2e] ${label} did not open port ${port} within ${timeoutMs}ms`);
}

export const config: WebdriverIO.Config = {
  runner: "local",
  hostname: "127.0.0.1",
  port: WEBDRIVER_PORT,
  path: "/",
  specs: ["./specs/**/*.spec.ts"],
  maxInstances: 1,
  capabilities: [{ browserName: "chrome", "wdio:enforceWebDriverClassic": true }],
  logLevel: "warn",
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: { ui: "bdd", timeout: 90_000 },
  tsConfigPath: path.join(import.meta.dirname, "tsconfig.json"),

  onPrepare: async () => {
    if (!fs.existsSync(APP_BINARY)) {
      throw new Error(
        `[e2e] No app binary at ${APP_BINARY}. Run: cargo build --features webdriver --manifest-path src-tauri/Cargo.toml`,
      );
    }
    writeFixtureHome(FIXTURE_HOME);

    // The debug app always loads http://localhost:1420 (tauri.conf.json devUrl). A `bun tauri dev`
    // already running on it is reused so the suite can run alongside a dev session.
    if (await isPortOpen(VITE_PORT, "localhost")) {
      console.warn(`[e2e] Port ${VITE_PORT} is already serving; reusing it instead of starting Vite`);
    } else {
      viteProcess = spawn(path.join(ROOT, "node_modules", ".bin", "vite"), ["--port", String(VITE_PORT)], {
        cwd: ROOT,
        stdio: ["ignore", "pipe", "pipe"],
      });
      pipe(viteProcess, "vite");
      await waitForPort(VITE_PORT, "localhost", 30_000, "Vite", viteProcess);
    }

    appProcess = spawn(APP_BINARY, [], {
      cwd: ROOT,
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, HOME: FIXTURE_HOME, TAURI_WEBDRIVER_PORT: String(WEBDRIVER_PORT) },
    });
    pipe(appProcess, "app");
    await waitForPort(WEBDRIVER_PORT, "127.0.0.1", 30_000, "App WebDriver server", appProcess);
  },

  onComplete: () => {
    appProcess?.kill();
    viteProcess?.kill();
    fs.rmSync(FIXTURE_HOME, { recursive: true, force: true });
  },
};
