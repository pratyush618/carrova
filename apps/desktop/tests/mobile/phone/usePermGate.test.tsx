import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { PortProvider } from '@/engine/port';
import { createFakePort } from '@/ipc/fake';
import { usePermGate } from '@/mobile/phone/usePermGate';

describe('usePermGate', () => {
  it('asking again reads the permission again after a failed read', async () => {
    const port = createFakePort({ stepMs: 0 });
    const step = vi
      .spyOn(port, 'phonePermStep')
      .mockRejectedValueOnce(new Error('busy'))
      .mockResolvedValue('ask');
    const wrapper = ({ children }: { readonly children: ReactNode }) => (
      <PortProvider port={port}>{children}</PortProvider>
    );
    const hook = renderHook(() => usePermGate(), { wrapper });

    act(() => {
      hook.result.current.ask(['sms']);
    });
    await waitFor(() => {
      expect(step).toHaveBeenCalledTimes(1);
    });
    expect(hook.result.current.asking).toBeNull();

    act(() => {
      hook.result.current.ask(['sms']);
    });
    await waitFor(() => {
      expect(hook.result.current.asking?.perm).toBe('sms');
    });
  });
});
