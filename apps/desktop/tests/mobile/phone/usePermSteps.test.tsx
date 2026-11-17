import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import type { PermStep } from '@/bindings/PermStep';
import { PortProvider } from '@/engine/port';
import { createFakePort } from '@/ipc/fake';
import { usePermSteps } from '@/mobile/phone/usePermSteps';

const SMS = ['sms'] as const;

describe('usePermSteps', () => {
  it('a read older than a newer one is dropped', async () => {
    const port = createFakePort({ stepMs: 0 });
    let finish = (_: PermStep) => undefined as void;
    vi.spyOn(port, 'phonePermStep')
      .mockReturnValueOnce(new Promise((resolve) => (finish = resolve)))
      .mockResolvedValue('ok');
    const wrapper = ({ children }: { readonly children: ReactNode }) => (
      <PortProvider port={port}>{children}</PortProvider>
    );
    const hook = renderHook(() => usePermSteps(SMS), { wrapper });

    // Back from settings, where access was granted.
    document.dispatchEvent(new Event('visibilitychange'));
    await waitFor(() => {
      expect(hook.result.current).toEqual(['ok']);
    });
    await act(async () => {
      finish('open_settings');
      await Promise.resolve();
    });

    expect(hook.result.current).toEqual(['ok']);
  });
});
