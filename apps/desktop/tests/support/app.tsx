import { render, type RenderResult } from '@testing-library/react';
import type { Settings } from '@/bindings/Settings';
import { App } from '@/app/App';
import type { DesktopHost } from '@/engine/host';
import type { CarrovaPort } from '@/engine/types';
import { createFakePort, type FakeOptions, type Scenario } from '@/ipc/fake';

export interface AppOptions {
  readonly scenario?: Scenario;
  /** Error-report consent and the file-manager question already answered (Home doesn't ask). */
  readonly returning?: boolean;
  /** Settings saved before the app starts. */
  readonly settings?: Partial<Settings>;
  /** A port to use instead of a fresh fake (scenario, step and sleep options are then ignored). */
  readonly port?: CarrovaPort;
  readonly host?: DesktopHost;
  /** Fake step delay; 0 runs every task straight through. */
  readonly stepMs?: number;
  /** The fake's clock (default: real timers); lets a test hold a task mid-way. */
  readonly sleep?: NonNullable<FakeOptions['sleep']>;
}

/** Renders the live app over the in-memory fake port (no step delays). */
export async function renderApp(
  options: AppOptions = {},
): Promise<RenderResult & { port: CarrovaPort }> {
  const port =
    options.port ??
    createFakePort({
      scenario: options.scenario ?? 'happy',
      stepMs: options.stepMs ?? 0,
      ...(options.sleep === undefined ? {} : { sleep: options.sleep }),
    });
  const returning = options.returning === true;
  if (returning) await port.setErrorReportConsent('denied');
  if (returning || options.settings !== undefined) {
    const saved = await port.settings();
    await port.saveSettings({
      ...saved,
      ...(returning && { file_manager_asked: true }),
      ...options.settings,
    });
  }
  return { ...render(<App port={port} host={options.host ?? {}} />), port };
}
