import './Toggle.css';

export interface ToggleProps {
  readonly on: boolean;
  /** Accessible name: what turning it on does. */
  readonly label: string;
  readonly onChange: (on: boolean) => void;
  readonly disabled?: boolean;
}

/** An on/off switch. Space or Enter flips it (native button behaviour). */
export function Toggle({ on, label, onChange, disabled = false }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      className="p-toggle"
      aria-checked={on}
      aria-label={label}
      disabled={disabled}
      onClick={() => {
        onChange(!on);
      }}
    />
  );
}
