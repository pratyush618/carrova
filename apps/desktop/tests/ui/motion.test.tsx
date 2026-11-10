import { act, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CodeDisplay, Handshake, ReduceMotionOverride, Seal } from '@/ui';
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

describe('Seal', () => {
  it('stamps in with a one-off burst normally', () => {
    render(<Seal ring="ARRIVED · ALL CHECKED · " label="Everything arrived" />);
    expect(screen.getByRole('img', { name: 'Everything arrived' })).toBeInTheDocument();
    expect(screen.getByTestId('seal-burst').children).toHaveLength(16);
  });

  it('is simply there under reduced motion', () => {
    render(
      <ReduceMotionOverride value>
        <Seal ring="ARRIVED · ALL CHECKED · " label="Everything arrived" />
      </ReduceMotionOverride>,
    );
    expect(screen.getByRole('img', { name: 'Everything arrived' })).toBeInTheDocument();
    expect(screen.queryByTestId('seal-burst')).not.toBeInTheDocument();
  });
});

describe('Handshake', () => {
  it('is decorative and shows each computer with its OS once known', () => {
    const { container } = render(
      <Handshake
        here="ana-laptop"
        there="ana-mbp"
        linked
        hereOs={{ kind: 'linux', label: 'Linux' }}
        thereOs={null}
      />,
    );
    const stage = container.firstElementChild;
    expect(stage).toHaveAttribute('aria-hidden', 'true');
    expect(stage).toHaveClass('p-handshake--linked');
    expect(stage).toHaveTextContent('ana-laptopLinuxana-mbp');
  });

  it('shuttles a dot along the wire until the code is proven', () => {
    const { container } = render(<Handshake here="a" there="b" linked={false} />);
    expect(container.querySelector('.p-handshake__dot')).not.toBeNull();
  });
});
