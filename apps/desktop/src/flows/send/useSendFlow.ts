import { useCallback, useMemo, useReducer, useRef } from 'react';
import { useEngineDispatch } from '../../app/state/EngineState';
import { toBridgeError } from '../../engine/errors';
import type { DesktopHost } from '../../engine/host';
import type { CarrovaPort, ItemKey, SendReport, SessionId } from '../../engine/types';
import { current } from '../remote';
import { appsOnly, NO_PICKS, usable } from './modes';
import { toEdits, type SelectionChange } from './selectionChange';
import {
  enqueue as enqueueOn,
  queueFrom,
  type Outlet,
  type Request,
  type SelectionQueue,
} from './selectionQueue';
import { initialSendFlow, sendFlowReducer, type SendFlowState, type SendMode } from './sendFlow';

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
  /** After a failed scan: scan the apps again in this session (picks kept), or repeat the failed start. */
  readonly retryScan: () => void;
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
  const queue = useRef<SelectionQueue>(queueFrom(null));
  const ticket = useRef(0);
  /** The latest start's mode and paths, for retrying it when it failed. */
  const started = useRef<[SendMode, readonly string[]]>(['auto', []]);
  /** Bumped by every stop: queued replies from before it are dropped. */
  const generation = useRef(0);
  const running = useRef(false);

  const stop = useCallback(() => {
    ticket.current++;
    generation.current++;
    if (running.current && session.current !== null) void port.cancel(session.current);
    running.current = false;
  }, [port]);

  /** Queue `request` for `on` in this session; `true` once it succeeded. */
  const enqueue = useCallback(
    (
      on: SendMode,
      request: Request,
      failed: Outlet['failed'] = (error) => ({ type: 'selectFailed', error }),
    ): Promise<boolean> => {
      const id = session.current;
      if (id === null) return Promise.resolve(false);
      const mine = generation.current;
      const live = () => mine === generation.current;
      return enqueueOn(queue.current, on, id, request, { dispatch, live, failed });
    },
    [],
  );

  const editIn = useCallback(
    (on: SendMode, change: SelectionChange) => {
      void enqueue(on, (id, base) =>
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

  /** Scan the apps into Auto; a failure stays shown there (the reducer keeps it), with its retry. */
  const scanApps = useCallback(() => {
    dispatch({ type: 'scan', mode: 'auto', action: { type: 'start' } });
    void enqueue(
      'auto',
      async (id) => {
        const scanned = appsOnly(await port.scanApps(id));
        const { selection } = scanned;
        return includeCaches
          ? port.select(id, selection, toEdits({ kind: 'caches', included: true }, selection))
          : scanned;
      },
      (error) => ({ type: 'scan', mode: 'auto', action: { type: 'fail', error } }),
    );
  }, [port, enqueue, includeCaches]);

  /** Show `on`, asking for its first reply when this session hasn't yet. */
  const switchMode = useCallback(
    (on: SendMode) => {
      if (on === mode.current) return;
      mode.current = on;
      dispatch({ type: 'mode', mode: on });
      if (opened.current[on]) return;
      opened.current[on] = true;
      if (on === 'manual') {
        const generationNow = generation.current;
        // An Auto session's Manual list starts empty, in the engine's words.
        void enqueue('manual', (id) =>
          port.select(id, NO_PICKS, []).catch((reason: unknown) => {
            // Ask again on the next switch: later picks need this base.
            if (generationNow === generation.current) opened.current.manual = false;
            throw reason;
          }),
        );
        return;
      }
      // Manual sessions skip the app scan until Auto is first shown.
      scanApps();
    },
    [port, enqueue, scanApps],
  );

  const addPaths = useCallback(
    (paths: readonly string[]) => {
      if (paths.length === 0 || session.current === null) return;
      switchMode('manual');
      dispatch({ type: 'measuring', paths });
      void enqueue('manual', async (id, base, show) => {
        const reply =
          base === null
            ? null
            : await port.pick(id, base.selection, [...paths]).catch((reason: unknown) => {
                show({ type: 'picked', paths, result: null, outcomes: [] });
                throw reason;
              });
        const result = reply?.scan ?? null;
        show({ type: 'picked', paths, result, outcomes: reply?.outcomes ?? [] });
        return result;
      });
    },
    [port, enqueue, switchMode],
  );

  const removePick = useCallback(
    (key: ItemKey) => {
      void enqueue('manual', (id, base) =>
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
      started.current = [on, paths];
      session.current = null;
      mode.current = on;
      opened.current = { auto: on === 'auto', manual: on === 'manual' };
      dispatch({ type: 'reset', mode: on });
      dispatch({ type: 'scan', mode: on, action: { type: 'start' } });
      (on === 'auto' ? port.scan() : port.startManual()).then(
        (result) => {
          if (mine !== ticket.current) return;
          session.current = result.session;
          queue.current = queueFrom({ auto: null, manual: null, [on]: result });
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

  const retryScan = useCallback(() => {
    if (session.current === null) start(...started.current);
    else scanApps();
  }, [start, scanApps]);

  const resetToSuggested = useCallback(() => {
    void enqueue('auto', async (id) => {
      const suggested = await port.suggest(id, 'suggested');
      return port.select(id, appsOnly(suggested).selection, []);
    });
  }, [port, enqueue]);

  const confirmSelection = useCallback(async () => {
    const on = mode.current;
    // With both modes open, the engine may hold the other one's selection: hand it this one.
    const handed =
      !(opened.current.auto && opened.current.manual) ||
      enqueue(on, (id, base) =>
        base === null ? Promise.resolve(null) : port.select(id, base.selection, []),
      );
    const result = (await queue.current.latest)?.[on] ?? null;
    return (await handed) && usable(result);
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
      const mine = generation.current;
      if (id === null || !(await confirmSelection())) return;
      // The user may have started over while the selection was handed over.
      if (id === session.current && mine === generation.current) runSend(id);
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
      retryScan,
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
      retryScan,
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
