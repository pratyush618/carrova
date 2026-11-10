import { render, type RenderResult } from '@testing-library/react';
import type { ReactElement, ReactNode } from 'react';
import { ChromeContext, ReduceMotionOverride, Window, type Chrome } from '@/ui';

const noop = (): void => undefined;

/** Chrome for a flow at step 1 of 4 (enough for any layout). */
export const testChrome: Chrome = {
  hubLinks: [],
  hubStatus: null,
  flow: { name: 'Send', steps: ['Pick', 'Pair', 'Their review', 'Move'], step: 1, onClose: noop },
};

export interface ScreenOptions {
  readonly reduceMotion?: boolean;
  readonly overlays?: readonly ReactNode[];
}

/** Renders a screen the way the shell does: inside chrome and a window. */
export function renderScreen(ui: ReactElement, options: ScreenOptions = {}): RenderResult {
  return render(
    <ReduceMotionOverride value={options.reduceMotion ?? false}>
      <ChromeContext value={testChrome}>
        <Window overlays={options.overlays ?? []}>{ui}</Window>
      </ChromeContext>
    </ReduceMotionOverride>,
  );
}
