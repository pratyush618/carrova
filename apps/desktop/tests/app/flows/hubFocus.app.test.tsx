import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { renderApp } from '../../support/app';

describe('hub navigation', () => {
  it.each(['History', 'Apps', 'Settings'])('moves focus to the %s page', async (label) => {
    const user = userEvent.setup();
    await renderApp({ returning: true });
    await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
    await user.click(screen.getByRole('button', { name: label }));
    expect(await screen.findByRole('main', { name: label })).toHaveFocus();
  });

  it("keeps a page's own heading as the focus target (Home)", async () => {
    const user = userEvent.setup();
    await renderApp({ returning: true });
    await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
    await user.click(screen.getByRole('button', { name: 'History' }));
    await user.click(screen.getByRole('button', { name: 'Home' }));
    expect(await screen.findByRole('heading', { level: 1 })).toHaveFocus();
  });
});
