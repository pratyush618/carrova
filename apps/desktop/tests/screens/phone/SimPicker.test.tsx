import { screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderScreen } from '../../support/render';
import type { SimChoice } from '@/bindings/SimChoice';
import { SimPicker } from '@/screens/phone/SimPicker';

const sim = (slot: number, name: string): SimChoice => ({
  id: slot,
  slot,
  name,
  default: slot === 1,
});

describe('SimPicker labels', () => {
  it('names each SIM by its slot and operator', () => {
    renderScreen(
      <SimPicker
        label="Send from"
        sims={[sim(1, 'Jio'), sim(2, 'airtel')]}
        value={1}
        onChange={() => undefined}
      />,
    );
    expect(screen.getByRole('radio', { name: 'SIM 1 · Jio' })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: 'SIM 2 · airtel' })).toBeInTheDocument();
  });

  it('shows only the slot when the phone gives no name', () => {
    renderScreen(
      <SimPicker
        label="Send from"
        sims={[sim(1, ''), sim(2, 'airtel')]}
        value={1}
        onChange={() => undefined}
      />,
    );
    expect(screen.getByRole('radio', { name: 'SIM 1' })).toBeInTheDocument();
  });
});
