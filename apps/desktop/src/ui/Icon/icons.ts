/**
 * Icon paths on a 24×24 grid, stroked with `currentColor` (from the mockup's
 * sprite; circles written as arcs so every icon is a list of paths).
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
  plug: ['M9 3v5M15 3v5M6.5 8h11v3a5.5 5.5 0 0 1-11 0zM12 16.5V21'],
  wifi: ['M2.5 9a14 14 0 0 1 19 0M5.5 12.5a9.5 9.5 0 0 1 13 0M8.5 16a5 5 0 0 1 7 0M12 19.5v.1'],
  disk: ['M3.5 13.5l2.5-8h12l2.5 8v5h-17zM3.5 13.5h17M16.5 16.5v.1'],
  folder: [
    'M3.5 6.5a1.5 1.5 0 0 1 1.5-1.5h4.5l2 2.5H19a1.5 1.5 0 0 1 1.5 1.5v9a1.5 1.5 0 0 1-1.5 1.5H5a1.5 1.5 0 0 1-1.5-1.5z',
  ],
  spark: ['M12 3v4M12 17v4M3 12h4M17 12h4M6 6l2.5 2.5M15.5 15.5L18 18M18 6l-2.5 2.5M8.5 15.5L6 18'],
  swap: ['M7 7h13M16 3l4 4-4 4M17 17H4M8 13l-4 4 4 4'],
} as const satisfies Record<string, readonly string[]>;

export type IconName = keyof typeof ICONS;

/** Icons that point along the reading direction and mirror in RTL. */
export const DIRECTIONAL: ReadonlySet<IconName> = new Set<IconName>(['chev', 'back']);
