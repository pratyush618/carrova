import { setThemePref, useScheme } from './theme';

// Sun and moon line glyphs, drawn like the app's icons (24-unit grid, stroked).
const SUN =
  'M12 8a4 4 0 1 0 0 8a4 4 0 1 0 0-8M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M5.3 18.7l1.4-1.4M17.3 6.7l1.4-1.4';
const MOON = 'M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z';

/** One button flips light and dark; until it's pressed, the page follows the OS. */
export function ThemeSwitch() {
  const scheme = useScheme();
  const next = scheme === 'dark' ? 'light' : 'dark';
  return (
    <button
      type="button"
      className="theme-switch"
      aria-label={`Switch to ${next} theme`}
      title={`Switch to ${next} theme`}
      onClick={(event) => {
        const r = event.currentTarget.getBoundingClientRect();
        setThemePref(next, { x: r.left + r.width / 2, y: r.top + r.height / 2 });
      }}
    >
      <svg className="icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <path d={scheme === 'dark' ? SUN : MOON} />
      </svg>
    </button>
  );
}
