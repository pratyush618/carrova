import type { AppRef } from './model';

/** One fictional session: ezio-desktop sends to ezio-thinkpad-new (from the mockup). */
export const SENDER = 'ezio-desktop';
export const RECEIVER = 'ezio-thinkpad-new';
export const NOW = new Date(2026, 10, 8, 10, 30);
export const YESTERDAY = new Date(2026, 10, 7, 14, 2);

export const GB = 1_000_000_000;
export const MB = 1_000_000;
export const KB = 1_000;

/** An app with its total size on the sending computer. */
export interface SizedApp extends AppRef {
  readonly bytes: number;
}

/** Apps of the fictional session. */
export const APPS = {
  git: { id: 'git', glyph: 'git', name: 'Git', bytes: 4 * KB },
  ssh: { id: 'ssh', glyph: '>_', name: 'SSH', bytes: 14 * KB },
  gpg: { id: 'gpg', glyph: 'gpg', name: 'GnuPG', bytes: 96 * KB },
  shell: { id: 'shell', glyph: '$_', name: 'Bash and Zsh', bytes: 2.3 * MB },
  vscode: { id: 'vscode', glyph: '{ }', name: 'VS Code', bytes: 1.2 * GB },
} as const satisfies Record<string, SizedApp>;

/** The engine's transfer order: small configs first. */
export const APP_ORDER: readonly SizedApp[] = [
  APPS.git,
  APPS.ssh,
  APPS.gpg,
  APPS.shell,
  APPS.vscode,
];

export const TOTAL_BYTES = 1.2 * GB;
