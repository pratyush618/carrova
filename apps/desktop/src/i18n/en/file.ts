/** Encrypted file: save and open. */
export const file = {
  lock: {
    title: 'Lock the file with a passphrase',
    lede: "You'll type it on the other computer. Carrova can't recover it if it's lost.",
    passphrase: 'Passphrase',
    again: 'Type it again',
    strength: {
      weak: 'Too easy to guess. Try three or four unrelated words.',
      fair: 'Getting there. Another word makes it much stronger.',
      good: 'Good. One more word makes it great.',
      strong: 'Strong. Four words or more is great.',
    },
    mismatch: "The two passphrases don't match yet.",
    where: 'Save to',
    choose: 'Choose where…',
    kdf: 'Protection',
    kdfLevels: {
      standard: {
        label: 'Standard (suggested)',
        consequence: 'Uses 256 MB of memory; locking and opening take about a second.',
      },
      strong: {
        label: 'Strong',
        consequence:
          'Uses 512 MB of memory; locking and opening take 2–3 times as long. Harder to guess.',
      },
      maximum: {
        label: 'Maximum',
        consequence:
          'Uses 1 GB of memory; locking and opening take 6–8 times as long. The other computer needs that much free memory to open it.',
      },
    },
    kdfNote:
      'The passphrase is stretched with Argon2id at strong settings, so guessing it takes a very long time.',
    save: 'Save file',
    saving: 'Saving…',
  },
  saved: {
    ring: 'PACKED · ENCRYPTED · PACKED · ENCRYPTED · ',
    seal: 'Packed and encrypted',
    title: 'Your setup is in one file.',
    lede: '{{size}}, encrypted. Copy it to the other computer any way you like, then choose Open a file there.',
    showInFolder: 'Show in folder',
  },
  open: {
    title: 'Open your Carrova file',
    file: 'File',
    fileHint: 'The .carrova file saved on the other computer.',
    filePlaceholder: '~/carrova-setup.carrova',
    another: 'Choose a file…',
    opening: 'Opening…',
    passphrase: 'Passphrase',
    wrong: "That passphrase doesn't open this file. Check caps lock and try again.",
    open: 'Open',
  },
} as const;
