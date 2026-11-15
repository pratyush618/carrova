import { screen } from '@testing-library/react';
import type { UserEvent } from '@testing-library/user-event';
import type { DesktopHost } from '@/engine/host';
import { createFakePort, type FakeOptions } from '@/ipc/fake';
import { renderApp } from './app';

/** The app as a phone runs it: the fake port reports Android (no step delays). */
export async function renderPhone(options: Omit<FakeOptions, 'os'> = {}, host: DesktopHost = {}) {
  const port = createFakePort({ stepMs: 0, ...options, os: 'android' });
  return renderApp({ port, host });
}

/** From the welcome screen, scan the computer's code and wait until it is linked. */
export async function scanToLinked(user: UserEvent) {
  await user.click(await screen.findByRole('button', { name: 'Add a computer' }));
  await user.click(await screen.findByRole('button', { name: 'Scan the code' }));
  await screen.findByRole('heading', { level: 1, name: 'Linked' });
}
