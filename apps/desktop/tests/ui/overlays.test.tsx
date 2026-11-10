import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { Button, Dialog, Sheet, Window } from '@/ui';

function WithSheet({ onClose }: { readonly onClose: () => void }) {
  const [open, setOpen] = useState(false);
  const close = () => {
    onClose();
    setOpen(false);
  };
  return (
    <Window
      overlays={
        open
          ? [
              <Sheet
                key="s"
                title="Every change"
                onClose={close}
                foot={<Button onClick={close}>Done</Button>}
              >
                <Button>Replace</Button>
              </Sheet>,
            ]
          : []
      }
    >
      <Button
        onClick={() => {
          setOpen(true);
        }}
      >
        See every change
      </Button>
    </Window>
  );
}

describe('Sheet', () => {
  it('takes focus, keeps Tab inside, closes on Escape and gives focus back', async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<WithSheet onClose={onClose} />);
    const opener = screen.getByRole('button', { name: 'See every change' });
    await user.click(opener);

    const sheet = screen.getByRole('dialog', { name: 'Every change' });
    expect(screen.getByRole('heading', { name: 'Every change' })).toHaveFocus();
    // Close (header), Replace, Done — then back to Close.
    await user.tab();
    await user.tab();
    await user.tab();
    expect(screen.getByRole('button', { name: 'Done' })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole('button', { name: 'Close' })).toHaveFocus();
    expect(sheet).toContainElement(
      document.activeElement instanceof HTMLElement ? document.activeElement : null,
    );

    await user.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalledOnce();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(opener).toHaveFocus();
  });

  it('makes the screen underneath inert', async () => {
    render(<WithSheet onClose={vi.fn()} />);
    await userEvent.click(screen.getByRole('button', { name: 'See every change' }));
    expect(
      screen.getByRole('button', { name: 'See every change', hidden: true }).closest('[inert]'),
    ).not.toBeNull();
  });
});

describe('Dialog', () => {
  it('focuses the action marked as the safe default', () => {
    render(
      <Dialog
        tone="warning"
        icon="warn"
        title="Ignore the free-space check?"
        onDismiss={vi.fn()}
        foot={
          <>
            <Button kind="primary" data-initial-focus>
              Keep it safe
            </Button>
            <Button kind="danger">Ignore and continue</Button>
          </>
        }
      >
        <p>Carrova checks space so a move can’t fill your disk.</p>
      </Dialog>,
    );
    expect(
      screen.getByRole('alertdialog', { name: 'Ignore the free-space check?' }),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Keep it safe' })).toHaveFocus();
  });
});
