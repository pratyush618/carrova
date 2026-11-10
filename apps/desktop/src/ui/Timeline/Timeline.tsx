import type { ReactNode } from 'react';
import './Timeline.css';

export interface TimelineEntry {
  readonly id: string;
  readonly label: ReactNode;
  /** Machine-readable time (ISO 8601). */
  readonly dateTime: string;
  /** Formatted time to show. */
  readonly time: string;
}

/** What happened, in order. */
export function Timeline({ entries }: { readonly entries: readonly TimelineEntry[] }) {
  return (
    <ol className="p-timeline">
      {entries.map((e) => (
        <li key={e.id}>
          <span />
          <span>{e.label}</span>
          <time dateTime={e.dateTime}>{e.time}</time>
        </li>
      ))}
    </ol>
  );
}

/** Literal log lines (already redacted by the engine). Always left-to-right. */
export function Log({ lines }: { readonly lines: readonly string[] }) {
  return (
    <pre className="p-log" dir="ltr" tabIndex={0}>
      {lines.join('\n')}
    </pre>
  );
}
