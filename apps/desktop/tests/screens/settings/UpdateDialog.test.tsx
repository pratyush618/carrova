import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { UPDATE } from '@/screens/settings/fixtures';
import type { UpdateIntents, UpdateModel } from '@/screens/settings/model';
import { UpdateDialog } from '@/screens/settings/UpdateDialog';

const intents = (): UpdateIntents => ({
  onLater: vi.fn(),
  onInstall: vi.fn(),
  onCancel: vi.fn(),
  onInstallAnyway: vi.fn(),
  onRestart: vi.fn(),
});

function show(model: UpdateModel | undefined, on = intents()) {
  if (model === undefined) throw new Error('missing fixture');
  renderScreen(<UpdateDialog model={model} on={on} />);
  return on;
}

describe('UpdateDialog', () => {
  it('shows the download as measured, and cancels it', async () => {
    const on = show(UPDATE['downloading']);
    expect(screen.getByRole('heading', { name: 'Downloading Carrova 0.2.0' })).toBeInTheDocument();
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '39');
    expect(screen.getByText('39% · 38 MB of 96 MB · 4.1 MB/s')).toBeInTheDocument();
    expect(screen.getByText('about 15 seconds left')).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(on.onCancel).toHaveBeenCalledOnce();
  });

  it('says there is no estimate without a usable speed', () => {
    show({
      status: 'downloading',
      version: '0.2.0',
      progress: { done: 1_000, total: null, percent: null, bytes_per_sec: null, secs_left: null },
    });
    expect(screen.getByText('1 kB downloaded')).toBeInTheDocument();
    expect(screen.getByText('No estimate')).toBeInTheDocument();
    expect(screen.getByRole('progressbar')).not.toHaveAttribute('aria-valuenow');
  });

  it('checks the signature without a made-up percent', () => {
    show(UPDATE['verifying']);
    expect(screen.getByRole('heading', { name: 'Checking the signature' })).toBeInTheDocument();
    expect(screen.queryByRole('progressbar')).not.toBeInTheDocument();
  });

  it('waits for a move, and installs anyway only after the warning', async () => {
    const on = show(UPDATE['waiting']);
    expect(
      screen.getByRole('heading', { name: 'Waiting until the move finishes' }),
    ).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Install anyway…' }));
    expect(on.onInstallAnyway).not.toHaveBeenCalled();
    expect(screen.getByRole('alertdialog', { name: 'Install while the move runs?' })).toBeVisible();

    await userEvent.click(screen.getByRole('button', { name: 'Wait for it' }));
    expect(on.onInstallAnyway).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Install anyway…' }));
    await userEvent.click(screen.getByRole('button', { name: 'Install anyway' }));
    expect(on.onInstallAnyway).toHaveBeenCalledOnce();
  });

  it('asks to restart once installed', async () => {
    const on = show(UPDATE['installed']);
    expect(screen.getByRole('heading', { name: 'Restart to finish' })).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Restart now' }));
    expect(on.onRestart).toHaveBeenCalledOnce();
  });

  it('offers to download a failed install again', async () => {
    const on = show(UPDATE['failed']);
    await userEvent.click(screen.getByRole('button', { name: 'Download again' }));
    expect(on.onInstall).toHaveBeenCalledOnce();
  });

  it('has nothing to retry when the check failed', () => {
    show({
      status: 'failed',
      version: null,
      error: { code: 'network', message: 'Offline', hint: null },
    });
    expect(screen.queryByRole('button', { name: 'Download again' })).not.toBeInTheDocument();
  });
});
