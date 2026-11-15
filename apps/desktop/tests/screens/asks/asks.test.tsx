import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { AskToast } from '@/screens/asks/AskToast';
import { ClipToast } from '@/screens/asks/ClipToast';
import { TransferToast } from '@/screens/asks/TransferToast';

describe('Asks', () => {
  it('AskToast accept calls answerAsk(true)', async () => {
    const user = userEvent.setup();
    const onAnswer = vi.fn();
    renderScreen(
      <AskToast
        device="Pixel 8"
        kind={{ kind: 'file', name: 'report.pdf', size: 2_400_000 }}
        onAnswer={onAnswer}
      />,
    );
    expect(screen.getByText(/Pixel 8 wants to send you report.pdf/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Accept' }));
    expect(onAnswer).toHaveBeenCalledWith(true);
  });

  it('asks before opening a link, naming its host only', async () => {
    const user = userEvent.setup();
    const onAnswer = vi.fn();
    renderScreen(
      <AskToast
        device="Pixel 8"
        kind={{ kind: 'handoff', url_host: 'docs.rs', file: null }}
        onAnswer={onAnswer}
      />,
    );
    expect(screen.getByText(/Pixel 8 sent a link · docs.rs/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Not now' }));
    expect(onAnswer).toHaveBeenCalledWith(false);
  });

  it('says a password stayed here', () => {
    renderScreen(<ClipToast reason="sensitive" onDismiss={vi.fn()} />);
    expect(screen.getByText('A password stayed on this computer')).toBeInTheDocument();
  });

  it('shows a file arriving, then where it was saved', () => {
    const transfer = {
      device: 'p8',
      id: 1,
      file_name: '',
      done: 512,
      total: 1024,
      state: { kind: 'active' },
      dismissed: false,
    } as const;
    const { rerender } = renderScreen(
      <TransferToast device="Pixel 8" transfer={transfer} onDismiss={vi.fn()} />,
    );
    expect(screen.getByRole('progressbar')).toBeInTheDocument();
    rerender(
      <TransferToast
        device="Pixel 8"
        transfer={{ ...transfer, file_name: 'report.pdf', done: 1024, state: { kind: 'done' } }}
        onDismiss={vi.fn()}
      />,
    );
    expect(screen.getByText('report.pdf saved to Downloads')).toBeInTheDocument();
  });

  it('says neutrally that a file was not taken (declined, expired or switched off there)', () => {
    renderScreen(
      <TransferToast
        device="Pixel 8"
        transfer={{
          device: 'p8',
          id: 1,
          file_name: '',
          done: 0,
          total: 9,
          state: { kind: 'failed', reason: 'refused' },
          dismissed: false,
        }}
        onDismiss={vi.fn()}
      />,
    );
    expect(screen.getByText("Pixel 8 didn't take the file")).toBeInTheDocument();
  });
});
