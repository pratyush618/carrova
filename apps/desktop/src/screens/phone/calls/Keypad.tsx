import { useTranslation } from 'react-i18next';
import { Button } from '../../../ui';
import './calls.css';

const KEYS = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '*', '0', '#'] as const;

/** The dial pad of a call: each key sends its tone. */
export function Keypad({ onTone }: { readonly onTone: (digit: string) => void }) {
  const { t } = useTranslation();
  return (
    <div className="s-call__keypad" role="group" aria-label={t('calls.window.keypadLabel')}>
      {KEYS.map((key) => (
        <Button
          key={key}
          size="sm"
          onClick={() => {
            onTone(key);
          }}
        >
          {key}
        </Button>
      ))}
    </div>
  );
}
