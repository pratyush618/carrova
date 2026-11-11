import { useCallback, useMemo, useReducer, useRef } from 'react';
import { useEngineDispatch } from '../../app/state/EngineState';
import { toBridgeError } from '../../engine/errors';
import type { DesktopHost } from '../../engine/host';
import type {
  BridgeError,
  CarrovaPort,
  ItemKey,
  ScanResult,
  SendReport,
  SessionId,
} from '../../engine/types';
import { current } from '../remote';
import { appsOnly, NO_PICKS, usable } from './modes';
import { toEdits, type SelectionChange } from './selectionChange';
import {
  initialSendFlow,
  sendFlowReducer,
  type ByMode,
  type SendFlowAction,
  type SendFlowState,
  type SendMode,
} from './sendFlow';

/** Commands of the sending side; screens call these, never the port. */
export interface SendController {
  readonly state: SendFlowState;
  /**
   * A fresh session (forgets any earlier one): Auto scans the apps; Manual
   * skips the scan and picks `paths` (e.g. sent from the file manager).
   */
  readonly start: (mode?: SendMode, paths?: readonly string[]) => void;
  /** Show the other mode; both keep their lists. The first Auto in a Manual session scans the apps. */
  readonly switchMode: (mode: SendMode) => void;
  /** The system pickers, starting in home; `null` when the host has none. */
  readonly addFiles: (() => void) | null;
  readonly addFolders: (() => void) | null;
  /** Pick absolute `paths` (switches to Manual); the engine measures them. */
  readonly addPaths: (paths: readonly string[]) => void;
  readonly removePick: (key: ItemKey) => void;
  readonly edit: (change: SelectionChange) => void;
  readonly resetToSuggested: () => void;
  /** Waits until the engine holds the visible mode's latest selection; `true` if it can be used. */
  readonly confirmSelection: () => Promise<boolean>;
  readonly send: () => void;
  /** Stop the current code and start pairing again with a new one. */
  readonly newCode: () => void;
  readonly answerPrompt: (offerNewCode: boolean) => void;
  /** Back to the selection (stops pairing). */
  readonly back: () => void;
  /** Leave the flow: stops whatever runs and forgets the state. */
  readonly abandon: () => void;
  /** Pick up an interrupted send from History. */
  readonly resume: (session: SessionId) => void;
}

/** One queued request: `mode`'s next reply from its last one (`null`: unchanged). */
type Request = (id: SessionId, base: ScanResult | null) => Promise<ScanResult | null>;

/**
 * The effect layer over {@link sendFlowReducer}: each command is one port call.
 * `includeCaches`: the user's suggested default (Settings) for a fresh scan.
 */
