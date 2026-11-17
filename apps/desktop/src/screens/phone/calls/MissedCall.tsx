import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { SimChoice } from '../../../bindings/SimChoice';
import { presetSim } from '../../../flows/phone/usePhoneSims';
import { Button } from '../../../ui';
import { initials } from '../messages/ThreadList';
import { SimPicker } from '../SimPicker';
import './calls.css';

/** A call this computer rang for and nobody answered: call back from the phone (from the SIM picked), or close. */
export function MissedCall({
  who,
  name,
  phone,
  sims,
  on,
}: {
  readonly who: string;
  /** The caller's contact name (`null`: unknown), for the initials. */
  readonly name: string | null;
  readonly phone: string;
  /** The SIMs to call back from (none with one SIM: the phone's default). */
  readonly sims: readonly SimChoice[];
  readonly on: {
    readonly onCallBack: (sim: number | null) => void;
    readonly onClose: () => void;
  };
}) {
  const { t } = useTranslation();
  const [picked, setPicked] = useState<number | null>(null);
  // The missed call doesn't say which SIM it came in on: the phone's default leads.
  const sim = presetSim(sims, picked);
  return (
    <main className="s-call">
      <span className="s-call__av" aria-hidden="true">
        {initials(name)}
      </span>
      <h1 className="s-call__who">{t('calls.window.missedFrom', { who })}</h1>
      <p className="s-call__sub">{phone}</p>
      <SimPicker label={t('calls.log.callFrom')} sims={sims} value={sim} onChange={setPicked} />
      <div className="s-call__acts">
        <Button onClick={on.onClose}>{t('calls.window.close')}</Button>
        <Button
          kind="primary"
          icon="call"
          onClick={() => {
            on.onCallBack(sim);
          }}
        >
          {t('calls.window.callBack')}
        </Button>
      </div>
    </main>
  );
}
