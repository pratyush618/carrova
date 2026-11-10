/** Welcome, Home, "Use a file instead". */
export const start = {
  welcome: {
    title: 'Bring your setup to a new computer.',
    lede: 'Settings, keys and data for the apps you use. You choose what moves; the apps themselves get reinstalled.',
    promises: 'What Carrova promises',
    direct: {
      title: 'Straight between your computers',
      sub: 'No account, no cloud. Everything is encrypted on the way.',
    },
    approve: {
      title: 'Nothing changes until you approve',
      sub: 'You see exactly what will change on the new computer first.',
    },
    undo: {
      title: 'Every move can be undone',
      sub: 'Replaced files are backed up and kept until you remove them.',
    },
    reports: {
      title: 'Send error reports if Carrova crashes',
      sub: 'No file names, contents or keys. You can change this in Settings.',
    },
    start: 'Get started',
  },
  home: {
    title: 'What are we doing on this computer?',
    send: { title: 'Send', sub: 'This is the old computer' },
    receive: { title: 'Receive', sub: 'This is the new computer' },
    useFile: 'Use an encrypted file instead',
    lastReceived: 'Received from {{peer}} {{day}}, {{apps}}.',
    lastSent: 'Sent to {{peer}} {{day}}, {{apps}}.',
    undo: 'Undo',
  },
  fileChoose: {
    title: 'Use an encrypted file',
    body: 'Save your setup into one file protected by a passphrase, then open it on the other computer.',
    save: { title: 'Save to a file', sub: 'On the old computer' },
    open: { title: 'Open a file', sub: 'On the new computer' },
  },
} as const;