export function useSendFlow(
  port: CarrovaPort,
  includeCaches: boolean,
  host: DesktopHost = {},
): SendController {
  const [state, dispatch] = useReducer(sendFlowReducer, initialSendFlow);
  const engine = useEngineDispatch();
  const session = useRef<SessionId | null>(null);
  const mode = useRef<SendMode>('auto');
  /** Modes whose first reply was asked for in this session. */
  const opened = useRef<Record<SendMode, boolean>>({ auto: false, manual: false });
  /**
   * Each mode's reply, once every queued request has answered. Requests run
   * one after another, each on its mode's result before, so the engine
   * applies the user's edits and picks in order.
   */
  const latest = useRef<Promise<ByMode | null>>(Promise.resolve(null));
  /** Each mode's newest request: only its outcome is shown. */
  const newest = useRef<Record<SendMode, Promise<unknown> | null>>({ auto: null, manual: null });
  const ticket = useRef(0);
  const running = useRef(false);

  const stop = useCallback(() => {
    ticket.current++;
    if (running.current && session.current !== null) void port.cancel(session.current);
    running.current = false;
  }, [port]);

  /** Queue `request` for `on`; a failure shows as `failed(error)` and changes nothing. */
  const enqueue = useCallback(
    (
      on: SendMode,
      request: Request,
      failed: (error: BridgeError) => SendFlowAction = (error) => ({ type: 'selectFailed', error }),
    ) => {
      const id = session.current;
      if (id === null) return;
      const queued: Promise<ByMode | null> = latest.current.then(async (all) => {
        if (all === null) return null;
        try {
          const result = await request(id, all[on]);
          if (result === null) return all;
          if (newest.current[on] === queued) dispatch({ type: 'selected', mode: on, result });
          return { ...all, [on]: result };
        } catch (reason: unknown) {
          if (newest.current[on] === queued) dispatch(failed(toBridgeError(reason)));
          // Refused requests change nothing: the engine keeps the previous selection.
          return all;
        }
      });
      latest.current = queued;
      newest.current[on] = queued;
    },
    [],
  );

  const editIn = useCallback(
    (on: SendMode, change: SelectionChange) => {
      enqueue(on, (id, base) =>
        base === null
          ? Promise.resolve(null)
          : port.select(id, base.selection, toEdits(change, base.selection)),
      );
    },
    [port, enqueue],
  );

  const edit = useCallback(
    (change: SelectionChange) => {
      editIn(mode.current, change);
    },
    [editIn],
  );

  /** Show `on`, asking for its first reply when this session hasn't yet. */
  const switchMode = useCallback(
    (on: SendMode) => {
      if (on === mode.current) return;
      mode.current = on;
      dispatch({ type: 'mode', mode: on });
      if (opened.current[on]) return;
      opened.current[on] = true;
      if (on === 'manual') {
        // An Auto session's Manual list starts empty, in the engine's words.
        enqueue('manual', (id) => port.select(id, NO_PICKS, []));
        return;
      }
      // Manual sessions skip the app scan until Auto is first shown.
      dispatch({ type: 'scan', mode: 'auto', action: { type: 'start' } });
      enqueue(
        'auto',
        async (id) => {
          try {
            const scanned = appsOnly(await port.scanApps(id));
            const { selection } = scanned;
            return includeCaches
              ? await port.select(
                  id,
                  selection,
                  toEdits({ kind: 'caches', included: true }, selection),
                )
              : scanned;
          } catch (reason: unknown) {
            opened.current.auto = false;
            throw reason;
          }
        },
        (error) => ({ type: 'scan', mode: 'auto', action: { type: 'fail', error } }),
      );
    },
    [port, enqueue, includeCaches],
  );

  const addPaths = useCallback(
    (paths: readonly string[]) => {
      if (paths.length === 0 || session.current === null) return;
      switchMode('manual');
      dispatch({ type: 'measuring', paths });
      enqueue('manual', async (id, base) => {
        const reply =
          base === null
            ? null
            : await port.pick(id, base.selection, [...paths]).catch((reason: unknown) => {
                dispatch({ type: 'picked', paths, result: null, outcomes: [] });
                throw reason;
              });
        const result = reply?.scan ?? null;
        dispatch({ type: 'picked', paths, result, outcomes: reply?.outcomes ?? [] });
        return result;
      });
    },
    [port, enqueue, switchMode],
  );

  const removePick = useCallback(
    (key: ItemKey) => {
      enqueue('manual', (id, base) =>
        base === null
          ? Promise.resolve(null)
          : port.select(id, base.selection, [{ kind: 'remove_pick', item: key }]),
      );
    },
    [port, enqueue],
  );

  const home = current(state.scan)?.report.platform.roots.home ?? null;
  const { pickFiles, pickFolders } = host;
  /** A host picker that adds what the user chose; `null` when the host has none. */
  const picker = useCallback(
    (pick: DesktopHost['pickFiles']) =>
      pick === undefined
        ? null
        : () => {
            pick(home).then(
              (paths) => {
                if (paths !== null) addPaths(paths);
              },
              (reason: unknown) => {
                dispatch({ type: 'selectFailed', error: toBridgeError(reason) });
              },
            );
          },
    [home, addPaths],
  );
  const addFiles = useMemo(() => picker(pickFiles), [picker, pickFiles]);
  const addFolders = useMemo(() => picker(pickFolders), [picker, pickFolders]);

  const start = useCallback(
    (on: SendMode = 'auto', paths: readonly string[] = []) => {
      stop();
      const mine = ticket.current;
      session.current = null;
      mode.current = on;
      opened.current = { auto: on === 'auto', manual: on === 'manual' };
      dispatch({ type: 'reset', mode: on });
      dispatch({ type: 'scan', mode: on, action: { type: 'start' } });
      (on === 'auto' ? port.scan() : port.startManual()).then(
        (result) => {
          if (mine !== ticket.current) return;
          session.current = result.session;
          latest.current = Promise.resolve({ auto: null, manual: null, [on]: result });
          dispatch({ type: 'scan', mode: on, action: { type: 'ok', value: result } });
          // The user's suggested default (Settings): still one tap to undo.
          if (on === 'auto' && includeCaches) editIn('auto', { kind: 'caches', included: true });
          addPaths(paths);
        },
        (reason: unknown) => {
          if (mine === ticket.current) {
            dispatch({
              type: 'scan',
              mode: on,
              action: { type: 'fail', error: toBridgeError(reason) },
            });
          }
        },
      );
    },
    [port, stop, editIn, addPaths, includeCaches],
  );

  const resetToSuggested = useCallback(() => {
    enqueue('auto', async (id) => {
      const suggested = await port.suggest(id, 'suggested');
      return port.select(id, appsOnly(suggested).selection, []);
    });
  }, [port, enqueue]);

  const confirmSelection = useCallback(async () => {
    const on = mode.current;
    // With both modes open, the engine may hold the other one's selection: hand it this one.
    if (opened.current.auto && opened.current.manual) {
      enqueue(on, (id, base) =>
        base === null ? Promise.resolve(null) : port.select(id, base.selection, []),
      );
    }
    return usable((await latest.current)?.[on] ?? null);
  }, [port, enqueue]);

  /** Runs a send task; only the newest one's outcome lands. */
  const track = useCallback((task: Promise<SendReport>) => {
    const mine = ++ticket.current;
    running.current = true;
    dispatch({ type: 'run', action: { type: 'start' } });
    task.then(
      (report) => {
        if (mine !== ticket.current) return;
        running.current = false;
        dispatch({ type: 'run', action: { type: 'ok', value: report } });
      },
      (reason: unknown) => {
        if (mine !== ticket.current) return;
        running.current = false;
        dispatch({ type: 'run', action: { type: 'fail', error: toBridgeError(reason) } });
      },
    );
  }, []);

  const runSend = useCallback(
    (id: SessionId) => {
      track(port.runSend(id));
    },
    [port, track],
  );

  const send = useCallback(() => {
    void (async () => {
      const id = session.current;
      if (id !== null && (await confirmSelection())) runSend(id);
    })();
  }, [confirmSelection, runSend]);

  const newCode = useCallback(() => {
    const id = session.current;
    if (id === null) return;
    stop();
    runSend(id);
  }, [runSend, stop]);

  const answerPrompt = useCallback(
    (offerNewCode: boolean) => {
      const id = session.current;
      if (id === null) return;
      engine({ type: 'pairing_answered', session: id });
      void port.answerPairing(id, offerNewCode);
    },
    [engine, port],
  );

  const back = useCallback(() => {
    stop();
    dispatch({ type: 'back' });
  }, [stop]);

  const abandon = useCallback(() => {
    stop();
    dispatch({ type: 'reset' });
  }, [stop]);

  const resume = useCallback(
    (id: SessionId) => {
      stop();
      session.current = id;
      // The reducer shows a resumed send in Auto, with neither mode's list asked for yet.
      mode.current = 'auto';
      opened.current = { auto: false, manual: false };
      dispatch({ type: 'resumed', session: id });
      track(
        port.resume(id).then((opened) => {
          session.current = opened.session;
          return port.runSend(opened.session);
        }),
      );
    },
    [port, stop, track],
  );

  return useMemo(
    () => ({
      state,
      start,
      switchMode,
      addFiles,
      addFolders,
      addPaths,
      removePick,
      edit,
      resetToSuggested,
      confirmSelection,
      send,
      newCode,
      answerPrompt,
      back,
      abandon,
      resume,
    }),
    [
      state,
      start,
      switchMode,
      addFiles,
      addFolders,
      addPaths,
      removePick,
      edit,
      resetToSuggested,
      confirmSelection,
      send,
      newCode,
      answerPrompt,
      back,
      abandon,
      resume,
    ],
  );
}
