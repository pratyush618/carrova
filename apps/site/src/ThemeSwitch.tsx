import { setThemePref, useThemePref, type ThemePref } from './theme';

const OPTIONS: readonly { value: ThemePref; label: string }[] = [
  { value: 'system', label: 'System' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
];

/** System / Light / Dark, the same three choices as the app's Appearance setting. */
export function ThemeSwitch() {
  const pref = useThemePref();
  return (
    <fieldset className="theme-switch">
      <legend className="sr-only">Theme</legend>
      {OPTIONS.map((o) => (
        <label key={o.value}>
          <input
            type="radio"
            name="theme"
            value={o.value}
            checked={pref === o.value}
            onChange={(event) => {
              const r = event.currentTarget.getBoundingClientRect();
              setThemePref(o.value, { x: r.left + r.width / 2, y: r.top + r.height / 2 });
            }}
          />
          <span>{o.label}</span>
        </label>
      ))}
    </fieldset>
  );
}
