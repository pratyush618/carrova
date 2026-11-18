import { screen } from '@testing-library/react';
import type { UserEvent } from '@testing-library/user-event';

/** From Home, open the linked computer's page, then the phone feature screen `name`. */
export async function openScreen(user: UserEvent, name: string) {
  await user.click(await screen.findByRole('button', { name: /Desk PC/ }));
  await user.click(await screen.findByRole('button', { name: new RegExp(`^${name}`) }));
  await screen.findByRole('heading', { level: 1, name });
}
