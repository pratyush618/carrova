import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { renderApp } from '../support/app';

const REPORTS = 'Send error reports if Carrova crashes?';

describe('App', () => {
  it('opens on Home and asks once about error reports while consent was never asked', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp();
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' }),
    ).toBeInTheDocument();
    await user.click(await screen.findByRole('button', { name: 'Turn on' }));

    expect(await port.errorReportConsent()).toBe('allowed');
    expect(screen.queryByRole('heading', { name: REPORTS })).not.toBeInTheDocument();
  });

  it('starts the send flow from the S key', async () => {
    const user = userEvent.setup();
    await renderApp({ returning: true });
    await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
    await user.keyboard('s');
    expect(
      await screen.findByRole('heading', { level: 1, name: "Here's what we found" }),
    ).toBeInTheDocument();
    expect(screen.getByRole('list', { name: 'Step 1 of 4' })).toBeInTheDocument();
  });

  it('does not ask about error reports once answered', async () => {
    await renderApp({ returning: true });
    await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
    expect(screen.queryByRole('heading', { name: REPORTS })).not.toBeInTheDocument();
  });

  it('opens Customize as a modal sheet and closes it with Escape', async () => {
    const user = userEvent.setup();
    await renderApp({ returning: true });
    await user.click(await screen.findByRole('button', { name: /^Send/ }));
    await user.click(await screen.findByRole('button', { name: 'Customize' }));

    const sheet = screen.getByRole('dialog', { name: 'Customize what moves' });
    expect(sheet).toHaveAttribute('aria-modal', 'true');
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Customize' })).toHaveFocus();
  });
});
