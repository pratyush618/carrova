import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../../support/render';
import type { LinkedDeviceView } from '@/engine/types';
import { DevicePage } from '@/screens/devices/DevicePage';
import { pixel } from '@/screens/devices/fixtures';

const on = () => ({
  onFeature: vi.fn(),
  onRing: vi.fn(),
  onStopRing: vi.fn(),
  onSendClipboard: vi.fn(),
  onForget: vi.fn(),
  onCalls: vi.fn(),
  onCallAudio: vi.fn(),
});

const withCalls = (state: 'on' | 'off'): LinkedDeviceView => ({
  ...pixel,
  features: [
    ...pixel.features,
    { feature: 'calls', group: 'quiet', local: state, remote: 'on', effective: state },
  ],
});

describe('calls on the device page', () => {
  it('opens the call log while calls are on', async () => {
    const user = userEvent.setup();
    const intents = on();
    renderScreen(<DevicePage device={withCalls('on')} on={intents} />);
    await user.click(screen.getByRole('button', { name: 'Call history' }));
    expect(intents.onCalls).toHaveBeenCalledOnce();
  });

  it('opens the call-audio setup from Set up', async () => {
    const user = userEvent.setup();
    const intents = on();
    renderScreen(<DevicePage device={withCalls('on')} on={intents} />);
    await user.click(screen.getByRole('button', { name: 'Set up' }));
    expect(intents.onCallAudio).toHaveBeenCalledOnce();
  });

  it('offers no call log while calls are off', () => {
    renderScreen(<DevicePage device={withCalls('off')} on={on()} />);
    expect(screen.queryByRole('button', { name: 'Call history' })).not.toBeInTheDocument();
  });
});
