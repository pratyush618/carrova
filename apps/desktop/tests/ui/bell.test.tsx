import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { Bell, type BellModel } from '@/ui';

const model = (patch: Partial<BellModel> = {}): BellModel => ({
  unread: 2,
  groups: [
    {
      title: 'Today',
      rows: [
        {
          id: 7,
          icon: 'receive',
          tone: 'accent',
          peer: "Ana's laptop",
          summary: 'wants to send you files.',
          when: '14:02',
          unread: true,
          review: true,
        },
      ],
    },
  ],
  onReview: vi.fn<(id: number) => void>(),
  onMarkAllRead: vi.fn<() => void>(),
  onSettings: vi.fn<() => void>(),
  ...patch,
});

function Page({ bell }: { readonly bell: BellModel }) {
  return (
    <div>
      <Bell model={bell} />
      <main>
        <button type="button">Current page</button>
      </main>
    </div>
  );
}

describe('Bell', () => {
  it('says how many are new', () => {
    render(<Page bell={model()} />);
    expect(screen.getByRole('button', { name: 'Notifications, 2 new' })).toBeVisible();
  });

  it('opens over the current page without navigating', async () => {
    const bell = model();
    render(<Page bell={bell} />);
    await userEvent.click(screen.getByRole('button', { name: /Notifications/ }));
    expect(screen.getByRole('region', { name: 'Notifications' })).toBeVisible();
    expect(screen.getByText("Ana's laptop")).toBeVisible();
    expect(screen.getByRole('button', { name: 'Current page' })).toBeVisible();
    expect(bell.onSettings).not.toHaveBeenCalled();
    expect(bell.onReview).not.toHaveBeenCalled();
  });

  it('closes on an outside click', async () => {
    render(<Page bell={model()} />);
    await userEvent.click(screen.getByRole('button', { name: /Notifications/ }));
    await userEvent.click(screen.getByRole('button', { name: 'Current page' }));
    expect(screen.queryByRole('region', { name: 'Notifications' })).not.toBeInTheDocument();
  });

  it('closes on Escape and gives focus back to the bell', async () => {
    render(<Page bell={model()} />);
    const button = screen.getByRole('button', { name: /Notifications/ });
    await userEvent.click(button);
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('region', { name: 'Notifications' })).not.toBeInTheDocument();
    expect(button).toHaveFocus();
  });

  it('closes on a second click', async () => {
    render(<Page bell={model()} />);
    const button = screen.getByRole('button', { name: /Notifications/ });
    await userEvent.click(button);
    await userEvent.click(button);
    expect(screen.queryByRole('region', { name: 'Notifications' })).not.toBeInTheDocument();
  });

  it('reviews a request and closes', async () => {
    const bell = model();
    render(<Page bell={bell} />);
    await userEvent.click(screen.getByRole('button', { name: /Notifications/ }));
    await userEvent.click(screen.getByRole('button', { name: 'Review' }));
    expect(bell.onReview).toHaveBeenCalledWith(7);
    expect(screen.queryByRole('region', { name: 'Notifications' })).not.toBeInTheDocument();
  });

  it('marks everything read, and has nothing to mark when all is read', async () => {
    const bell = model();
    const { rerender } = render(<Page bell={bell} />);
    await userEvent.click(screen.getByRole('button', { name: /Notifications/ }));
    await userEvent.click(screen.getByRole('button', { name: 'Mark all read' }));
    expect(bell.onMarkAllRead).toHaveBeenCalledOnce();
    rerender(<Page bell={{ ...bell, unread: 0 }} />);
    expect(screen.queryByRole('button', { name: 'Mark all read' })).not.toBeInTheDocument();
  });

  it('tells an empty bell what will show up', async () => {
    render(<Page bell={model({ unread: 0, groups: [] })} />);
    await userEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    expect(screen.getByText(/Nothing yet/)).toBeVisible();
  });
});
