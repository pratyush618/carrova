/** The one override dialog: what happens, what's still safe, a button naming the exact risk. */
export const override = {
  keepSafe: 'Keep it safe',
  unmovable: {
    title: 'Include {{item}} anyway?',
    suggestion_one: "Carrova's suggestion: reinstall it. You'll get one command for it.",
    suggestion_other:
      "Carrova's suggestion: reinstall them. You'll get one command that does all {{count}}.",
    go: 'Include anyway',
  },
  running: {
    title: 'Apply while {{app}} is open?',
    body: "{{app}} saves its state when it closes. If it's open while Carrova replaces that state, it will likely write over it, leaving a mix of old and new.",
    safe: "Your other apps aren't affected, and Undo still works.",
    go: "Apply while it's open",
  },
  space: {
    title: 'Ignore the free-space check?',
    body: 'Carrova checks space so a move can’t fill your disk. If the disk fills anyway, other apps may fail to save files.',
    safe: 'Carrova itself stays safe: it pauses and keeps what arrived.',
    go: 'Ignore and continue',
  },
  cross_os: {
    title: 'Bring {{item}} anyway?',
    body: 'Its recipe keeps it on the system it came from ({{from}}). Carrova would restore it to the same place as on {{from}}, and apps on {{to}} may never read it.',
    safe: 'Undo removes it like everything else.',
    understand: 'I understand apps on {{to}} may not use it',
    keep: 'Keep it behind',
    go: 'Bring anyway',
  },
  elsewhere: {
    title: 'Put this somewhere else?',
    body: 'Pick a folder inside your home, config or data folders. Carrova refuses anywhere outside them.',
    safe: 'The app may not find its settings in the new place.',
    go: 'Choose a folder',
  },
} as const;
