/** The flows the fake port can act out (`?fake=<scenario>` in dev and tests). */
export const SCENARIOS = [
  /** Everything works. */
  'happy',
  /** The receiver's disk fills up mid-transfer; the transfer pauses, then resumes. */
  'disk-full',
  /** The LAN connection drops during the transfer and comes back. */
  'reconnect',
  /** Someone enters a wrong code; the sender is asked whether to show a new one. */
  'wrong-code',
  /** Apply fails part-way and is rolled back. */
  'apply-fails',
  /** The plan has a blocker (an app is running); approve is refused. */
  'blocked',
  /** The archive comes from a Mac: paths are mapped and one item only works on macOS. */
  'cross-os',
  /** A plugin would change an item's files; the receiver is asked to allow it. */
  'plugin',
  /** No recipes are available. */
  'no-recipes',
  /** "Check now" finds a newer recipe list; it installs when asked. */
  'recipes-update',
  /** The archive holds Manual picks: two safe ones ("Accept all") and an SSH key. */
  'picks',
  /** The app was opened from the file manager's "Send with Carrova" with two paths. */
  'forwarded',
  /** Two known devices and a new one nearby, two known ones away (one can be woken). */
  'nearby',
  /** 21 new devices, one with a very long name, on a computer with a long name too: more than the map has room for. */
  'nearby-crowded',
  /** A phone (Handoff off on its side) and a computer are linked; three clips arrived. */
  'linked',
  /**
   * Trusted computers send without a code, with `nearby`'s devices and
   * `linked`'s links: Desk PC opens Review, Ana's laptop declines.
   */
  'known-send',
  /** As `known-send`, and Desk PC asks to send this computer a move. */
  'move-request',
] as const;

export type Scenario = (typeof SCENARIOS)[number];

/** Whether trusted computers send without a code in `scenario` (`about().known_send`). */
export const knownSend = (scenario: Scenario): boolean =>
  scenario === 'known-send' || scenario === 'move-request';

function isScenario(value: string): value is Scenario {
  return SCENARIOS.some((scenario) => scenario === value);
}

/** The scenario named by `?fake=<name>` in `search`, if any. */
export function scenarioFromSearch(search: string): Scenario | undefined {
  const name = new URLSearchParams(search).get('fake');
  if (name === null) {
    return undefined;
  }
  return isScenario(name) ? name : 'happy';
}
