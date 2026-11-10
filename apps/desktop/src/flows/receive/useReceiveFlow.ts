import { useCallback, useEffect, useMemo, useReducer, useRef } from 'react';
import { toBridgeError } from '../../engine/errors';
import type { DesktopHost } from '../../engine/host';
import type {
  ConflictRule,
  ItemKey,
  MoveDefaults,
  CarrovaPort,
  Remap,
  SessionId,
} from '../../engine/types';
import type { DecisionAnswer, ImportChoices } from '../../screens/receive/model';
import { current } from '../remote';
import { connectTarget } from './entry';
import {
  allowCrossOs,
  allowPlugin,
  setChoices,
  setItemRule,
  setPathRule,
  setRemap,
  type PlanOverride,
} from './planOptions';
import { initialReceiveFlow, receiveFlowReducer, type ReceiveFlowState } from './receiveFlow';

/** Commands of the receiving side (LAN and file); screens call these, never the port. */
export interface ReceiveController {
  readonly state: ReceiveFlowState;
  readonly start: (origin: 'lan' | 'file') => void;
  readonly setCode: (value: string) => void;
  readonly setAddress: (value: string) => void;
  readonly setPath: (value: string) => void;
  readonly setPassphrase: (value: string) => void;
  /** Present only when the host has a native "open" dialog. */
  readonly choosePath: (() => void) | null;
  /** Connect (LAN) or open the file, then plan with the suggested options. */
  readonly open: () => void;
  readonly answer: (key: ItemKey, answer: DecisionAnswer) => void;
  readonly setRule: (item: ItemKey, rule: ConflictRule) => void;
  readonly setFileRule: (path: string, rule: ConflictRule | null) => void;
  readonly setRemap: (item: ItemKey, remap: Remap | null) => void;
  readonly setChoices: (patch: Partial<ImportChoices>) => void;
  readonly allowCrossOs: (item: ItemKey) => void;
  readonly allowPlugin: (plugin: string) => void;
  /** Don't allow a plugin: the items it would change are left behind. */
  readonly leavePlugin: (plugin: string) => void;
  /** An override the user confirmed in its warning dialog. */
  readonly override: (override: PlanOverride, app: string | null) => void;
  readonly approve: () => void;
  /** Hold (`true`) or continue the running LAN transfer between batches. */
  readonly setPaused: (paused: boolean) => void;
  /** Continue a transfer paused for low space, ignoring the check (after the warning). */
  readonly continueDespiteSpace: () => void;
  /** Pick the session up again after a failure (what arrived is kept). */
  readonly retry: () => void;
  /** "Cancel, change nothing": ends the review as a dry run. */
  readonly endReview: () => void;
  /** Leave: stops pairing or a running transfer (between steps) and forgets the state. */
  readonly abandon: () => void;
  /** Continue an interrupted import from History. */
  readonly resume: (session: SessionId, origin: 'lan' | 'file') => void;
}

