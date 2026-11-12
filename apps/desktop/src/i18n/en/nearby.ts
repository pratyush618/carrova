/** The Nearby page, its visibility pill, and remembered devices. */
export const nearby = {
  title: 'Nearby',
  show: 'Show nearby devices as',
  views: { map: 'Map', list: 'List' },
  me: 'This computer',
  map: 'Nearby devices',
  more_one: '+{{count}} more',
  more_other: '+{{count}} more',
  // Starts with the chip's visible text, so voice control finds it by what it shows.
  moreAll_one: '+{{more}} more: show the device in the list',
  moreAll_other: '+{{more}} more: show all {{count}} devices in the list',
  here: 'Here now',
  away: 'Known, not here',
  status: { known: 'Known', new: 'New, needs a code', away: 'Away' },
  new: 'New',
  lastMove: 'last move {{day}}',
  noMove: 'no moves yet',
  card: {
    lastMove: 'Last move',
    moves: 'Moves together',
    key: 'Key',
    never: 'Never',
  },
  actions: {
    sendWithCode: 'Send with code',
    wake: 'Wake',
    forget: 'Forget',
  },
  actionFor: '{{action}}: {{name}}',
  empty: {
    title: 'Looking for Carrova devices',
    body: 'Open Carrova on the other computer, on the same network. If it never shows up, a code always works.',
    action: 'Send with a code',
  },
  visibility: {
    label: 'Who can see this computer',
    pill: 'Visible to <b>{{who}}</b> as <b>{{name}}</b>',
    pillUnnamed: 'Visible to <b>{{who}}</b>',
    until: 'until {{time}}',
    who: { nobody: 'no one', known: 'known devices', everyone: 'everyone' },
    choices: {
      nobody: { title: 'No one', body: 'Nobody can find this computer. Codes still work.' },
      known: {
        title: 'Known devices',
        body: "Only computers you've moved with can find this computer.",
      },
      everyone: {
        title: 'Everyone for 10 minutes',
        body: 'Anyone on this network can see you. New devices still need a code.',
      },
    },
    settings: { title: 'Nearby settings', body: 'Known devices, Wake-on-LAN' },
  },
  wake: {
    sent: 'Wake sent to {{name}}. It can take a minute to show up.',
    no_hint: "No network address is saved for {{name}}, so it can't be woken from here.",
    failed: "Couldn't wake {{name}}. Try again, or start it by hand.",
  },
  failed: {
    visibility: "Couldn't change who can see this computer. Try again.",
    rename: "Couldn't rename the device. Try again.",
    forget: "Couldn't forget the device. Try again.",
    reset: "Couldn't reset this computer's identity. Nothing changed. Try again.",
  },
  remembered: 'Remembered {{name}} ({{fingerprint}})',
} as const;
