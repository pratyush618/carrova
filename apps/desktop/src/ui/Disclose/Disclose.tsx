import type { ReactNode } from 'react';
import { Icon } from '../Icon/Icon';
import './Disclose.css';

export interface DiscloseProps {
  readonly summary: ReactNode;
  readonly children: ReactNode;
  readonly defaultOpen?: boolean;
}

/** Progressive disclosure for controls (never for consequences). Native details/summary. */
export function Disclose({ summary, children, defaultOpen = false }: DiscloseProps) {
  return (
    <details className="p-disclose" open={defaultOpen}>
      <summary>
        <Icon name="chev" size="sm" />
        {summary}
      </summary>
      <div className="p-disclose__body">{children}</div>
    </details>
  );
}
