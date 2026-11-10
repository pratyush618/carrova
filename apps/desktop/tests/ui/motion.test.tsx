import { act, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Arrival, CodeDisplay, ReduceMotionOverride } from '@/ui';
import { revealDuration } from '@/ui/CodeDisplay/flaps';

const tiles = () =>
  screen.getByRole('img', { name: 'Pairing code: 7 zebra lemon' }).querySelectorAll('.p-flap');
const letters = () => Array.from(tiles(), (t) => t.textContent).join('');

function stubReducedMotionMedia(matches: boolean) {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: matches && query.includes('reduce'),
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }));
}

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('CodeDisplay', () => {
  it('names the whole code once for screen readers', () => {
    render(<CodeDisplay words={['7', 'zebra', 'lemon']} />);
    expect(tiles()).toHaveLength(11);
    expect(letters()).toBe('7zebralemon');
  });

  it('flips tiles in and settles every one on its real character', () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval', 'performance'] });
    render(<CodeDisplay words={['7', 'zebra', 'lemon']} reveal />);
    act(() => {
      vi.advanceTimersByTime(60);
    });
    expect(letters()).not.toBe('7zebralemon');
    act(() => {
      vi.advanceTimersByTime(revealDuration(11) + 120);
    });
    expect(letters()).toBe('7zebralemon');
    expect(Array.from(tiles(), (t) => t.getAttribute('data-settled'))).not.toContain('false');
  });

  it('shows the code at once when the user reduces motion', () => {
    render(
      <ReduceMotionOverride value>
        <CodeDisplay words={['7', 'zebra', 'lemon']} reveal />
      </ReduceMotionOverride>,
    );
    expect(letters()).toBe('7zebralemon');
  });

  it('shows the code at once when the OS asks for reduced motion', () => {
    stubReducedMotionMedia(true);
    render(<CodeDisplay words={['7', 'zebra', 'lemon']} reveal />);
    expect(letters()).toBe('7zebralemon');
  });
});

describe('Arrival', () => {
  it('lands with a burst normally', () => {
    render(<Arrival word="Arrived" meta="5 apps, 1.2 GB" />);
    expect(screen.getByRole('img', { name: 'Arrived. 5 apps, 1.2 GB' })).toBeInTheDocument();
    expect(screen.getByTestId('arrival-burst').children).toHaveLength(14);
  });

  it('drops the burst under reduced motion', () => {
    render(
      <ReduceMotionOverride value>
        <Arrival word="Arrived" meta="5 apps, 1.2 GB" />
      </ReduceMotionOverride>,
    );
    expect(screen.queryByTestId('arrival-burst')).not.toBeInTheDocument();
  });
});
