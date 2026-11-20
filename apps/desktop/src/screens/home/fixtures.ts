import type { MoveRequestView } from '../../engine/types';

/** Desk PC asks to send its setup, 1:42 left (`sendreq:open`). */
export const deskPcRequest = (): MoveRequestView => ({
  id: 1,
  device: 'd2',
  name: 'Desk PC',
  summary: { apps: 12, bytes: 340_000_000, has_sensitive: true },
  expires_at: Math.floor(Date.now() / 1000) + 102,
});
