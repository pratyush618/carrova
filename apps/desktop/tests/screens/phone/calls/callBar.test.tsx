import { screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../../support/render';
import { CallBar } from '@/screens/phone/calls/CallBar';

const on = { onAnswer: vi.fn(), onDecline: vi.fn(), onMute: vi.fn(), onEnd: vi.fn() };

describe('CallBar', () => {
  it('keeps the running timer out of the announced status', () => {
    renderScreen(
      <CallBar
        view={{
          kind: 'active',
          device: 'p8',
          timer: '00:42',
          muted: false,
          audio: 'phone',
          keypad: false,
        }}
        who="Sam Rivera"
        phone="Pixel 8"
        left={false}
        on={on}
      />,
    );
    const status = screen.getByRole('status');
    expect(status).toHaveTextContent('Sam Rivera');
    expect(status).not.toHaveTextContent('00:42');
    expect(screen.getByText('00:42', { exact: false })).toBeInTheDocument();
  });
});
