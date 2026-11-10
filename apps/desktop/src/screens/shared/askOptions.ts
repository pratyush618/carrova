import type { TFunction } from 'i18next';
import type { SegmentOption } from '../../ui';
import type { AskAnswer } from './model';

/** "Leave behind" | "Bring them": neither is worded as the bad choice. */
export function askOptions(t: TFunction): readonly SegmentOption<AskAnswer>[] {
  return [
    { value: 'leave', label: t('send.ask.leave') },
    { value: 'bring', label: t('send.ask.bring') },
  ];
}
