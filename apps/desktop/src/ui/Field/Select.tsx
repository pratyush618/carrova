import { Icon } from '../Icon/Icon';
import './Field.css';

export interface SelectOption<T extends string> {
  readonly value: T;
  readonly label: string;
}

export interface SelectProps<T extends string> {
  /** Accessible name (the visible row title usually says the same). */
  readonly label: string;
  readonly options: readonly SelectOption<T>[];
  readonly value: T;
  readonly onChange: (value: T) => void;
}

/** A native select over a closed set of typed values. */
export function Select<T extends string>({ label, options, value, onChange }: SelectProps<T>) {
  return (
    <select
      className="p-input p-select"
      aria-label={label}
      value={value}
      onChange={(event) => {
        const picked = options.find((o) => o.value === event.target.value);
        if (picked) onChange(picked.value);
      }}
    >
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );
}

export interface SearchProps {
  readonly label: string;
  readonly placeholder: string;
  readonly value: string;
  readonly onChange: (value: string) => void;
}

/** A filter box. */
export function Search({ label, placeholder, value, onChange }: SearchProps) {
  return (
    <div className="p-search">
      <Icon name="search" size="sm" />
      <input
        type="search"
        aria-label={label}
        placeholder={placeholder}
        value={value}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
    </div>
  );
}

export type Strength = 'weak' | 'fair' | 'good' | 'strong';

/** Passphrase strength as four bars. Decorative: the hint states the strength in words. */
export function Meter({ strength }: { readonly strength: Strength }) {
  return (
    <div className={`p-meter p-meter--${strength}`} aria-hidden="true">
      <i />
      <i />
      <i />
      <i />
    </div>
  );
}
