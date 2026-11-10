/** Progress screens shared by sending and receiving. */
export const transfer = {
  title: {
    moving: 'Moving your setup',
    paused: 'Paused: this disk is full',
    held: 'Paused',
    reconnecting: 'Lost the connection',
    applying: 'Putting things in place',
  },
  lede: {
    moving: 'Small settings go first, so most apps are done in seconds.',
    paused:
      'Carrova kept everything that arrived. Free up {{size}} and it picks up where it stopped.',
    held: 'Everything that arrived is kept. Resume whenever you like.',
    reconnecting: "Trying again. It'll pick up where it left off, even after sleep or a restart.",
    applying:
      'Replaced files are backed up first. If anything goes wrong, Carrova undoes it automatically.',
  },
  spaceTitle: '{{size}} more space needed',
  spaceBody: 'Checked every 5 seconds.',
  openFiles: 'Open Files',
  arrivedTotal: '{{size}} arrived',
  almostDone: 'Almost done',
  waitingSpace: 'Waiting for space',
  retrying: 'Retrying for up to 10 minutes',
  apps: 'Apps moving',
  chip: {
    waiting: 'Waiting',
    moving: 'Moving',
    paused: 'Paused',
    arrived: 'Arrived',
    inPlace: 'In place',
    placing: 'Putting in place…',
    failed: "Didn't move",
  },
  lockSend: 'Encrypted. Only {{peer}} can read this.',
  lockReceive: 'Encrypted. Each piece is checked as it arrives.',
  ignoreSpace: 'Ignore the space check',
  pause: 'Pause',
  resume: 'Resume',
  heldMeta: 'Paused by you',
  seeArrived: 'See what arrived',
  applyNote: "Can't stop now: this finishes, or undoes itself if anything fails.",
} as const;
