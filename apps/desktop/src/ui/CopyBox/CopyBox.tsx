import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '../Button/Button';
import './CopyBox.css';

export interface CopyBoxProps {
  /** An address, path or command; shown left-to-right. */
  readonly text: string;
  /** Overrides the clipboard (tests, or a platform clipboard via IPC). */
  readonly onCopy?: (text: string) => Promise<void>;
}

const clipboardWrite = (text: string): Promise<void> => navigator.clipboard.writeText(text);

/** A literal to copy, with one Copy button that confirms in place. */
export function CopyBox({ text, onCopy = clipboardWrite }: CopyBoxProps) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  return (
    <div className="p-copybox">
      <code dir="ltr">{text}</code>
      <Button
        size="sm"
        icon={copied ? 'check' : 'copy'}
        onClick={() => {
          onCopy(text).then(
            () => {
              setCopied(true);
            },
            () => {
              setCopied(false);
            },
          );
        }}
      >
        {copied ? t('ui.copied') : t('ui.copy')}
      </Button>
    </div>
  );
}
