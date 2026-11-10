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
  /** No recipes are available. */
  'no-recipes',
] as const;

export type Scenario = (typeof SCENARIOS)[number];

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
