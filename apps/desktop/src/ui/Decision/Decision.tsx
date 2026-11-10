import type { ReactNode } from 'react';
import { AppGlyph } from '../AppGlyph/AppGlyph';
import type { IconName } from '../Icon/icons';
import { Segmented, type SegmentOption } from '../Segmented/Segmented';
import './Decision.css';

interface DecisionBase {
  readonly icon: IconName;
  /** The question, in plain words ("Your SSH keys"). */
  readonly title: string;
  /** The consequence, literally ("Anyone with a copy can log in as you"). */
  readonly why: ReactNode;
  /** About keys or secrets: the tile reads amber until answered. */
  readonly sensitive?: boolean;
}

/** Answered in place with a segmented choice. */
interface Choice<T extends string> extends DecisionBase {
  readonly options: readonly SegmentOption<T>[];
  /** `null` until the user answers; approve stays disabled meanwhile. */
  readonly value: T | null;
  readonly onChange: (value: T) => void;
  /** The answer is a risk (bring keys): it reads amber. */
  readonly warn?: boolean;
}

/** Answered elsewhere (a dialog or sheet) through its own buttons. */
interface Action extends DecisionBase {
  readonly action: ReactNode;
  readonly answered: boolean;
}

export type DecisionProps<T extends string> = Choice<T> | Action;

/** A sensitive or blocking question that needs an explicit answer. Its tile turns green once answered. */
export function Decision<T extends string>(props: DecisionProps<T>) {
  const { icon, title, why, sensitive = false } = props;
  const answered = 'action' in props ? props.answered : props.value !== null;
  return (
    <div className="p-decision">
      <AppGlyph glyph={{ icon }} tone={answered ? 'success' : sensitive ? 'warning' : 'neutral'} />
      <p className="p-decision__title">{title}</p>
      <p className="p-decision__why">{why}</p>
      <div className="p-decision__act">
        {'action' in props ? (
          props.action
        ) : (
          <Segmented
            label={title}
            options={props.options}
            value={props.value}
            onChange={props.onChange}
            warn={props.warn ?? false}
          />
        )}
      </div>
    </div>
  );
}
