import { useEffect, useState } from 'react';
import type { RollbackPreview, SessionId } from '../../engine/types';

/**
 * What undoing `session` would do now, asked once per session through
 * `peek`; `null` while there is nothing to undo, the engine hasn't answered,
 * or it failed.
 */
export function useUndoPreview(
  peek: (session: SessionId) => Promise<RollbackPreview>,
  session: SessionId | null,
): RollbackPreview | null {
  const [seen, setSeen] = useState<{ session: SessionId; preview: RollbackPreview } | null>(null);
  // shortcut: counts aren't refreshed while the rail stays open on one move, re-peek on file-change events if users act on stale counts.
  useEffect(() => {
    if (session === null) return;
    let live = true;
    void peek(session).then(
      (preview) => {
        if (live) setSeen({ session, preview });
      },
      // No preview: the rail leaves the counts out; the undo dialog asks again.
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [peek, session]);
  // Never show the last session's counts under another one.
  return seen?.session === session ? seen.preview : null;
}
