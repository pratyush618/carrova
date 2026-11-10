/**
 * Real-app smoke: WebdriverIO drives the debug Carrova binary through
 * `tauri-driver`, which proxies WebDriver to WebKitWebDriver (the app renders
 * with WebKitGTK). Run it via `run.sh` (builds the app, wraps in `xvfb-run`).
 *
 * The app never sees the real user's home: `tauri-driver` (and through it
 * WebKitWebDriver and the app) start with HOME and every XDG base dir inside
 * a fresh temp dir, removed when the run ends.
 */
import { spawn, type ChildProcess } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { connect } from 'node:net';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const DRIVER_HOST = '127.0.0.1';
/** tauri-driver's port; WebKitWebDriver gets the next one. */
const DRIVER_PORT = Number(process.env['CARROVA_E2E_DRIVER_PORT'] ?? 4444);
const DRIVER_READY_TIMEOUT_MS = 30_000;

/** The app under test: `CARROVA_E2E_APP`, else run.sh's embedded debug build. */
const application = resolve(
  process.env['CARROVA_E2E_APP'] ?? join(here, '../../../target/e2e-app/debug/carrova-desktop'),
);

declare global {
  namespace WebdriverIO {
    /** tauri-driver's vendor capability: the app binary to launch. */
    interface Capabilities {
      'tauri:options'?: { application: string };
    }
  }
}

let driver: ChildProcess | undefined;
let sandbox: string | undefined;

/** HOME + XDG dirs inside `root`, so nothing touches the real ~/.config or ~/.local. */
function isolatedEnv(root: string): NodeJS.ProcessEnv {
  const dirs = {
    HOME: join(root, 'home'),
    XDG_CONFIG_HOME: join(root, 'config'),
    XDG_DATA_HOME: join(root, 'data'),
    XDG_STATE_HOME: join(root, 'state'),
    XDG_CACHE_HOME: join(root, 'cache'),
  };
  for (const dir of Object.values(dirs)) mkdirSync(dir, { recursive: true });
  // A developer recipe override would change what the catalogue shows.
  const { CARROVA_RECIPES: _ignored, ...inherited } = process.env;
  return { ...inherited, ...dirs };
}

/** Resolves once something accepts TCP connections on the driver port. */
async function waitForDriver(deadline: number): Promise<void> {
  for (;;) {
    const up = await new Promise<boolean>((done) => {
      const socket = connect(DRIVER_PORT, DRIVER_HOST);
      socket.once('connect', () => {
        socket.destroy();
        done(true);
      });
      socket.once('error', () => {
        done(false);
      });
    });
    if (up) return;
    if (driver?.exitCode != null) {
      throw new Error(`tauri-driver exited early (code ${String(driver.exitCode)})`);
    }
    if (Date.now() > deadline) {
      throw new Error(`tauri-driver not listening on ${DRIVER_HOST}:${String(DRIVER_PORT)}`);
    }
    await new Promise((r) => setTimeout(r, 100));
  }
}

function stop(): void {
  driver?.kill();
  driver = undefined;
  if (sandbox !== undefined) rmSync(sandbox, { recursive: true, force: true });
  sandbox = undefined;
}

export const config: WebdriverIO.Config = {
  runner: 'local',
  hostname: DRIVER_HOST,
  port: DRIVER_PORT,
  specs: ['./specs/**/*.ts'],
  // One app window at a time: the app owns per-user state.
  maxInstances: 1,
  capabilities: [{ 'tauri:options': { application } }],
  logLevel: 'warn',
  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: { ui: 'bdd', timeout: 120_000 },
  waitforTimeout: 15_000,

  onPrepare: async () => {
    sandbox = mkdtempSync(join(tmpdir(), 'carrova-e2e-app-'));
    const env = isolatedEnv(sandbox);
    // Workers start after onPrepare and inherit this: specs check the sandbox.
    process.env['CARROVA_E2E_SANDBOX'] = sandbox;
    driver = spawn(
      'tauri-driver',
      ['--port', String(DRIVER_PORT), '--native-port', String(DRIVER_PORT + 1)],
      {
        env,
        stdio: ['ignore', 'inherit', 'inherit'],
      },
    );
    driver.once('error', (e) => {
      console.error(`cannot start tauri-driver: ${e.message}`);
    });
    await waitForDriver(Date.now() + DRIVER_READY_TIMEOUT_MS);
  },
  onComplete: stop,
};

// Ctrl-C / kill: never leave tauri-driver or the sandbox behind.
for (const signal of ['SIGINT', 'SIGTERM'] as const) {
  process.once(signal, () => {
    stop();
    process.exit(130);
  });
}
