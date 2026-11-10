import { render, type RenderResult } from '@testing-library/react';
import { App } from '@/app/App';
import type { DesktopHost } from '@/engine/host';
import type { CarrovaPort } from '@/engine/types';
import { createFakePort, type FakeOptions, type Scenario } from '@/ipc/fake';

export interface AppOptions {
  readonly scenario?: Scenario;
  /** Error-report consent already answered (Home doesn't ask). */
  readonly returning?: boolean;
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
  const port = createFakePort({
    scenario: options.scenario ?? 'happy',
    stepMs: options.stepMs ?? 0,
    ...(options.sleep === undefined ? {} : { sleep: options.sleep }),
  });
  if (options.returning === true) await port.setErrorReportConsent('denied');
  return { ...render(<App port={port} host={options.host ?? {}} />), port };
}
