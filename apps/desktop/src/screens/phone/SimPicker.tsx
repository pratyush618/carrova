import { useTranslation } from 'react-i18next';
import type { SimChoice } from '../../bindings/SimChoice';
import { Segmented } from '../../ui';

/** Which SIM a text or call goes from, as Android names them ("SIM 1 · Jio"); nothing with one SIM. */
export function SimPicker({
  label,
  sims,
  value,
  onChange,
}: {
  readonly label: string;
  readonly sims: readonly SimChoice[];
  readonly value: number | null;
  readonly onChange: (sim: number) => void;
}) {
  const { t } = useTranslation();
  if (sims.length === 0) return null;
  return (
    <Segmented
      label={label}
      options={sims.map((s) => ({
        value: String(s.id),
        label:
          s.name === ''
            ? t('ecosystem.simSlot', { slot: s.slot })
            : t('ecosystem.sim', { slot: s.slot, name: s.name }),
      }))}
      value={value === null ? null : String(value)}
      onChange={(v) => {
        onChange(Number(v));
      }}
    />
  );
}
