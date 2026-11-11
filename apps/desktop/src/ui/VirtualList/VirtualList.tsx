import {
  Fragment,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from 'react';
import './VirtualList.css';

export interface VirtualListProps<T> {
  readonly items: readonly T[];
  /** Every row is exactly this tall, in px (the rows' own CSS must agree). */
  readonly rowHeight: number;
  /** One row: a list item, no key needed. */
  readonly render: (item: T, index: number) => ReactNode;
  /** Rows kept mounted beyond each edge of the view. */
  readonly overscan?: number;
  /** The list's accessible name. */
  readonly label?: string;
}

/**
 * A long list that mounts only the rows in view (plus `overscan` each side), in
 * a scroll box of its own. shortcut: fixed row height and the viewport is read
 * on scroll, not on resize; upgrade when rows must vary or the window resizes mid-list.
 */
export function VirtualList<T>({
  items,
  rowHeight,
  render,
  overscan = 8,
  label,
}: VirtualListProps<T>) {
  const scroller = useRef<HTMLDivElement>(null);
  const [view, setView] = useState({ top: 0, height: 0 });
  const measure = (el: HTMLElement) => {
    setView({ top: el.scrollTop, height: el.clientHeight });
  };
  useLayoutEffect(() => {
    if (scroller.current) measure(scroller.current);
  }, []);

  const start = Math.max(0, Math.floor(view.top / rowHeight) - overscan);
  const end = Math.min(items.length, Math.ceil((view.top + view.height) / rowHeight) + overscan);
  return (
    <div
      ref={scroller}
      className="p-vlist p-list--card"
      onScroll={(event) => {
        measure(event.currentTarget);
      }}
    >
      <div className="p-vlist__space" style={{ blockSize: items.length * rowHeight }}>
        <ul
          className="p-list p-vlist__rows"
          aria-label={label}
          style={
            {
              insetBlockStart: start * rowHeight,
              '--p-vlist-row': `${String(rowHeight)}px`,
            } as CSSProperties
          }
        >
          {items.slice(start, end).map((item, i) => (
            <Fragment key={start + i}>{render(item, start + i)}</Fragment>
          ))}
        </ul>
      </div>
    </div>
  );
}
