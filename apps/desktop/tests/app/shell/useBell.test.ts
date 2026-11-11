import { renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useBell } from '@/app/shell/useBell';

vi.mock('@/app/controllers/Controllers', () => ({
  useControllers: () => ({
    inbox: { notices: [], markRead: vi.fn() },
    receive: { start: vi.fn() },
  }),
}));

describe('useBell', () => {
  it('opens the Notifications settings from the bell', () => {
    const go = vi.fn();
    const { result } = renderHook(() => useBell(go));
    result.current.onSettings();
    expect(go).toHaveBeenCalledWith({
      type: 'openHub',
      page: 'settings',
      section: 'notifications',
    });
  });
});
