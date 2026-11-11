/** Home (first run and returning), "Use a file instead", the one-time questions. */
export const start = {
  home: {
    first: {
      title: 'Bring your setup with you.',
      lede: 'Configs, keys and editor settings, moved over your network or in an encrypted file. Apps are reinstalled, never copied.',
    },
    returning: {
      title: 'Good to see you again.',
      lede: 'Move from this computer, to it, or use a saved file.',
    },
    send: { title: 'Send from this computer', sub: 'This one has the setup you want to keep' },
    receive: {
      title: 'Receive on this computer',
      sub: "This is the new one, or the one you're setting up",
    },
    hint: 'Start here on your old computer',
    useFile: 'Use an encrypted file instead',
    useFileSub: "for when the computers can't see each other",
    how: {
      label: 'How a move works',
      pick: {
        title: 'Pick what moves',
        body: 'We find your apps and suggest what to bring. Keys stay off until you say so.',
      },
      pair: {
        title: 'Pair with a code',
        body: 'A short code, typed on the other computer. Nothing else to set up.',
      },
      review: {
        title: 'Review, then move',
        body: 'The new computer shows every change first. Every move can be undone.',
      },
    },
    promise: 'Nothing is removed from either computer. Carrova never sends anything to us.',
    recent: 'Recent moves',
    seeAll: 'See all',
    undo: 'Undo',
    undoMove: 'Undo: {{move}}',
  },
  reports: {
    title: 'Send error reports if Carrova crashes?',
    whatsSent: "What's sent",
    body: 'Off unless you turn it on. You see each report before it goes: no file names, contents or keys. You can change this in Settings.',
    allow: 'Turn on',
    deny: 'No thanks',
  },
  fileManager: {
    title: 'Add “Send with Carrova” to your file manager?',
    how: 'How it works',
    body: 'Right-click files or folders and choose Send with Carrova: Carrova opens with them picked. Only for you; you can turn it off in Settings.',
    yes: 'Add it',
    no: 'Not now',
    failed: "Couldn't add it to the file manager. Try again, or restart Carrova.",
  },
  /** A file-manager send that arrived while another move runs. */
  forwarded: { busy: 'Finish or cancel the current move first' },
  fileChoose: {
    title: 'Use an encrypted file',
    body: 'Save your setup into one file protected by a passphrase, then open it on the other computer.',
    save: { title: 'Save to a file', sub: 'On the old computer' },
    open: { title: 'Open a file', sub: 'On the new computer' },
  },
} as const;
