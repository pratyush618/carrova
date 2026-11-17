import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { CarrovaPort } from '@/engine/types';
import { useCallsSetup } from '@/flows/calls/useCallsSetup';
import { createFakePort } from '@/ipc/fake';
import { CallsSetupDialog } from '@/screens/phone/calls-setup/CallsSetupDialog';

function Wizard({ port }: { readonly port: CarrovaPort }) {
  return <CallsSetupDialog setup={useCallsSetup(port)} onClose={() => undefined} />;
}

describe('Calls setup', () => {
  it('install_needs_agree_before_config', async () => {
    const user = userEvent.setup();
    const port = createFakePort({ stepMs: 0 });
    vi.spyOn(port, 'callAudioStep').mockResolvedValueOnce('install').mockResolvedValue('pair');
    const config = vi.spyOn(port, 'setCallAudioConfig');
    render(<Wizard port={port} />);
    await user.click(screen.getByRole('button', { name: 'Start' }));
    await screen.findByRole('heading', { name: 'Install one package' });
    await user.click(screen.getByRole('button', { name: 'Install' }));
    expect(await screen.findByText('Installed')).toBeInTheDocument();
    const next = screen.getByRole('button', { name: 'Continue' });
    expect(next).toBeDisabled();
    await user.click(next);
    expect(config).not.toHaveBeenCalled();
    await user.click(screen.getByRole('checkbox', { name: 'I agree to this change' }));
    await user.click(next);
    expect(config).toHaveBeenCalledWith(true);
    await screen.findByRole('heading', { name: 'Pair your phone' });
  });

  it('offers talking on the phone when this computer has no hands-free audio', async () => {
    const user = userEvent.setup();
    const port = createFakePort({ stepMs: 0 });
    vi.spyOn(port, 'callAudioStep').mockResolvedValue('no_bt');
    render(<Wizard port={port} />);
    await user.click(screen.getByRole('button', { name: 'Start' }));
    await user.click(await screen.findByRole('button', { name: 'Talk on the phone' }));
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'Talk on the phone' })).toBeInTheDocument();
    });
  });
});
