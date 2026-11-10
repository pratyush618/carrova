/**
 * Task flows and their steps, in order: the single source of truth for step
 * dots, next/back and which screen a position shows.
 */
export const FLOWS = {
  send: ['found', 'code', 'live', 'done'],
  receive: ['code', 'review', 'progress', 'done'],
  save: ['what', 'lock', 'done'],
  open: ['file', 'review', 'progress', 'done'],
  // `confirm` is the preview dialog, opened over whatever offered Undo; it
  // counts as the first dot, and confirming enters the flow at `result`.
  undo: ['confirm', 'result'],
} as const satisfies Record<string, readonly [string, ...string[]]>;

export type FlowId = keyof typeof FLOWS;
export type FlowStep<F extends FlowId = FlowId> = (typeof FLOWS)[F][number];

/** A step inside a flow, typed so the step always belongs to the flow. */
export type FlowPosition = {
  [F in FlowId]: { readonly flow: F; readonly step: FlowStep<F> };
}[FlowId];

const steps = (flow: FlowId): readonly string[] => FLOWS[flow];

/** 0-based index of `at` in its flow. */
export function stepIndex(at: FlowPosition): number {
  return steps(at.flow).indexOf(at.step);
}

/** Number of steps in `flow`. */
export function flowLength(flow: FlowId): number {
  return steps(flow).length;
}

/** The first step of `flow`. */
export function flowStart(flow: FlowId): FlowPosition {
  switch (flow) {
    case 'send':
      return { flow, step: FLOWS.send[0] };
    case 'receive':
      return { flow, step: FLOWS.receive[0] };
    case 'save':
      return { flow, step: FLOWS.save[0] };
    case 'open':
      return { flow, step: FLOWS.open[0] };
    case 'undo':
      return { flow, step: FLOWS.undo[0] };
  }
}

/** The position `index` steps into `flow`, or `null` past either end. */
export function position(flow: FlowId, index: number): FlowPosition | null {
  switch (flow) {
    case 'send':
      return at(flow, FLOWS.send, index);
    case 'receive':
      return at(flow, FLOWS.receive, index);
    case 'save':
      return at(flow, FLOWS.save, index);
    case 'open':
      return at(flow, FLOWS.open, index);
    case 'undo':
      return at(flow, FLOWS.undo, index);
  }
}

function at<F extends FlowId>(
  flow: F,
  list: readonly FlowStep<F>[],
  index: number,
): { readonly flow: F; readonly step: FlowStep<F> } | null {
  const step = list[index];
  return step === undefined ? null : { flow, step };
}
