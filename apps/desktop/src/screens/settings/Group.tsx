import type { ReactNode } from 'react';
import { List } from '../../ui';

/** One titled group of Settings rows, with an optional line under the title. */
export function Group({
  title,
  sub,
  children,
}: {
  readonly title: string;
  readonly sub?: string;
  readonly children: ReactNode;
}) {
  return (
    <section className="s-settings-group">
      <h2>{title}</h2>
      {sub !== undefined && <p className="s-settings-sub">{sub}</p>}
      <List>{children}</List>
    </section>
  );
}
