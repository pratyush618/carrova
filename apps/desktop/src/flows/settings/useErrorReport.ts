import { useCallback, useMemo, useState } from 'react';
import type { DesktopHost } from '../../engine/host';
import type { ErrorReport, IssueDraft, CarrovaPort } from '../../engine/types';
import type { CrashModel } from '../../screens/settings/model';

/** The report exactly as stored: version, system, kind, then each redacted line. */
export function reportLines(report: ErrorReport): string[] {
  return [
    `carrova ${report.app_version}, ${report.os}`,
    report.kind,
    ...report.message_chain,
    ...report.backtrace_frames,
  ];
}

export interface ErrorReportController {
  readonly model: CrashModel | null;
  readonly open: (id: string) => void;
  /** First press prepares the issue text; second press opens it in the browser. */
  readonly send: () => void;
  readonly remove: () => void;
  readonly close: () => void;
}

/** Viewing one report and turning it into a GitHub issue the user reviews and sends. */
export function useErrorReport(
  port: CarrovaPort,
  host: DesktopHost,
  onRemoved: () => void,
): ErrorReportController {
  const [id, setId] = useState<string | null>(null);
  const [report, setReport] = useState<ErrorReport | null>(null);
  const [draft, setDraft] = useState<IssueDraft | null>(null);
  const { openUrl } = host;

  const open = useCallback(
    (next: string) => {
      setId(next);
      setReport(null);
      setDraft(null);
      void port.errorReport(next).then(setReport);
    },
    [port],
  );

  const send = useCallback(() => {
    if (id === null) return;
    if (draft === null) void port.errorReportIssue(id).then(setDraft);
    else if (openUrl) void openUrl(draft.url);
  }, [port, id, draft, openUrl]);

  const close = useCallback(() => {
    setId(null);
    setReport(null);
    setDraft(null);
  }, []);

  const remove = useCallback(() => {
    if (id === null) return;
    void port.deleteErrorReport(id).then(() => {
      close();
      onRemoved();
    });
  }, [port, id, close, onRemoved]);

  const model = useMemo(
    () =>
      report === null
        ? null
        : { report: reportLines(report), draft: draft && { title: draft.title, body: draft.body } },
    [report, draft],
  );
  return useMemo(() => ({ model, open, send, remove, close }), [model, open, send, remove, close]);
}
