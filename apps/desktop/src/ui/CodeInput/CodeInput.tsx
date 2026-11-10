import type { KeyboardEvent } from 'react';
import type { FieldControl } from '../Field/Field';
import { acceptSuggestion, suggestWord } from './complete';
import '../Field/Field.css';
import './CodeInput.css';

export interface CodeInputProps {
  readonly control: FieldControl;
  readonly value: string;
  /** Pairing words to complete from (the engine's word list). */
  readonly words: readonly string[];
  readonly onChange: (value: string) => void;
  readonly onSubmit: () => void;
}

/**
 * The pairing-code input. Shows the rest of the word as a ghost after three
 * letters; Tab or → accepts it, Enter connects. Always left-to-right.
 */
export function CodeInput({ control, value, words, onChange, onSubmit }: CodeInputProps) {
  const word = suggestWord(value, words);
  const rest = word === null ? '' : word.slice(value.split(/[-\s]/).pop()?.length ?? 0);

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') {
      onSubmit();
      return;
    }
    if (event.key !== 'Tab' && event.key !== 'ArrowRight') return;
    const accepted = acceptSuggestion(value, words);
    if (accepted === null) return;
    event.preventDefault();
    onChange(accepted);
  };

  return (
    <div className="p-ghost">
      <input
        {...control}
        className="p-input p-input--code"
        dir="ltr"
        autoComplete="off"
        spellCheck={false}
        value={value}
        onChange={(event) => {
          onChange(event.target.value);
        }}
        onKeyDown={onKeyDown}
      />
      <span className="p-ghost__hint" aria-hidden="true" dir="ltr">
        <span className="p-ghost__typed">{value}</span>
        {rest}
      </span>
    </div>
  );
}
