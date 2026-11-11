import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { renderApp } from '../../support/app';

describe('hub pages', () => {
  it.each(['History', 'Apps', 'Settings'])(
    '%s renders no heading that repeats its sidebar label',
    async (label) => {
      const user = userEvent.setup();
      await renderApp({ returning: true });
      await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
      await user.click(screen.getByRole('button', { name: label }));
      await screen.findByRole('button', { name: label, current: 'page' });
      expect(screen.queryByRole('heading', { name: label })).not.toBeInTheDocument();
    },
  );
});