/** `defaults`: the user's suggested defaults (Settings); every new move starts from them. */
export function useReceiveFlow(
  port: CarrovaPort,
  host: DesktopHost,
  defaults: MoveDefaults,
): ReceiveController {
  const [state, dispatch] = useReducer(receiveFlowReducer, initialReceiveFlow);
  const ticket = useRef(0);
  const { session, options } = state;
  const applying = state.apply.status !== 'idle';
  const review = current(state.review);

  // The plan follows the options: every change the user makes is re-planned.
  useEffect(() => {
    if (session === null || applying) return;
    const mine = ++ticket.current;
    dispatch({ type: 'plan', action: { type: 'start' } });
    port.plan(session, options).then(
      (value) => {
        if (mine === ticket.current) dispatch({ type: 'plan', action: { type: 'ok', value } });
      },
      (reason: unknown) => {
        if (mine === ticket.current) {
          dispatch({ type: 'plan', action: { type: 'fail', error: toBridgeError(reason) } });
        }
      },
    );
  }, [port, session, options, applying]);

  const opened = useCallback((task: Promise<{ session: SessionId }>) => {
    const mine = ++ticket.current;
    dispatch({ type: 'opening' });
    task.then(
      (result) => {
        if (mine === ticket.current) dispatch({ type: 'opened', session: result.session });
      },
      (reason: unknown) => {
        if (mine === ticket.current) dispatch({ type: 'openFailed', error: toBridgeError(reason) });
      },
    );
  }, []);

  /** Pick session `id` up again and plan from the options it last planned with. */
  const resumed = useCallback(
    (id: SessionId) => {
      const mine = ++ticket.current;
      dispatch({ type: 'opening' });
      port
        .resume(id)
        .then(async ({ session: back }) => ({ back, stored: await port.planOptions(back) }))
        .then(
          ({ back, stored }) => {
            if (mine === ticket.current) {
              dispatch({ type: 'resumed', session: back, options: stored });
            }
          },
          (reason: unknown) => {
            if (mine === ticket.current) {
              dispatch({ type: 'openFailed', error: toBridgeError(reason) });
            }
          },
        );
    },
    [port],
  );

  const open = useCallback(() => {
    if (state.origin === 'lan') opened(port.connect(connectTarget(state.code)));
    else opened(port.openArchive(state.file.path.trim(), state.file.passphrase));
  }, [opened, port, state.origin, state.code, state.file.path, state.file.passphrase]);

  const { pickOpenPath } = host;
  const choosePath = useMemo(
    () =>
      pickOpenPath === undefined
        ? null
        : () => {
            void pickOpenPath().then((path) => {
              if (path !== null) dispatch({ type: 'path', value: path });
            });
          },
    [pickOpenPath],
  );

  const approve = useCallback(() => {
    if (session === null) return;
    const mine = ++ticket.current;
    dispatch({ type: 'apply', action: { type: 'start' } });
    // A resumed session's plan approved earlier with the same options needs no second approval.
    const approved = review?.approval === 'given' ? Promise.resolve() : port.approve(session);
    approved
      .then(() => port.apply(session))
      .then(
        (value) => {
          if (mine === ticket.current) dispatch({ type: 'apply', action: { type: 'ok', value } });
        },
        (reason: unknown) => {
          if (mine === ticket.current) {
            dispatch({ type: 'apply', action: { type: 'fail', error: toBridgeError(reason) } });
          }
        },
      );
  }, [port, session, review]);

  const connecting = state.code.status === 'connecting';
  /** Stops whatever runs: pairing (no session yet) or a transfer between steps. */
  const stop = useCallback(() => {
    ticket.current++;
    if (connecting) void port.cancelConnect();
    if (session !== null && state.apply.status === 'loading') void port.cancel(session);
  }, [port, connecting, session, state.apply.status]);

  const start = useCallback(
    (origin: 'lan' | 'file') => {
      ticket.current++;
      dispatch({ type: 'start', origin, defaults });
    },
    [defaults],
  );

  return useMemo(
    () => ({
      state,
      start,
      setCode: (value: string) => {
        dispatch({ type: 'code', value });
      },
      setAddress: (value: string) => {
        dispatch({ type: 'address', value });
      },
      setPath: (value: string) => {
        dispatch({ type: 'path', value });
      },
      setPassphrase: (value: string) => {
        dispatch({ type: 'passphrase', value });
      },
      choosePath,
      open,
      answer: (key: ItemKey, answer: DecisionAnswer) => {
        dispatch({ type: 'answer', key, answer });
      },
      setRule: (item: ItemKey, rule: ConflictRule) => {
        dispatch({ type: 'options', options: setItemRule(options, item, rule) });
      },
      setFileRule: (path: string, rule: ConflictRule | null) => {
        dispatch({ type: 'options', options: setPathRule(options, path, rule) });
      },
      setRemap: (item: ItemKey, remap: Remap | null) => {
        dispatch({ type: 'options', options: setRemap(options, item, remap) });
      },
      setChoices: (patch: Partial<ImportChoices>) => {
        dispatch({ type: 'options', options: setChoices(options, patch) });
      },
      allowCrossOs: (item: ItemKey) => {
        dispatch({ type: 'options', options: allowCrossOs(options, item) });
      },
      allowPlugin: (plugin: string) => {
        dispatch({ type: 'options', options: allowPlugin(options, plugin) });
      },
      leavePlugin: (plugin: string) => {
        for (const key of state.asked.plugins[plugin]?.items ?? []) {
          dispatch({ type: 'answer', key, answer: 'leave' });
        }
      },
      override: (override: PlanOverride, app: string | null) => {
        dispatch({ type: 'override', override, app });
      },
      approve,
      setPaused: (paused: boolean) => {
        if (session !== null) void port.setTransferPaused(session, paused);
      },
      continueDespiteSpace: () => {
        if (session !== null) void port.ignoreFreeSpace(session);
      },
      retry: () => {
        if (session !== null) resumed(session);
      },
      endReview: () => {
        if (session !== null) void port.endReview(session);
        start(state.origin);
      },
      abandon: () => {
        stop();
        start(state.origin);
      },
      resume: (id: SessionId, origin: 'lan' | 'file') => {
        start(origin);
        resumed(id);
      },
    }),
    [state, start, choosePath, open, options, approve, session, resumed, port, stop],
  );
}
