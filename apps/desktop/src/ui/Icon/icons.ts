/**
 * Icon paths on a 24×24 grid, stroked with `currentColor` (from the mockups'
 * sprites; circles written as arcs so every icon is a list of paths).
 */
export const ICONS = {
  send: ['M12 15V4M7 9l5-5 5 5M5 14v4a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-4'],
  receive: ['M12 4v11M7 10l5 5 5-5M5 14v4a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-4'],
  file: [
    'M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8zM14 3v5h5M9.5 15v-1.5a2.5 2.5 0 0 1 5 0V15M9 15h6v3H9z',
  ],
  lock: ['M7 11V8a5 5 0 0 1 10 0v3M5.5 11h13v9.5h-13z'],
  check: ['M5 12.5l4.5 4.5L19 7'],
  close: ['M6 6l12 12M18 6L6 18'],
  chev: ['M9 6l6 6-6 6'],
  back: ['M15 6l-6 6 6 6'],
  history: ['M3.5 12a8.5 8.5 0 1 0 2.6-6.1M3.5 4v4.5H8M12 7.5V12l3 2'],
  apps: ['M4.5 4.5h6v6h-6zM13.5 4.5h6v6h-6zM4.5 13.5h6v6h-6zM13.5 13.5h6v6h-6z'],
  settings: [
    'M4 7h9M17 7h3M4 17h3M11 17h9',
    'M13 7a2 2 0 1 0 4 0a2 2 0 1 0-4 0',
    'M7 17a2 2 0 1 0 4 0a2 2 0 1 0-4 0',
  ],
  undo: ['M9 14L4 9l5-5M4 9h10.5a5.5 5.5 0 0 1 0 11H11'],
  key: ['M4 15a4 4 0 1 0 8 0a4 4 0 1 0-8 0', 'M11 12l8-8M16 7l2.5 2.5M14 9l1.5 1.5'],
  warn: ['M12 4L2.5 20h19zM12 10v4.5M12 17.2v.3'],
  info: ['M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0', 'M12 11v5.5M12 7.8v.3'],
  laptop: ['M5 5.5h14v10H5zM2.5 19h19'],
  search: ['M4.5 11a6.5 6.5 0 1 0 13 0a6.5 6.5 0 1 0-13 0', 'M20 20l-4.3-4.3'],
  copy: ['M9 9h11v11H9zM5 15H4V4h11v1'],
  pause: ['M9 5v14M15 5v14'],
  play: ['M8 5.5v13l10.5-6.5z'],
  plug: ['M9 3v5M15 3v5M6.5 8h11v3a5.5 5.5 0 0 1-11 0zM12 16.5V21'],
  wifi: ['M2.5 9a14 14 0 0 1 19 0M5.5 12.5a9.5 9.5 0 0 1 13 0M8.5 16a5 5 0 0 1 7 0M12 19.5v.1'],
  disk: ['M3.5 13.5l2.5-8h12l2.5 8v5h-17zM3.5 13.5h17M16.5 16.5v.1'],
  folder: [
    'M3.5 6.5a1.5 1.5 0 0 1 1.5-1.5h4.5l2 2.5H19a1.5 1.5 0 0 1 1.5 1.5v9a1.5 1.5 0 0 1-1.5 1.5H5a1.5 1.5 0 0 1-1.5-1.5z',
  ],
  swap: ['M7 7h13M16 3l4 4-4 4M17 17H4M8 13l-4 4 4 4'],
  // v2 additions (mockup `P`, scaled from its 20-unit grid).
  home: ['M4.2 10.8 12 4.2 19.8 10.8v9h-15.6zM9.6 19.8V14.4h4.8v5.4'],
  box: ['M12 3.36 19.8 7.68v8.64L12 20.64 4.2 16.32V7.68zM4.2 7.68 12 12l7.8-4.32M12 12v8.64'],
  shield: [
    'M12 3 19.2 6v6c0 4.68-3.24 7.68-7.2 9-3.96-1.32-7.2-4.32-7.2-9V6zM8.88 12l2.28 2.28 3.96-4.32',
  ],
  arrow: ['M4.8 12h14.4M14.4 7.2l4.8 4.8-4.8 4.8'],
  minus: ['M6 12h12'],
  running: ['M12 3.6a8.4 8.4 0 1 0 0 16.8 8.4 8.4 0 0 0 0-16.8zM9.6 9v6M14.4 9v6'],
  noNet: [
    'M12 3.6a8.4 8.4 0 1 0 0 16.8 8.4 8.4 0 0 0 0-16.8zM3.6 12h16.8M12 3.6c2.28 2.4 3.36 5.28 3.36 8.4s-1.08 6-3.36 8.4c-2.28-2.4-3.36-5.28-3.36-8.4S9.72 6 12 3.6M4.8 4.8l14.4 14.4',
  ],
  eye: [
    'M2.4 12s3.6-6.6 9.6-6.6S21.6 12 21.6 12s-3.6 6.6-9.6 6.6S2.4 12 2.4 12zM12 9.6a2.4 2.4 0 1 0 0 4.8 2.4 2.4 0 0 0 0-4.8z',
  ],
  pen: ['M4.8 19.2l1.2-4.8L15.6 4.8l3.6 3.6-9.6 9.6zM13.8 6.6l3.6 3.6'],
  terminal: ['M3.6 4.8h16.8v14.4H3.6zM7.2 9.6l3 2.4L7.2 14.4M12.6 14.4H16.8'],
  cpu: [
    'M7.2 7.2h9.6v9.6H7.2zM9.6 3.6v3.6M14.4 3.6v3.6M9.6 16.8v3.6M14.4 16.8v3.6M3.6 9.6h3.6M3.6 14.4h3.6M16.8 9.6h3.6M16.8 14.4h3.6',
  ],
  clock: ['M12 3.6a8.4 8.4 0 1 0 0 16.8 8.4 8.4 0 0 0 0-16.8zM12 7.8V12l3 1.8'],
  archive: ['M3.6 5.4h16.8v4.2H3.6zM5.4 9.6v9h13.2V9.6M9.6 13.2h4.8'],
  external: ['M10.2 4.8H4.8v14.4h14.4v-5.4M13.8 4.8H19.2v5.4M19.2 4.8l-8.4 8.4'],
  // Settings sections (mockup `P`, scaled from its 20-unit grid).
  bell: ['M6.6 16.2V10.8a5.4 5.4 0 0 1 10.8 0v5.4l1.8 1.8H4.8zM10.2 20.4h3.6'],
  radar: [
    'M12 18.6v.2M8.64 15.36a4.8 4.8 0 0 1 6.72 0M5.52 12.24a9.12 9.12 0 0 1 12.96 0M2.64 9.12a13.2 13.2 0 0 1 18.72 0',
  ],
  refresh: ['M19.2 12a7.2 7.2 0 1 1-2.16-5.16M19.2 4.2v3.6h-3.6'],
} as const satisfies Record<string, readonly string[]>;

export type IconName = keyof typeof ICONS;

/** Icons that point along the reading direction and mirror in RTL. */
export const DIRECTIONAL: ReadonlySet<IconName> = new Set<IconName>(['chev', 'back', 'arrow']);
