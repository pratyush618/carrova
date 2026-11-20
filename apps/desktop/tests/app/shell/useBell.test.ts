import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useBell } from '@/app/shell/useBell';
import type { MoveRequestView } from '@/engine/types';

const request = (id: number): MoveRequestView => ({
  id,
  device: `d${String(id)}`,
  name: 'Desk PC',
  summary: { apps: 12, bytes: 340_000_000, has_sensitive: true },
  expires_at: 0,
});

const controllers = {
  inbox: { notices: [], markRead: vi.fn() },
  receive: { start: vi.fn(), reviewRequested: vi.fn() },
  requests: {
    pending: [] as MoveRequestView[],
    review: vi.fn<(id: number) => Promise<string | null>>(() => Promise.resolve('import-1')),
  },
};

vi.mock('@/app/controllers/Controllers', () => ({ useControllers: () => controllers }));

beforeEach(() => {
  controllers.requests.pending = [];
  vi.clearAllMocks();
});

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

  it('starts a receive by code when no move request waits', () => {
    const go = vi.fn();
    const { result } = renderHook(() => useBell(go));
    result.current.onReview(3);
    expect(controllers.receive.start).toHaveBeenCalledWith('lan');
    expect(go).toHaveBeenCalledWith({ type: 'startFlow', flow: 'receive' });
  });

  it('reviews the newest waiting move request', async () => {
    controllers.requests.pending = [request(1), request(2)];
    const go = vi.fn();
    const { result } = renderHook(() => useBell(go));
    result.current.onReview(3);
    expect(controllers.requests.review).toHaveBeenCalledWith(2);
    await waitFor(() => {
      expect(go).toHaveBeenCalledWith({
        type: 'enterFlow',
        at: { flow: 'receive', step: 'review' },
      });
    });
    expect(controllers.receive.reviewRequested).toHaveBeenCalledWith('import-1');
    expect(controllers.receive.start).not.toHaveBeenCalled();
  });
});
