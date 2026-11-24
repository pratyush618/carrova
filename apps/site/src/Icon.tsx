import { ICONS, type IconName } from '../../desktop/src/ui/Icon/icons';

/** The desktop app's line icons, so the site and app draw the same glyphs. Decorative only. */
export function Icon({ name }: { readonly name: IconName }) {
  return (
    <svg className="icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      {ICONS[name].map((d) => (
        <path key={d} d={d} />
      ))}
    </svg>
  );
}
