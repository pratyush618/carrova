import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { renderApp } from '../support/app';

describe('App', () => {
  it('opens on the welcome screen while error-report consent was never asked', async () => {
    await renderApp();
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Bring your setup to a new computer.' }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('switch', { name: 'Send error reports if Carrova crashes' }),
    ).not.toBeChecked();
  });

  it('records the consent answer and walks from welcome to Home and into the send flow', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp();
    await user.click(
      await screen.findByRole('switch', { name: 'Send error reports if Carrova crashes' }),
    );
    await user.click(screen.getByRole('button', { name: 'Get started' }));
    expect(
      screen.getByRole('heading', { level: 1, name: 'What are we doing on this computer?' }),
    ).toHaveFocus();
    expect(await port.errorReportConsent()).toBe('allowed');

    await user.click(screen.getByRole('button', { name: /Send/ }));
    expect(
      await screen.findByRole('heading', { level: 1, name: "Here's what we found" }),
    ).toBeInTheDocument();
    expect(screen.getByRole('img', { name: 'Step 1 of 4' })).toBeInTheDocument();
  });

  it('opens on Home for a returning user', async () => {
    await renderApp({ returning: true });
    expect(
      await screen.findByRole('heading', { level: 1, name: 'What are we doing on this computer?' }),
    ).toBeInTheDocument();
  });

  it('opens Customize as a modal sheet and closes it with Escape', async () => {
    const user = userEvent.setup();
    await renderApp({ returning: true });
    await user.click(await screen.findByRole('button', { name: /Send/ }));
    await user.click(await screen.findByRole('button', { name: 'Customize' }));

    const sheet = screen.getByRole('dialog', { name: 'Customize what moves' });
    expect(sheet).toHaveAttribute('aria-modal', 'true');
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Customize' })).toHaveFocus();
  });
});
