import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { PhoneUpdate, type PhoneUpdateIntents } from '@/mobile/update/PhoneUpdate';
import type { UpdateModel } from '@/screens/settings/model';

const intents = (): PhoneUpdateIntents => ({
  onBack: vi.fn(),
  onDownload: vi.fn(),
  onDownloadOnMobileData: vi.fn(),
  onCancel: vi.fn(),
  onPause: vi.fn(),
  onUseMobileData: vi.fn(),
  onOpenPermission: vi.fn(),
  onReleaseNotes: vi.fn(),
});

const OFFER: UpdateModel = { status: 'available', version: '0.4.0', notes: 'Faster transfers' };
const PROGRESS = {
  done: 18_400_000,
  total: 52_100_000,
  percent: 35,
  bytes_per_sec: 3_200_000,
  secs_left: 11,
};

function show(model: UpdateModel, askNetwork = false) {
  const on = intents();
  renderScreen(<PhoneUpdate model={model} current="0.3.0" askNetwork={askNetwork} on={on} />);
  return on;
}

describe('the phone update page', () => {
  it("offers the release with what's new, and downloads on the user's tap", async () => {
    const on = show(OFFER);
    expect(screen.getByText('Carrova 0.4.0')).toBeInTheDocument();
    expect(screen.getByText('You have 0.3.0')).toBeInTheDocument();
    expect(screen.getByText('Faster transfers')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Full notes' }));
    expect(on.onReleaseNotes).toHaveBeenCalledOnce();

    await userEvent.click(screen.getByRole('button', { name: 'Download' }));
    expect(on.onDownload).toHaveBeenCalledOnce();
  });

  it('waits for Wi-Fi on mobile data unless the user picks mobile data', async () => {
    const on = show(OFFER, true);
    expect(screen.getByRole('radio', { name: 'Wait for Wi-Fi' })).toBeChecked();
    await userEvent.click(screen.getByRole('button', { name: 'Download on Wi-Fi' }));
    expect(on.onDownload).toHaveBeenCalledOnce();

    await userEvent.click(screen.getByRole('radio', { name: 'Use mobile data' }));
    await userEvent.click(screen.getByRole('button', { name: 'Download' }));
    expect(on.onDownloadOnMobileData).toHaveBeenCalledOnce();
  });

  it('shows the download as measured, and pauses it', async () => {
    const on = show({ status: 'downloading', version: '0.4.0', progress: PROGRESS });
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '35');
    await userEvent.click(screen.getByRole('button', { name: 'Pause' }));
    expect(on.onPause).toHaveBeenCalledWith(true);
    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(on.onCancel).toHaveBeenCalledOnce();
  });

  it('resumes a paused download from where it stopped', async () => {
    const still = { ...PROGRESS, bytes_per_sec: null, secs_left: null };
    const on = show({ status: 'paused', version: '0.4.0', progress: still });
    expect(screen.getByText('It picks up from here when you resume.')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Resume' }));
    expect(on.onPause).toHaveBeenCalledWith(false);
  });

  it('waits for Wi-Fi, or goes on mobile data when asked', async () => {
    const on = show({ status: 'waiting', version: '0.4.0', blocker: 'wifi' });
    expect(screen.getByText("Wi-Fi isn't on")).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Use mobile data' }));
    expect(on.onUseMobileData).toHaveBeenCalledOnce();
  });

  it("opens Android's install permission when Carrova may not install apps", async () => {
    const on = show({ status: 'permission', version: '0.4.0' });
    expect(screen.getByText('Turn on Allow from this source')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Open Android settings' }));
    expect(on.onOpenPermission).toHaveBeenCalledOnce();
  });

  it("waits for Android's installer, and can open it again", async () => {
    const on = show({ status: 'installed', version: '0.4.0' });
    expect(screen.getByText('Checked · waiting for Android')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Open the installer' }));
    expect(on.onDownload).toHaveBeenCalledOnce();
  });

  it('says a rejected download changed nothing, and offers it again', async () => {
    const on = show({
      status: 'failed',
      version: '0.4.0',
      error: {
        code: 'bad_update',
        message: "the download is not signed with Carrova's release key",
        hint: null,
      },
    });
    expect(screen.getByRole('alert')).toHaveTextContent("The update didn't install");
    expect(screen.getByRole('alert')).toHaveTextContent("Nothing changed: you're still on 0.3.0.");
    await userEvent.click(screen.getByRole('button', { name: 'Download again' }));
    expect(on.onDownload).toHaveBeenCalledOnce();
  });
});
